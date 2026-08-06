//! Reading the trade journal: structured queries and semantic search.
//!
//! Each tool resolves the calling user from the per-request `UserContext`, scopes the
//! read-service call to them, and serializes through the shared envelope.

use rmcp::{
    ErrorData, RoleServer, handler::server::wrapper::Parameters, model::*, service::RequestContext,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use base64::Engine as _;
use tradstry_backend::service::db::schema::tables::{tags_table, trading_principle_table};
use tradstry_backend::service::read_service::journal as journal_service;

use crate::server::{TradstryMcp, envelope, internal, project, validate_keys};

/// Trade outcome filter for `query_trades`.
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum TradeStatusParam {
    Profit,
    Loss,
}

/// Parameters for `query_trades`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryTradesParams {
    /// Optional ticker symbol to filter by (e.g. "AAPL"). Case-insensitive.
    pub symbol: Option<String>,
    /// Optional trading account id to restrict results to a single account.
    pub workspace_id: Option<String>,
    /// Optional playbook id to restrict results to trades that use that playbook.
    /// Ignored when `untagged_only` is true.
    pub playbook_id: Option<String>,
    /// When true, return only trades that have NO playbook assigned (overrides `playbook_id`).
    pub untagged_only: Option<bool>,
    /// Restrict to winning ("profit") or losing ("loss") trades.
    pub status: Option<TradeStatusParam>,
    /// Only trades whose percent P/L is at least this (percent, not dollars; e.g. -50).
    pub min_pl_pct: Option<f64>,
    /// Only trades whose percent P/L is at most this (percent, not dollars).
    pub max_pl_pct: Option<f64>,
    /// `true` = only trades with a stop-loss set; `false` = only trades with no stop.
    pub has_stop_loss: Option<bool>,
    /// Case-insensitive substring to search for within each trade's `mistakes` notes
    /// (e.g. "30-min rule"). Filtered server-side.
    pub mistake_contains: Option<String>,
    /// Optional inclusive lower bound on the trade close date (ISO 8601, e.g. "2025-01-01").
    pub date_from: Option<String>,
    /// Optional inclusive upper bound on the trade close date (ISO 8601, e.g. "2025-12-31").
    pub date_to: Option<String>,
    /// Maximum number of trades to return. Defaults to 50 (max 500).
    pub limit: Option<u32>,
    /// Optional subset of top-level fields to return per trade (e.g.
    /// ["id","symbol","pl_dollars","playbook_id"]) to cut token cost. Omit for all fields.
    /// Unknown names are rejected with the list of valid ones.
    pub fields: Option<Vec<String>>,
    /// Opaque pagination cursor from a previous response's `next_cursor`. Returns
    /// the next page of trades after it. Omit for the first page.
    pub after_cursor: Option<String>,
}

/// Parameters for `search_trades`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SearchTradesParams {
    /// Natural-language query to semantically search the user's trades and notes.
    pub query: String,
    /// Trading account id to scope the search to. Required: the vector index is
    /// partitioned by account.
    pub workspace_id: Option<String>,
    /// Optional inclusive lower bound on trade close date (ISO 8601) to scope the search.
    pub date_from: Option<String>,
    /// Optional inclusive upper bound on trade close date (ISO 8601) to scope the search.
    pub date_to: Option<String>,
    /// Maximum number of results to return. Defaults to 10, capped at 100.
    pub limit: Option<u32>,
}

/// Keyset cursor: `created_at` is immutable, unlike the user-editable `close_date`, so a
/// page boundary cannot shift under an edit mid-pagination.
#[derive(serde::Serialize, serde::Deserialize)]
struct Cursor {
    c: String,
    i: String,
}

fn encode_cursor(created_at: &str, id: &str) -> String {
    let json = serde_json::to_vec(&Cursor {
        c: created_at.to_string(),
        i: id.to_string(),
    })
    .unwrap_or_default();
    base64::engine::general_purpose::STANDARD.encode(json)
}

fn decode_cursor(cursor: &str) -> Option<(String, String)> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cursor)
        .ok()?;
    let c: Cursor = serde_json::from_slice(&bytes).ok()?;
    Some((c.c, c.i))
}

#[tool_router(router = journal_router, vis = "pub")]
impl TradstryMcp {
    #[tool(
        description = "Query the user's journaled trades. IMPORTANT — money vs percent: `pl_dollars` is the realized P&L in account currency and is the ONLY field to sum or total; `pl_percent` is the percent change from entry to exit and must never be added up or reported as money. Optional filters, all applied in SQL: symbol, account, playbook (or untagged-only), status (profit/loss), percent-P/L range (min_pl_pct/max_pl_pct, in percent), stop-loss presence (has_stop_loss), a case-insensitive substring match on the mistakes field (mistake_contains), and inclusive close-date range. Each trade carries its current `tags` (each with the category `role`) and `violated_principle_ids`, so you can see what is already linked before calling tag_trade or flag_violation — a tag whose role is `mistake` is what marks a trade flawed. Fields that are unset for a trade are omitted from the response. Row limit defaults to 50 (max 500)."
    )]
    pub async fn query_trades(
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
            workspace_id: params.workspace_id,
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

        validate_keys(
            "fields",
            params.fields.as_deref(),
            crate::views::TRADE_FIELDS,
        )?;
        // Batched, not per-trade: a page can be 500 rows, and the model needs a trade's
        // current tags and violations to link anything sensibly — `add`/`remove` are
        // guesswork without them.
        let entry_ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
        let mut tags_by_trade = tags_table::tags_for_trades(user_db.pool(), &entry_ids)
            .await
            .map_err(internal)?;
        let mut violations_by_trade = trading_principle_table::principles_for_trades(
            user_db.pool(),
            user_db.user_id(),
            &entry_ids,
        )
        .await
        .map_err(internal)?;

        let trades: Vec<crate::views::McpTrade> = entries
            .iter()
            .map(|e| {
                let mut t = crate::views::McpTrade::from(e);
                t.tags = tags_by_trade
                    .remove(&e.id)
                    .unwrap_or_default()
                    .iter()
                    .map(crate::views::McpTradeTag::from)
                    .collect();
                t.violated_principle_ids = violations_by_trade.remove(&e.id).unwrap_or_default();
                t
            })
            .collect();
        let data = project(
            serde_json::to_value(&trades).map_err(internal)?,
            params.fields.as_deref(),
        );
        envelope(data, next_cursor)
    }

    #[tool(
        description = "Semantically search the user's trades and notes. Requires a workspace_id — call list_workspaces first to obtain one."
    )]
    pub async fn search_trades(
        &self,
        Parameters(params): Parameters<SearchTradesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;

        // The hybrid vector index is partitioned by account, so a scope account
        // id is required (matches the in-app semantic_search tool).
        let workspace_id = params.workspace_id.ok_or_else(|| {
            ErrorData::invalid_params("workspace_id is required for semantic search", None)
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
                &workspace_id,
                params.date_from.as_deref(),
                params.date_to.as_deref(),
                top_k,
            )
            .await
            .map_err(internal)?;

        envelope(&results, None)
    }
}

#[cfg(test)]
use tradstry_backend::service::db::schema::tables::journal_table::JournalEntry;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tradstry_backend::service::db::schema::tables::journal_table::JournalEntry;

    /// Build a minimal `JournalEntry` with only the fields used by `filter_entries`.
    fn make_entry(symbol: &str, close_date: &str) -> JournalEntry {
        JournalEntry {
            id: "test-id".to_string(),
            user_id: "u1".to_string(),
            workspace_id: "acc1".to_string(),
            open_date: "2025-01-01".to_string(),
            close_date: close_date.to_string(),
            entry_price: 0.0,
            exit_price: 0.0,
            position_size: 0.0,
            contract_multiplier: 1.0,
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
