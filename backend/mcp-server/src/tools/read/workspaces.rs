//! Workspace listing. The entry point: every other tool needs a `workspace_id` from here.
//!
//! Each tool resolves the calling user from the per-request `UserContext`, scopes the
//! read-service call to them, and serializes through the shared envelope.

use rmcp::{
    ErrorData, RoleServer, handler::server::wrapper::Parameters, model::*, service::RequestContext,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use tradstry_backend::service::read_service::workspaces as workspaces_service;

use crate::server::{TradstryMcp, envelope, internal};

/// Parameters for `list_workspaces`.
///
/// No inputs are required — the tool returns all workspaces belonging to the
/// authenticated user.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListWorkspacesParams {}

#[tool_router(router = workspaces_router, vis = "pub")]
impl TradstryMcp {
    #[tool(
        description = "List the user's trading workspaces (id, name, asset class, broker, currency). Call this first to obtain a workspace_id for calculate_analytics, advanced_analytics, or search_trades. A workspace can have at most one brokerage account. `total_value` is omitted when the broker reports none — absent means unknown, not zero."
    )]
    pub async fn list_workspaces(
        &self,
        Parameters(_p): Parameters<ListWorkspacesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let workspaces: Vec<crate::views::McpWorkspace> =
            workspaces_service::list_workspaces(&user_db)
                .await
                .map_err(internal)?
                .iter()
                .map(crate::views::McpWorkspace::from)
                .collect();

        envelope(&workspaces, None)
    }
}
