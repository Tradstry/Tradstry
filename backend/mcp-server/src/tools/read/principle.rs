//! Trading principles: the rules the user wrote for themselves.
//!
//! Each tool resolves the calling user from the per-request `UserContext`, scopes the
//! read-service call to them, and serializes through the shared envelope.

use rmcp::{
    ErrorData, RoleServer, handler::server::wrapper::Parameters, model::*, service::RequestContext,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use tradstry_backend::service::read_service::principle as principle_service;

use crate::server::{TradstryMcp, envelope, internal};

/// Parameters for `get_principles`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetPrinciplesParams {
    /// The trading account whose principles to return. Call `list_workspaces` first.
    pub workspace_id: String,
    /// Optional: narrow to one playbook's principles plus the account-wide ones.
    pub playbook_id: Option<String>,
}

#[tool_router(router = principle_router, vis = "pub")]
impl TradstryMcp {
    #[tool(
        description = "Get the user's trading principles for an account: the rule, why it exists, \
                       the intervention that enforces it, and what breaking it has cost (violation \
                       count, cumulative P&L in dollars and percent, win rate on violating trades). \
                       Covers account-wide principles (playbookId null) and playbook-scoped ones. \
                       Requires workspace_id — call list_workspaces first. Pass playbook_id to narrow \
                       to that playbook's principles plus the account-wide ones."
    )]
    pub async fn get_principles(
        &self,
        Parameters(params): Parameters<GetPrinciplesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let principles = principle_service::list_principles(&user_db, &params.workspace_id)
            .await
            .map_err(internal)?;

        let filtered: Vec<_> = match params.playbook_id {
            Some(playbook_id) => principles
                .into_iter()
                .filter(|p| {
                    p.playbook_id.is_none() || p.playbook_id.as_deref() == Some(&playbook_id)
                })
                .collect(),
            None => principles,
        };

        envelope(&filtered, None)
    }
}
