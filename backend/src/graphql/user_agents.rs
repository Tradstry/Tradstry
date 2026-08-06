use async_graphql::{Context, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::db::Db;
use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::user_agents_table::{
    UserAgent, delete_user_agent, find_user_agent, list_user_agents,
};
use crate::service::read_service::users::ensure_user;

async fn get_user_db(ctx: &Context<'_>) -> Result<UserDb> {
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
pub struct UserAgentQuery;

#[Object]
impl UserAgentQuery {
    async fn user_agents(&self, ctx: &Context<'_>, workspace_id: String) -> Result<Vec<UserAgent>> {
        let user_db = get_user_db(ctx).await?;
        Ok(list_user_agents(user_db.pool(), user_db.user_id(), &workspace_id).await?)
    }

    async fn user_agent(&self, ctx: &Context<'_>, id: String) -> Result<Option<UserAgent>> {
        let user_db = get_user_db(ctx).await?;
        Ok(find_user_agent(user_db.pool(), &id, user_db.user_id()).await?)
    }
}

#[derive(Default)]
pub struct UserAgentMutation;

#[Object]
impl UserAgentMutation {
    async fn delete_user_agent(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(delete_user_agent(user_db.pool(), &id, user_db.user_id()).await?)
    }
}
