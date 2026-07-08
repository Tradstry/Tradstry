//! The Tradstry MCP server handler and its read-only tools.
//!
//! Every tool resolves the calling user from the per-request `UserContext`
//! that `auth::require_auth` threads through the Axum extensions, scopes the
//! backend read-service call to that user, and serializes the result to JSON.

use std::sync::Arc;

use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    service::RequestContext,
    tool, tool_handler, tool_router,
};

use base64::Engine as _;
use serde::Serialize;
use serde_json::Value;
use tradstry_backend::service::ai::vector_database::blocks::extract_notebook_blocks;
use tradstry_backend::service::db::schema::tables::journal_table::JournalEntry;
use tradstry_backend::service::media::extract_keyframes;
use tradstry_backend::service::read_service::{
    accounts as accounts_service,
    analytics::{self as analytics_service, AnalyticsTimeFilter},
    journal as journal_service, notebook as notebook_service, playbook as playbook_service,
};

use crate::app_state::AppState;
use crate::tools::{
    AdvancedAnalyticsParams, CalculateAnalyticsParams, GetNotebookParams, GetPlaybookParams,
    ListAccountsParams, QueryTradesParams, SearchTradesParams, TradeStatusParam, ViewMediaParams,
};
use crate::user_context::UserContext;

/// Extract the `YYYY-MM-DD` date portion from an ISO-prefixed close_date string.
///
/// All stored formats are ISO-prefixed (`"2025-01-15"`, `"2025-01-15T09:30:00+00:00"`,
/// `"2025-01-15 09:30"`), so the first 10 chars are always the date. Lexicographic
/// comparison of `YYYY-MM-DD` against `YYYY-MM-DD` is correct for inclusive date ranges.
///
/// Retained as a `#[cfg(test)]` helper for `filter_entries`; production filtering
/// now happens in SQL via `list_journal_entries_filtered`.
#[cfg(test)]
fn date_part(close_date: &str) -> &str {
    if close_date.len() >= 10 {
        &close_date[..10]
    } else {
        close_date.trim()
    }
}

/// Pure in-memory filter, retained as a `#[cfg(test)]` test helper. Production
/// `query_trades` now filters in SQL (`list_journal_entries_filtered`); this
/// mirrors that behavior for unit tests without a database connection.
///
/// Filters `entries` by optional `symbol` (case-insensitive), optional inclusive
/// `date_from`/`date_to` (YYYY-MM-DD, compared against the date portion of
/// `close_date`), then truncates to `limit` (default 50, max 500).
#[cfg(test)]
fn filter_entries(
    mut entries: Vec<JournalEntry>,
    symbol: Option<&str>,
    date_from: Option<&str>,
    date_to: Option<&str>,
    limit: Option<u32>,
) -> Vec<JournalEntry> {
    if let Some(sym) = symbol {
        let needle = sym.to_ascii_uppercase();
        entries.retain(|e| e.symbol.to_ascii_uppercase() == needle);
    }
    if let Some(from) = date_from {
        entries.retain(|e| date_part(&e.close_date) >= from);
    }
    if let Some(to) = date_to {
        entries.retain(|e| date_part(&e.close_date) <= to);
    }
    let cap = limit.unwrap_or(50).min(500) as usize;
    entries.truncate(cap);
    entries
}

/// The MCP server handler. Cloned once per session by the streamable-HTTP
/// transport; the shared `AppState` is behind an `Arc` so clones are cheap.
#[derive(Clone)]
pub struct TradstryMcp {
    #[allow(dead_code)] // read by the #[tool_handler] macro
    tool_router: ToolRouter<Self>,
    state: Arc<AppState>,
}

impl TradstryMcp {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            state,
        }
    }

    /// Resolve the authenticated user from the request extensions threaded by
    /// `auth::require_auth`. Returns an error if no user context is present
    /// (which should never happen for requests that passed the auth layer).
    fn user(&self, ctx: &RequestContext<RoleServer>) -> Result<UserContext, ErrorData> {
        ctx.extensions
            .get::<axum::http::request::Parts>()
            .and_then(|p| p.extensions.get::<UserContext>())
            .cloned()
            .ok_or_else(|| ErrorData::internal_error("missing user context", None))
    }

    /// Best-effort refresh of the local replica before a read, then open the
    /// user-scoped DB. Sync failures are logged and ignored — we serve local
    /// (possibly slightly stale) data rather than failing the tool.
    async fn synced_user_db(
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
fn internal<E: std::fmt::Display>(e: E) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

/// Current version of the tool-response envelope. Bump on any breaking change
/// to a tool's `data` shape so cache-sensitive LLM callers can detect it.
const RESPONSE_VERSION: u32 = 1;

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
fn envelope<T: Serialize>(
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

/// Project a JSON value to only the requested top-level `fields`. Applies to each
/// element when `value` is an array; `None` returns it unchanged. Lets LLM callers
/// request a subset (e.g. `["id","symbol","total_pl"]`) to cut token cost.
fn project(value: Value, fields: Option<&[String]>) -> Value {
    let Some(fields) = fields else { return value };
    match value {
        Value::Array(items) => {
            Value::Array(items.into_iter().map(|it| project_object(it, fields)).collect())
        }
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

/// Opaque keyset pagination cursor: base64 of `{c: created_at, i: id}`.
#[derive(Serialize, serde::Deserialize)]
struct Cursor {
    c: String,
    i: String,
}

/// Encode a `(created_at, id)` pair into an opaque cursor string.
fn encode_cursor(created_at: &str, id: &str) -> String {
    let json = serde_json::to_string(&Cursor {
        c: created_at.to_string(),
        i: id.to_string(),
    })
    .unwrap_or_default();
    base64::engine::general_purpose::STANDARD.encode(json)
}

/// Decode an opaque cursor back into `(created_at, id)`. Returns `None` on any
/// malformed input (treated as "start from the beginning").
fn decode_cursor(cursor: &str) -> Option<(String, String)> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cursor)
        .ok()?;
    let parsed: Cursor = serde_json::from_slice(&bytes).ok()?;
    Some((parsed.c, parsed.i))
}

#[tool_router]
impl TradstryMcp {
    #[tool(
        description = "Query the user's journaled trades. Optional filters, all applied in SQL: symbol, account, playbook (or untagged-only), status (profit/loss), percent-P/L range (min_pl_pct/max_pl_pct), stop-loss presence (has_stop_loss), a case-insensitive substring match on the mistakes field (mistake_contains), and inclusive close-date range. Row limit defaults to 50 (max 500)."
    )]
    async fn query_trades(
        &self,
        Parameters(params): Parameters<QueryTradesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        // `untagged_only` takes precedence over `playbook_id` and maps to the
        // tri-state `Some(None)` (trades with no playbook).
        let playbook_id = if params.untagged_only == Some(true) {
            Some(None)
        } else {
            params.playbook_id.map(Some)
        };
        let status = params.status.map(|s| match s {
            TradeStatusParam::Profit => journal_service::TradeStatus::Profit,
            TradeStatusParam::Loss => journal_service::TradeStatus::Loss,
        });

        // Filtering is pushed into SQL so the journal indexes are used and only
        // the requested rows are read (see `list_journal_entries_filtered`).
        let filter = journal_service::JournalFilter {
            account_id: params.account_id,
            symbol: params.symbol,
            playbook_id,
            status,
            min_pl_pct: params.min_pl_pct,
            max_pl_pct: params.max_pl_pct,
            has_stop_loss: params.has_stop_loss,
            mistake_contains: params.mistake_contains,
            date_from: params.date_from,
            date_to: params.date_to,
            after: params.after_cursor.as_deref().and_then(decode_cursor),
            limit: params.limit,
        };
        let entries = journal_service::list_journal_entries_filtered(&user_db, &filter)
            .await
            .map_err(internal)?;

        // A full page implies there may be more — return a cursor to the last row.
        let page_size = params.limit.unwrap_or(50).min(500) as usize;
        let next_cursor = if entries.len() == page_size {
            entries.last().map(|e| encode_cursor(&e.created_at, &e.id))
        } else {
            None
        };

        let data = project(
            serde_json::to_value(&entries).map_err(internal)?,
            params.fields.as_deref(),
        );
        envelope(data, next_cursor)
    }

    #[tool(
        description = "Compute the user's trading analytics (win rate, profit factor, R multiples). Requires an account_id — call list_accounts first to obtain one."
    )]
    async fn calculate_analytics(
        &self,
        Parameters(params): Parameters<CalculateAnalyticsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;

        // Analytics are aggregated per account, so an account id is required.
        let account_id = params.account_id.ok_or_else(|| {
            ErrorData::invalid_params("account_id is required for analytics", None)
        })?;

        let time_filter = match (params.date_from, params.date_to) {
            (Some(start_date), Some(end_date)) => AnalyticsTimeFilter::Custom {
                start_date,
                end_date,
            },
            _ => AnalyticsTimeFilter::Last1Year,
        };

        let user_db = self.synced_user_db(&u.user_id).await?;

        let analytics =
            analytics_service::get_journal_analytics(&user_db, &account_id, &time_filter)
                .await
                .map_err(internal)?;

        envelope(
            project(
                serde_json::to_value(&analytics).map_err(internal)?,
                params.include.as_deref(),
            ),
            None,
        )
    }

    #[tool(
        description = "Advanced trading analytics — expectancy ($/R), SQN, max drawdown, recovery factor, equity curve, R-distribution, streaks, holding time, and breakdowns by symbol/day-of-week/session/playbook plus behavioral (mistake cost). Requires an account_id — call list_accounts first to obtain one."
    )]
    async fn advanced_analytics(
        &self,
        Parameters(params): Parameters<AdvancedAnalyticsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;

        // Analytics are aggregated per account, so an account id is required.
        let account_id = params.account_id.ok_or_else(|| {
            ErrorData::invalid_params("account_id is required for analytics", None)
        })?;

        let time_filter = match (params.date_from, params.date_to) {
            (Some(start_date), Some(end_date)) => AnalyticsTimeFilter::Custom {
                start_date,
                end_date,
            },
            _ => AnalyticsTimeFilter::Last1Year,
        };

        let user_db = self.synced_user_db(&u.user_id).await?;

        let analytics =
            analytics_service::get_advanced_analytics(&user_db, &account_id, &time_filter)
                .await
                .map_err(internal)?;

        envelope(
            project(
                serde_json::to_value(&analytics).map_err(internal)?,
                params.include.as_deref(),
            ),
            None,
        )
    }

    #[tool(
        description = "Semantically search the user's trades and notes. Requires an account_id — call list_accounts first to obtain one."
    )]
    async fn search_trades(
        &self,
        Parameters(params): Parameters<SearchTradesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;

        // The hybrid vector index is partitioned by account, so a scope account
        // id is required (matches the in-app semantic_search tool).
        let account_id = params.account_id.ok_or_else(|| {
            ErrorData::invalid_params("account_id is required for semantic search", None)
        })?;

        // date_from/date_to are intentionally not exposed: the backend translates
        // them into Qdrant exact-match conditions (not a range), which would
        // silently return zero results for any real date range query. Passing
        // None skips those conditions entirely.
        let top_k = params.limit.unwrap_or(10).min(100) as u64;

        let results = self
            .state
            .vector_db
            .hybrid_search(
                &params.query,
                &u.user_id,
                &account_id,
                params.date_from.as_deref(),
                params.date_to.as_deref(),
                top_k,
            )
            .await
            .map_err(internal)?;

        envelope(&results, None)
    }

    #[tool(
        description = "Get the user's trading playbooks with full details (edge, entry/exit/position-sizing/additional rules) and performance stats (win rate, profit, trade count). Pass playbook_id for one; omit for all."
    )]
    async fn get_playbook(
        &self,
        Parameters(params): Parameters<GetPlaybookParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        match params.playbook_id {
            Some(id) => {
                let maybe_playbook = playbook_service::get_playbook(&user_db, &id)
                    .await
                    .map_err(internal)?;
                match maybe_playbook {
                    Some(playbook) => {
                        envelope(&playbook, None)
                    }
                    None => Ok(CallToolResult::success(vec![Content::text(
                        "Playbook not found.",
                    )])),
                }
            }
            None => {
                let playbooks = playbook_service::list_playbooks(&user_db)
                    .await
                    .map_err(internal)?;
                envelope(&playbooks, None)
            }
        }
    }

    #[tool(
        description = "Get the user's notebook notes with their full text content and a media manifest \
                       listing attached images and videos. Each media item exposes a media_id that can \
                       be passed to the view_media tool (coming soon) to retrieve the actual bytes. \
                       Pass note_id for a single note; pass account_id to scope the listing to one \
                       trading account; omit both to list all notes."
    )]
    async fn get_notebook(
        &self,
        Parameters(params): Parameters<GetNotebookParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let notes = match params.note_id {
            Some(ref id) => {
                match notebook_service::get_notebook_note(&user_db, id)
                    .await
                    .map_err(internal)?
                {
                    Some(note) => vec![note],
                    None => {
                        return Ok(CallToolResult::success(vec![Content::text(
                            "Notebook note not found.",
                        )]));
                    }
                }
            }
            None => notebook_service::list_notebook_notes(&user_db, params.account_id.as_deref())
                .await
                .map_err(internal)?,
        };

        let out: Vec<serde_json::Value> = notes
            .iter()
            .map(|n| {
                let text = extract_notebook_blocks(&n.document_json)
                    .into_iter()
                    .map(|b| b.text)
                    .collect::<Vec<_>>()
                    .join("\n");
                let media: Vec<serde_json::Value> = n
                    .images
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "media_id": m.id,
                            "media_type": m.media_type,
                            "content_type": m.content_type,
                            "width": m.width,
                            "height": m.height,
                            "duration_seconds": m.duration_seconds,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "id": n.id,
                    "title": n.title,
                    "account_id": n.account_id,
                    "folder_id": n.folder_id,
                    "text": text,
                    "media": media,
                })
            })
            .collect();

        envelope(&out, None)
    }

    #[tool(
        description = "List the user's trading accounts (id, name, broker). Call this first to obtain an account_id for calculate_analytics or search_trades."
    )]
    async fn list_accounts(
        &self,
        Parameters(_p): Parameters<ListAccountsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let accounts = accounts_service::list_accounts(&user_db)
            .await
            .map_err(internal)?;

        envelope(&accounts, None)
    }

    #[tool(
        description = "Fetch the raw bytes of a media item (image or video) from the user's notebook \
                       and return it as native image content so the model can view it directly. \
                       Pass a media_id obtained from the get_notebook tool's media manifest. \
                       For images, the content is returned inline. \
                       For videos, a text guidance response is returned (full keyframe analysis is not yet implemented)."
    )]
    async fn view_media(
        &self,
        Parameters(params): Parameters<ViewMediaParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        // Look up the media row scoped to the authenticated user.
        let media = notebook_service::find_notebook_image(&user_db, &params.media_id)
            .await
            .map_err(internal)?;

        let Some(media) = media else {
            return Ok(CallToolResult::success(vec![Content::text(
                "Media not found.",
            )]));
        };

        let key = &media.cloudinary_public_id;
        let content_type = media.content_type.clone();

        match media.media_type.as_str() {
            "image" => {
                let bytes = self.state.r2.get_object(key).await.map_err(internal)?;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                Ok(CallToolResult::success(vec![Content::image(
                    b64,
                    content_type,
                )]))
            }
            _ => {
                // Video: fetch bytes and extract keyframes via ffmpeg.
                let bytes = self.state.r2.get_object(key).await.map_err(internal)?;
                let frames = extract_keyframes(&bytes, 8).await;
                if frames.is_empty() {
                    return Ok(CallToolResult::success(vec![Content::text(
                        "Could not extract frames from this video (ffmpeg unavailable or unsupported format).",
                    )]));
                }
                let media_id = &params.media_id;
                let mut contents: Vec<Content> = Vec::with_capacity(frames.len() + 1);
                contents.push(Content::text(format!(
                    "{} keyframes extracted from video {} (in chronological order); analyze them as frames of one clip.",
                    frames.len(),
                    media_id,
                )));
                for frame in frames {
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&frame);
                    contents.push(Content::image(b64, "image/jpeg"));
                }
                Ok(CallToolResult::success(contents))
            }
        }
    }
}

#[tool_handler]
impl ServerHandler for TradstryMcp {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = Implementation::new("tradstry-mcp", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(
            "Read-only access to the user's Tradstry trading journal and notebook. \
             Tools: list_accounts, query_trades, calculate_analytics, advanced_analytics, search_trades, get_playbook, get_notebook, view_media. \
             calculate_analytics, advanced_analytics, and search_trades require an account_id — call list_accounts first to obtain one. \
             advanced_analytics returns expectancy ($/R), SQN, max drawdown, recovery factor, equity curve, R-distribution, streaks, holding time, and breakdowns by symbol/day-of-week/session/playbook plus behavioral metrics (mistake cost). \
             get_notebook returns note text and a media manifest; each media item's media_id can be passed to \
             view_media to fetch and view the actual image bytes. \
             view_media returns images as native image content and stubs video (keyframes in a later release)."
                .to_string(),
        );
        info
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tradstry_backend::service::db::schema::tables::journal_table::JournalEntry;

    /// Build a minimal `JournalEntry` with only the fields used by `filter_entries`.
    fn make_entry(symbol: &str, close_date: &str) -> JournalEntry {
        JournalEntry {
            id: "test-id".to_string(),
            user_id: "u1".to_string(),
            account_id: "acc1".to_string(),
            open_date: "2025-01-01".to_string(),
            close_date: close_date.to_string(),
            entry_price: 0.0,
            exit_price: 0.0,
            position_size: 0.0,
            symbol: symbol.to_string(),
            symbol_name: String::new(),
            status: "closed".to_string(),
            total_pl: 0.0,
            net_roi: 0.0,
            duration: 0,
            stop_loss: None,
            risk_reward: None,
            trade_type: "long".to_string(),
            mistakes: String::new(),
            entry_tactics: String::new(),
            edges_spotted: String::new(),
            playbook_id: None,
            notes: None,
            broke_30min_rule: None,
            pre_trade_conviction: None,
            market_regime: None,
            is_planned_pre_market: None,
            revenge_trade: None,
            rule_adherence_score: None,
            created_at: close_date.to_string(),
        }
    }

    // --- date_part helper ---

    #[test]
    fn date_part_strips_time_component() {
        assert_eq!(date_part("2025-01-15T09:30:00+00:00"), "2025-01-15");
        assert_eq!(date_part("2025-01-15 09:30"), "2025-01-15");
        assert_eq!(date_part("2025-01-15"), "2025-01-15");
    }

    #[test]
    fn date_part_short_string_returned_trimmed() {
        assert_eq!(date_part("2025"), "2025");
    }

    // --- Fix 1: date_to inclusive with time-component close_date ---

    /// Regression: a trade closed on 2025-01-15 with a time component must be
    /// included when date_to = "2025-01-15".  The old raw-string compare would
    /// have excluded it because "2025-01-15T..." > "2025-01-15".
    #[test]
    fn date_to_includes_same_day_with_time_component() {
        let entries = vec![make_entry("AAPL", "2025-01-15T09:30:00+00:00")];
        let result = filter_entries(entries, None, None, Some("2025-01-15"), None);
        assert_eq!(
            result.len(),
            1,
            "same-day trade with time component should be included"
        );
    }

    #[test]
    fn date_to_excludes_next_day() {
        let entries = vec![make_entry("AAPL", "2025-01-16")];
        let result = filter_entries(entries, None, None, Some("2025-01-15"), None);
        assert!(
            result.is_empty(),
            "trade on 2025-01-16 should be excluded by date_to 2025-01-15"
        );
    }

    #[test]
    fn date_from_includes_same_day_with_time_component() {
        let entries = vec![make_entry("AAPL", "2025-01-15T09:30:00+00:00")];
        let result = filter_entries(entries, None, Some("2025-01-15"), None, None);
        assert_eq!(
            result.len(),
            1,
            "same-day trade with time component should be included by date_from"
        );
    }

    #[test]
    fn date_from_excludes_earlier_day() {
        let entries = vec![make_entry("AAPL", "2025-01-14")];
        let result = filter_entries(entries, None, Some("2025-01-15"), None, None);
        assert!(
            result.is_empty(),
            "trade on 2025-01-14 should be excluded by date_from 2025-01-15"
        );
    }

    // --- Symbol filter (case-insensitive) ---

    #[test]
    fn symbol_filter_is_case_insensitive() {
        let entries = vec![
            make_entry("aapl", "2025-01-15"),
            make_entry("TSLA", "2025-01-15"),
        ];
        let result = filter_entries(entries, Some("AAPL"), None, None, None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].symbol, "aapl");
    }

    // --- Limit clamp ---

    #[test]
    fn limit_truncates_results() {
        let entries: Vec<JournalEntry> = (0..10)
            .map(|i| make_entry("AAPL", &format!("2025-01-{:02}", i + 1)))
            .collect();
        let result = filter_entries(entries, None, None, None, Some(3));
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn limit_defaults_to_50() {
        let entries: Vec<JournalEntry> = (0..60)
            .map(|i| make_entry("AAPL", &format!("2025-01-{:02}", (i % 28) + 1)))
            .collect();
        let result = filter_entries(entries, None, None, None, None);
        assert_eq!(result.len(), 50);
    }

    #[test]
    fn limit_clamped_to_500() {
        let entries: Vec<JournalEntry> = (0..600)
            .map(|i| make_entry("AAPL", &format!("2025-01-{:02}", (i % 28) + 1)))
            .collect();
        let result = filter_entries(entries, None, None, None, Some(1000));
        assert_eq!(result.len(), 500);
    }
}
