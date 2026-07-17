//! Account listing. The entry point: every other tool needs an `account_id` from here.
//!
//! Each tool resolves the calling user from the per-request `UserContext`, scopes the
//! read-service call to them, and serializes through the shared envelope.

use rmcp::{
    ErrorData, RoleServer, handler::server::wrapper::Parameters, model::*, service::RequestContext,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use tradstry_backend::service::read_service::accounts as accounts_service;

use crate::server::{TradstryMcp, envelope, internal};

/// Parameters for `list_accounts`.
///
/// No inputs are required — the tool returns all accounts belonging to the
/// authenticated user.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListAccountsParams {}

#[tool_router(router = accounts_router, vis = "pub")]
impl TradstryMcp {
    #[tool(
        description = "List the user's trading accounts (id, name, broker, currency). Call this first to obtain an account_id for calculate_analytics, advanced_analytics, or search_trades. `total_value` is the broker's reported account value and is omitted entirely when the broker reports none — absent means unknown, not zero."
    )]
    pub async fn list_accounts(
        &self,
        Parameters(_p): Parameters<ListAccountsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let accounts: Vec<crate::views::McpAccount> = accounts_service::list_accounts(&user_db)
            .await
            .map_err(internal)?
            .iter()
            .map(crate::views::McpAccount::from)
            .collect();

        envelope(&accounts, None)
    }
}
