use async_graphql::{Context, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::db::Db;
use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::user_prompts_table::{self, UserPrompt};
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
