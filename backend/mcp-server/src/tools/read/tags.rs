//! Tag taxonomy: the categories and the tags inside them.
//!
//! Without this the write side is unusable — an agent cannot attach a tag to a trade
//! without first knowing the tag's id.

use rmcp::{
    ErrorData, RoleServer, handler::server::wrapper::Parameters, model::*, service::RequestContext,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use tradstry_backend::service::db::schema::tables::tags_table;

use crate::server::{TradstryMcp, envelope, internal};

/// Parameters for `list_tags`.
///
/// Tags are isolated per workspace.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListTagsParams {
    /// Workspace whose tag taxonomy should be returned. Obtain it from `list_workspaces`.
    pub workspace_id: String,
}

#[tool_router(router = tags_read_router, vis = "pub")]
impl TradstryMcp {
    #[tool(
        description = "List the user's tag categories and the tags inside each. Call this \
                       before tagging a trade — you need the tag ids. A category's `role` is \
                       what gives it analytic meaning: tags in the `mistake` category are \
                       what mark a trade as flawed, which drives the clean-vs-flawed and \
                       mistake-cost analytics."
    )]
    pub async fn list_tags(
        &self,
        Parameters(params): Parameters<ListTagsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;
        let pool = user_db.pool();
        let user_id = user_db.user_id();

        let categories = tags_table::list_categories(pool, user_id, &params.workspace_id)
            .await
            .map_err(internal)?;
        let tags = tags_table::list_tags(pool, user_id, &params.workspace_id, None)
            .await
            .map_err(internal)?;

        let out: Vec<serde_json::Value> = categories
            .iter()
            .map(|c| {
                let inner: Vec<serde_json::Value> = tags
                    .iter()
                    .filter(|t| t.category_id == c.id)
                    .map(|t| serde_json::json!({ "id": t.id, "name": t.name }))
                    .collect();
                serde_json::json!({
                    "id": c.id,
                    "name": c.name,
                    "role": c.role.as_ref().map(|r| r.as_str()),
                    "tags": inner,
                })
            })
            .collect();

        envelope(&out, None)
    }
}
