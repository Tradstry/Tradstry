//! The MCP server handler: shared plumbing only.
//!
//! The tools live in `tools::read::*` (one module per domain) and `tools::write::*`, each
//! with its own `#[tool_router]`, all merged in `new`. What stays here is what every tool
//! needs: user resolution, the response envelope, field projection, and key validation.

use std::sync::Arc;

use rmcp::{
    ErrorData, RoleServer, ServerHandler, handler::server::router::tool::ToolRouter, model::*,
    service::RequestContext, tool_handler,
};

use serde::Serialize;
use serde_json::Value;

use crate::app_state::AppState;
use crate::user_context::UserContext;

/// The MCP server handler. Cloned once per session by the streamable-HTTP
/// transport; the shared `AppState` is behind an `Arc` so clones are cheap.
#[derive(Clone)]
pub struct TradstryMcp {
    tool_router: ToolRouter<Self>,
    pub(crate) state: Arc<AppState>,
}

impl TradstryMcp {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            // One router per domain (tools/read/*) plus the notebook write tools.
            tool_router: Self::workspaces_router()
                + Self::journal_router()
                + Self::analytics_router()
                + Self::playbook_router()
                + Self::principle_router()
                + Self::notebook_router()
                + Self::tags_read_router()
                + Self::write_router()
                + Self::playbook_write_router()
                + Self::tags_write_router()
                + Self::principle_write_router(),
            state,
        }
    }

    /// Resolve the authenticated user from the request extensions threaded by
    /// `auth::require_auth`. Returns an error if no user context is present
    /// (which should never happen for requests that passed the auth layer).
    pub(crate) fn user(&self, ctx: &RequestContext<RoleServer>) -> Result<UserContext, ErrorData> {
        ctx.extensions
            .get::<axum::http::request::Parts>()
            .and_then(|p| p.extensions.get::<UserContext>())
            .cloned()
            .ok_or_else(|| ErrorData::internal_error("missing user context", None))
    }

    /// Best-effort refresh of the local replica before a read, then open the
    /// user-scoped DB. Sync failures are logged and ignored — we serve local
    /// (possibly slightly stale) data rather than failing the tool.
    pub(crate) async fn synced_user_db(
        &self,
        user_id: &str,
    ) -> Result<tradstry_backend::service::db::client::UserDb, ErrorData> {
        // Postgres is the single source of truth — no replica to sync. Reads are
        // always current.
        Ok(self.state.db.get_user_db(user_id))
    }
}

/// Convert any `anyhow::Error` (or other displayable error) into an rmcp
/// internal error.
pub(crate) fn internal<E: std::fmt::Display>(e: E) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

/// Current version of the tool-response envelope. Bump on any breaking change to a tool's
/// `data` shape so cache-sensitive LLM callers can detect it.
///
/// 2: trades expose `pl_dollars` + `pl_percent` instead of the ambiguous `total_pl` (a
/// percent) and its duplicate `net_roi`; accounts no longer leak internal ids.
pub(crate) const RESPONSE_VERSION: u32 = 2;

/// Versioned envelope wrapping every structured tool response, so adding fields
/// to `data` (or paginating it) is forward-compatible rather than a silent
/// schema break. `next_cursor` is present only on paginated tools.
#[derive(Serialize)]
struct Envelope<T> {
    version: u32,
    data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
}

/// Serialize `data` inside the versioned envelope and package it as MCP text.
pub(crate) fn envelope<T: Serialize>(
    data: T,
    next_cursor: Option<String>,
) -> Result<CallToolResult, ErrorData> {
    let json = serde_json::to_string(&Envelope {
        version: RESPONSE_VERSION,
        data,
        next_cursor,
    })
    .map_err(internal)?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

/// Reject unknown `fields` / `include` keys instead of dropping them.
///
/// Silently ignoring a key a model asked for is the worst failure mode available: it gets a
/// well-formed response with the section missing and cannot tell "you named it wrong" from
/// "you have no data". Naming the valid keys turns a silent hole into a fixable error.
pub(crate) fn validate_keys(
    param: &str,
    keys: Option<&[String]>,
    valid: &[&str],
) -> Result<(), ErrorData> {
    let Some(keys) = keys else { return Ok(()) };
    let unknown: Vec<&str> = keys
        .iter()
        .map(String::as_str)
        .filter(|k| !valid.contains(k))
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    Err(ErrorData::invalid_params(
        format!(
            "unknown {param}: {}. Valid values: {}",
            unknown.join(", "),
            valid.join(", ")
        ),
        None,
    ))
}

/// Project a JSON value to only the requested top-level `fields`. Applies to each
/// element when `value` is an array; `None` returns it unchanged. Lets LLM callers
/// request a subset (e.g. `["id","symbol","pl_dollars"]`) to cut token cost.
pub(crate) fn project(value: Value, fields: Option<&[String]>) -> Value {
    let Some(fields) = fields else { return value };
    match value {
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|it| project_object(it, fields))
                .collect(),
        ),
        other => project_object(other, fields),
    }
}

fn project_object(value: Value, fields: &[String]) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for f in fields {
                if let Some(v) = map.get(f) {
                    out.insert(f.clone(), v.clone());
                }
            }
            Value::Object(out)
        }
        other => other,
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TradstryMcp {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = Implementation::new("tradstry-mcp", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(
            "Read and write access to the user's Tradstry trading journal, playbooks, tags, \
             principles and notebook. \
             Call list_workspaces first — most tools need a workspace_id. \
             READ: query_trades, calculate_analytics, advanced_analytics, search_trades, \
             get_playbook, get_principles, list_tags, get_notebook, view_media. \
             WRITE: create_note/update_note/delete_note/move_note/create_folder (notebook, \
             Markdown bodies), create_playbook/update_playbook/delete_playbook/set_trade_playbook, \
             create_tag/tag_trade/delete_tag/merge_tags, \
             create_principle/update_principle/delete_principle/flag_violation. \
             MONEY: a trade's `pl_dollars` is the realized P&L and the ONLY field to sum; \
             `pl_percent` is a percent change and must never be reported as money. \
             LINKING: tags in the `mistake`-role category are what mark a trade flawed, which \
             drives the clean-vs-flawed and mistake-cost analytics; a principle can only be \
             violated by a trade in the account it governs. Prefer add over set when linking, \
             so you never discard the user's own judgment. \
             Agent-written notes belong in the account's System folder, which create_note uses \
             by default."
                .to_string(),
        );
        info
    }
}
