//! Playbooks: the user's written setups, with their realized performance.
//!
//! Each tool resolves the calling user from the per-request `UserContext`, scopes the
//! read-service call to them, and serializes through the shared envelope.

use rmcp::{
    ErrorData, RoleServer, handler::server::wrapper::Parameters, model::*, service::RequestContext,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use tradstry_backend::service::read_service::playbook as playbook_service;

use crate::server::{TradstryMcp, envelope, internal};

/// Parameters for `get_playbook`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GetPlaybookParams {
    /// Optional playbook id. When supplied, returns stats for that single
    /// playbook; otherwise returns stats for all of the user's playbooks.
    pub playbook_id: Option<String>,
}

#[tool_router(router = playbook_router, vis = "pub")]
impl TradstryMcp {
    #[tool(
        description = "Get the user's trading playbooks with full details (edge, entry/exit/position-sizing/additional rules) and performance stats (win rate, profit, trade count). Pass playbook_id for one; omit for all."
    )]
    pub async fn get_playbook(
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
                    Some(playbook) => envelope(&playbook, None),
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
}
