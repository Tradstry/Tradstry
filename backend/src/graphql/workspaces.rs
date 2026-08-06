use async_graphql::{Context, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::db::Db;
use crate::service::db::schema::tables::workspaces_table::{
    CreateWorkspaceInput, UpdateWorkspaceInput, Workspace,
};
use crate::service::read_service::users::ensure_user;
use crate::service::read_service::workspaces as workspace_service;

/// Resolve or create the internal user ID from the JWT, then build a UserDb.
async fn get_user_db(ctx: &Context<'_>) -> Result<crate::service::db::client::UserDb> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let db = ctx.data::<Arc<Db>>()?;
    let pool = db.pool();

    let full_name = jwt
        .other
        .get("full_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let email = jwt
        .other
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let user = ensure_user(pool, &jwt.sub, full_name, email).await?;

    Ok(db.get_user_db(&user.id))
}

#[derive(Default)]
pub struct WorkspaceQuery;

#[Object]
impl WorkspaceQuery {
    async fn workspaces(&self, ctx: &Context<'_>) -> Result<Vec<Workspace>> {
        let user_db = get_user_db(ctx).await?;
        Ok(workspace_service::list_workspaces(&user_db).await?)
    }

    async fn workspace(&self, ctx: &Context<'_>, id: String) -> Result<Option<Workspace>> {
        let user_db = get_user_db(ctx).await?;
        Ok(workspace_service::get_workspace(&user_db, &id).await?)
    }
}

#[derive(Default)]
pub struct WorkspaceMutation;

#[Object]
impl WorkspaceMutation {
    async fn create_workspace(
        &self,
        ctx: &Context<'_>,
        input: CreateWorkspaceInput,
    ) -> Result<Workspace> {
        let user_db = get_user_db(ctx).await?;
        Ok(workspace_service::create_workspace(&user_db, input).await?)
    }

    async fn update_workspace(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdateWorkspaceInput,
    ) -> Result<Workspace> {
        let user_db = get_user_db(ctx).await?;
        Ok(workspace_service::update_workspace(&user_db, &id, input).await?)
    }

    async fn delete_workspace(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(workspace_service::delete_workspace(&user_db, &id).await?)
    }
}
