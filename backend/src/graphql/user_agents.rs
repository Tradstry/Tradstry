use async_graphql::{Context, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::read_service::users::ensure_user;
use crate::service::turso::TursoClient;
use crate::service::turso::client::UserDb;
use crate::service::turso::schema::tables::user_agents_table::{
    delete_user_agent, find_user_agent, list_user_agents, UserAgent,
};

async fn get_user_db(ctx: &Context<'_>) -> Result<UserDb> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let turso = ctx.data::<Arc<TursoClient>>()?;
    let conn = turso.get_connection()?;

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

    let user = ensure_user(&conn, &jwt.sub, full_name, email).await?;

    Ok(turso.get_user_db(&user.id).await?)
}

#[derive(Default)]
pub struct UserAgentQuery;

#[Object]
impl UserAgentQuery {
    async fn user_agents(
        &self,
        ctx: &Context<'_>,
        account_id: String,
    ) -> Result<Vec<UserAgent>> {
        let user_db = get_user_db(ctx).await?;
        Ok(list_user_agents(user_db.conn(), user_db.user_id(), &account_id).await?)
    }

    async fn user_agent(
        &self,
        ctx: &Context<'_>,
        id: String,
    ) -> Result<Option<UserAgent>> {
        let user_db = get_user_db(ctx).await?;
        Ok(find_user_agent(user_db.conn(), &id, user_db.user_id()).await?)
    }
}

#[derive(Default)]
pub struct UserAgentMutation;

#[Object]
impl UserAgentMutation {
    async fn delete_user_agent(
        &self,
        ctx: &Context<'_>,
        id: String,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(delete_user_agent(user_db.conn(), &id, user_db.user_id()).await?)
    }
}
