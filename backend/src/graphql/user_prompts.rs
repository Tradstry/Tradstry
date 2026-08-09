use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::user_prompts_table::{self, UserPrompt};
use async_graphql::{Context, Object, Result};

async fn get_user_db(ctx: &Context<'_>) -> Result<UserDb> {
    crate::graphql::auth::user_db(ctx).await
}

#[derive(Default)]
pub struct UserPromptQuery;

#[Object]
impl UserPromptQuery {
    async fn user_prompts(&self, ctx: &Context<'_>) -> Result<Vec<UserPrompt>> {
        let user_db = get_user_db(ctx).await?;
        Ok(user_prompts_table::list_user_prompts(user_db.pool(), user_db.user_id()).await?)
    }
}

#[derive(Default)]
pub struct UserPromptMutation;

#[Object]
impl UserPromptMutation {
    async fn create_user_prompt(
        &self,
        ctx: &Context<'_>,
        name: String,
        content: String,
    ) -> Result<UserPrompt> {
        let user_db = get_user_db(ctx).await?;
        Ok(user_prompts_table::create_user_prompt(
            user_db.pool(),
            user_db.user_id(),
            &name,
            &content,
        )
        .await?)
    }

    async fn update_user_prompt(
        &self,
        ctx: &Context<'_>,
        id: String,
        name: Option<String>,
        content: Option<String>,
    ) -> Result<UserPrompt> {
        let user_db = get_user_db(ctx).await?;
        Ok(user_prompts_table::update_user_prompt(
            user_db.pool(),
            &id,
            user_db.user_id(),
            name.as_deref(),
            content.as_deref(),
        )
        .await?)
    }

    async fn delete_user_prompt(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(user_prompts_table::delete_user_prompt(user_db.pool(), &id, user_db.user_id()).await?)
    }
}
