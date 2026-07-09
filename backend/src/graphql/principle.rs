use async_graphql::{Context, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::db::Db;
use crate::service::db::schema::tables::trading_principle_table::{
    CreatePrincipleInput, UpdatePrincipleInput,
};
use crate::service::read_service::principle as principle_service;
use crate::service::read_service::users::ensure_user;

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
pub struct PrincipleQuery;

#[Object]
impl PrincipleQuery {
    async fn principles(
        &self,
        ctx: &Context<'_>,
        account_id: String,
    ) -> Result<Vec<principle_service::PrincipleWithStats>> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::list_principles(&user_db, &account_id).await?)
    }

    async fn principle(
        &self,
        ctx: &Context<'_>,
        id: String,
    ) -> Result<Option<principle_service::PrincipleWithStats>> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::get_principle(&user_db, &id).await?)
    }
}

#[derive(Default)]
pub struct PrincipleMutation;

// No `ai_jobs::enqueue_all_account_reindex` here: principles are not indexed
// into the vector store, so a reindex would be pure cost.
#[Object]
impl PrincipleMutation {
    async fn create_principle(
        &self,
        ctx: &Context<'_>,
        input: CreatePrincipleInput,
    ) -> Result<principle_service::PrincipleWithStats> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::create_principle(&user_db, input).await?)
    }

    async fn update_principle(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdatePrincipleInput,
    ) -> Result<principle_service::PrincipleWithStats> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::update_principle(&user_db, &id, input).await?)
    }

    async fn delete_principle(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::delete_principle(&user_db, &id).await?)
    }

    async fn reorder_principles(
        &self,
        ctx: &Context<'_>,
        ordered_ids: Vec<String>,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::reorder_principles(&user_db, &ordered_ids).await?)
    }
}
