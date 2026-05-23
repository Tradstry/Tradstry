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

use tradstry_backend::service::read_service::{
    accounts as accounts_service,
    analytics::{self as analytics_service, AnalyticsTimeFilter},
    journal as journal_service, playbook as playbook_service,
};
use tradstry_backend::service::turso::schema::tables::journal_table::JournalEntry;

use crate::app_state::AppState;
use crate::tools::{
    CalculateAnalyticsParams, GetPlaybookStatsParams, ListAccountsParams, QueryTradesParams,
    SearchTradesParams,
};
use crate::user_context::UserContext;

/// Extract the `YYYY-MM-DD` date portion from an ISO-prefixed close_date string.
///
/// All stored formats are ISO-prefixed (`"2025-01-15"`, `"2025-01-15T09:30:00+00:00"`,
/// `"2025-01-15 09:30"`), so the first 10 chars are always the date. Lexicographic
/// comparison of `YYYY-MM-DD` against `YYYY-MM-DD` is correct for inclusive date ranges.
fn date_part(close_date: &str) -> &str {
    if close_date.len() >= 10 {
        &close_date[..10]
    } else {
        close_date.trim()
    }
}

/// Pure filter function for `query_trades`, extracted so it can be unit-tested
/// without a database connection.
///
/// Filters `entries` by optional `symbol` (case-insensitive), optional inclusive
/// `date_from`/`date_to` (YYYY-MM-DD, compared against the date portion of
/// `close_date`), then truncates to `limit` (default 50, max 500).
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
}

/// Convert any `anyhow::Error` (or other displayable error) into an rmcp
/// internal error.
fn internal<E: std::fmt::Display>(e: E) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

#[tool_router]
impl TradstryMcp {
    #[tool(description = "Query the user's journaled trades with optional symbol/date filters.")]
    async fn query_trades(
        &self,
        Parameters(params): Parameters<QueryTradesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self
            .state
            .turso
            .get_user_db(&u.user_id)
            .await
            .map_err(internal)?;

        // The journal read service returns every entry for the user; there is
        // no symbol/date filter at that layer, so we filter in memory.
        let entries = journal_service::list_journal_entries(&user_db)
            .await
            .map_err(internal)?;

        let entries = filter_entries(
            entries,
            params.symbol.as_deref(),
            params.date_from.as_deref(),
            params.date_to.as_deref(),
            params.limit,
        );

        let json = serde_json::to_string(&entries).map_err(internal)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
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

        let user_db = self
            .state
            .turso
            .get_user_db(&u.user_id)
            .await
            .map_err(internal)?;

        let analytics =
            analytics_service::get_journal_analytics(&user_db, &account_id, &time_filter)
                .await
                .map_err(internal)?;

        let json = serde_json::to_string(&analytics).map_err(internal)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
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
            .hybrid_search(&params.query, &u.user_id, &account_id, None, None, top_k)
            .await
            .map_err(internal)?;

        let json = serde_json::to_string(&results).map_err(internal)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get per-playbook performance stats (win rate per edge) for the user.")]
    async fn get_playbook_stats(
        &self,
        Parameters(params): Parameters<GetPlaybookStatsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self
            .state
            .turso
            .get_user_db(&u.user_id)
            .await
            .map_err(internal)?;

        match params.playbook_id {
            Some(id) => {
                let maybe_playbook = playbook_service::get_playbook(&user_db, &id)
                    .await
                    .map_err(internal)?;
                match maybe_playbook {
                    Some(playbook) => {
                        let json = serde_json::to_string(&playbook).map_err(internal)?;
                        Ok(CallToolResult::success(vec![Content::text(json)]))
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
                let json = serde_json::to_string(&playbooks).map_err(internal)?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
        }
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
        let user_db = self
            .state
            .turso
            .get_user_db(&u.user_id)
            .await
            .map_err(internal)?;

        let accounts = accounts_service::list_accounts(&user_db)
            .await
            .map_err(internal)?;

        let json = serde_json::to_string(&accounts).map_err(internal)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for TradstryMcp {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.server_info = Implementation::new("tradstry-mcp", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(
            "Read-only access to the user's Tradstry trading journal. \
             Tools: list_accounts, query_trades, calculate_analytics, search_trades, get_playbook_stats. \
             calculate_analytics and search_trades require an account_id — call list_accounts first to obtain one."
                .to_string(),
        );
        info
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tradstry_backend::service::turso::schema::tables::journal_table::JournalEntry;

    /// Build a minimal `JournalEntry` with only the fields used by `filter_entries`.
    fn make_entry(symbol: &str, close_date: &str) -> JournalEntry {
        JournalEntry {
            id: "test-id".to_string(),
            user_id: "u1".to_string(),
            account_id: "acc1".to_string(),
            reviewed: false,
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
            stop_loss: 0.0,
            risk_reward: 0.0,
            trade_type: "long".to_string(),
            mistakes: String::new(),
            entry_tactics: String::new(),
            edges_spotted: String::new(),
            playbook_id: None,
            notes: None,
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
