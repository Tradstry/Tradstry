use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::user_agents_table::{
    UserAgent, delete_user_agent, find_user_agent, list_user_agents,
};
use async_graphql::{Context, Object, Result};

async fn get_user_db(ctx: &Context<'_>) -> Result<UserDb> {
    crate::graphql::auth::user_db(ctx).await
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
