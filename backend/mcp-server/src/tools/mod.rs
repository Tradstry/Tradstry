//! Input parameter structs for the Tradstry MCP tools.
//!
//! Each struct derives `Deserialize` (rmcp deserializes the JSON the client
//! sends), `Serialize`, and `schemars::JsonSchema` (rmcp turns this into the
//! tool's `inputSchema`). Doc comments on each field surface as the field
//! descriptions in that schema, so they double as user-facing documentation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `query_trades`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryTradesParams {
    /// Optional ticker symbol to filter by (e.g. "AAPL"). Case-insensitive.
    pub symbol: Option<String>,
    /// Optional inclusive lower bound on the trade close date (ISO 8601, e.g. "2025-01-01").
    pub date_from: Option<String>,
    /// Optional inclusive upper bound on the trade close date (ISO 8601, e.g. "2025-12-31").
    pub date_to: Option<String>,
    /// Maximum number of trades to return. Defaults to 50.
    pub limit: Option<u32>,
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
}

/// Parameters for `search_trades`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SearchTradesParams {
    /// Natural-language query to semantically search the user's trades and notes.
    pub query: String,
    /// Trading account id to scope the search to. Required: the vector index is
    /// partitioned by account.
    pub account_id: Option<String>,
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

/// Parameters for `view_media`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ViewMediaParams {
    /// The media_id of the image or video to fetch. Obtain media_ids from the
    /// `get_notebook` tool's media manifest. The media must belong to the
    /// authenticated user — foreign ids are rejected.
    pub media_id: String,
}
