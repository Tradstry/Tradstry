//! Computed analytics over the journal, basic and advanced.
//!
//! Each tool resolves the calling user from the per-request `UserContext`, scopes the
//! read-service call to them, and serializes through the shared envelope.

use rmcp::{
    ErrorData, RoleServer, handler::server::wrapper::Parameters, model::*, service::RequestContext,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use tradstry_backend::service::read_service::analytics::{
    self as analytics_service, AnalyticsTimeFilter,
};

use crate::server::{TradstryMcp, envelope, internal, project, validate_keys};

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
    /// ["profit_factor","biggest_loss"]) to cut token cost. Omit for the full blob.
    /// Unknown names are rejected with the list of valid ones.
    pub include: Option<Vec<String>>,
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
    /// Unknown names are rejected with the list of valid ones.
    pub include: Option<Vec<String>>,
}

#[tool_router(router = analytics_router, vis = "pub")]
impl TradstryMcp {
    #[tool(
        description = "Compute the user's trading analytics (win rate, profit factor, R multiples). Requires an account_id — call list_accounts first to obtain one."
    )]
    pub async fn calculate_analytics(
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

        validate_keys(
            "include",
            params.include.as_deref(),
            crate::views::ANALYTICS_SECTIONS,
        )?;
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
    pub async fn advanced_analytics(
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

        validate_keys(
            "include",
            params.include.as_deref(),
            crate::views::ADVANCED_SECTIONS,
        )?;
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
}
