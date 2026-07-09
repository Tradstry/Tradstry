//! Input parameter structs for the Tradstry MCP tools.
//!
//! Each struct derives `Deserialize` (rmcp deserializes the JSON the client
//! sends), `Serialize`, and `schemars::JsonSchema` (rmcp turns this into the
//! tool's `inputSchema`). Doc comments on each field surface as the field
//! descriptions in that schema, so they double as user-facing documentation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    pub account_id: Option<String>,
    /// Optional playbook id to restrict results to trades that use that playbook.
    /// Ignored when `untagged_only` is true.
    pub playbook_id: Option<String>,
    /// When true, return only trades that have NO playbook assigned (overrides `playbook_id`).
    pub untagged_only: Option<bool>,
    /// Restrict to winning ("profit") or losing ("loss") trades.
    pub status: Option<TradeStatusParam>,
    /// Only trades whose percent P/L is at least this (e.g. -50 to find big losers).
    pub min_pl_pct: Option<f64>,
    /// Only trades whose percent P/L is at most this (e.g. a ceiling to exclude moonshots).
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
    /// ["id","symbol","total_pl","playbook_id"]) to cut token cost. Omit for all fields.
    pub fields: Option<Vec<String>>,
    /// Opaque pagination cursor from a previous response's `next_cursor`. Returns
    /// the next page of trades after it. Omit for the first page.
    pub after_cursor: Option<String>,
}

/// Parameters for `calculate_analytics`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CalculateAnalyticsParams {
    /// Trading account id to compute analytics for. Required: analytics are
    /// aggregated per account.
    pub account_id: Option<String>,
    /// Optional inclusive start date for the analytics window (ISO 8601). When
    /// both `date_from` and `date_to` are supplied a custom range is used;
    /// otherwise the last year is used.
    pub date_from: Option<String>,
    /// Optional inclusive end date for the analytics window (ISO 8601).
    pub date_to: Option<String>,
    /// Optional subset of top-level analytics sections to return (e.g.
    /// ["profit_factor","by_playbook"]) to cut token cost. Omit for the full blob.
    pub include: Option<Vec<String>>,
}

/// Parameters for `search_trades`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SearchTradesParams {
    /// Natural-language query to semantically search the user's trades and notes.
    pub query: String,
    /// Trading account id to scope the search to. Required: the vector index is
    /// partitioned by account.
    pub account_id: Option<String>,
    /// Optional inclusive lower bound on trade close date (ISO 8601) to scope the search.
    pub date_from: Option<String>,
    /// Optional inclusive upper bound on trade close date (ISO 8601) to scope the search.
    pub date_to: Option<String>,
    /// Maximum number of results to return. Defaults to 10, capped at 100.
    pub limit: Option<u32>,
}

/// Parameters for `get_playbook`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetPlaybookParams {
    /// Optional playbook id. When supplied, returns stats for that single
    /// playbook; otherwise returns stats for all of the user's playbooks.
    pub playbook_id: Option<String>,
}

/// Parameters for `advanced_analytics`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AdvancedAnalyticsParams {
    /// Trading account id to compute advanced analytics for. Required: analytics
    /// are aggregated per account.
    pub account_id: Option<String>,
    /// Optional inclusive start date for the analytics window (ISO 8601). When
    /// both `date_from` and `date_to` are supplied a custom range is used;
    /// otherwise the last year is used.
    pub date_from: Option<String>,
    /// Optional inclusive end date for the analytics window (ISO 8601).
    pub date_to: Option<String>,
    /// Optional subset of top-level analytics sections to return (e.g.
    /// ["profit_factor","by_playbook"]) to cut token cost. Omit for the full blob.
    pub include: Option<Vec<String>>,
}

/// Parameters for `list_accounts`.
///
/// No inputs are required — the tool returns all accounts belonging to the
/// authenticated user.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListAccountsParams {}

/// Parameters for `get_notebook`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetNotebookParams {
    /// Optional single note id. When supplied, returns that one note; when
    /// omitted, all of the user's notes are returned (optionally scoped by
    /// account_id).
    pub note_id: Option<String>,
    /// Optional account id to scope the listing to a single trading account.
    pub account_id: Option<String>,
}

/// Parameters for `get_principles`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetPrinciplesParams {
    /// The trading account whose principles to return. Call `list_accounts` first.
    pub account_id: String,
    /// Optional: narrow to one playbook's principles plus the account-wide ones.
    pub playbook_id: Option<String>,
}

/// Parameters for `view_media`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ViewMediaParams {
    /// The media_id of the image or video to fetch. Obtain media_ids from the
    /// `get_notebook` tool's media manifest. The media must belong to the
    /// authenticated user — foreign ids are rejected.
    pub media_id: String,
}
