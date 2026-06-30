use async_graphql::{Context, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::db::Db;
use crate::service::db::schema::tables::accounts_table::{
    Account, CreateAccountInput, UpdateAccountInput,
};
use crate::service::read_service::accounts as account_service;
use crate::service::read_service::users::ensure_user;

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
pub struct AccountQuery;

#[Object]
impl AccountQuery {
    async fn accounts(&self, ctx: &Context<'_>) -> Result<Vec<Account>> {
        let user_db = get_user_db(ctx).await?;
        Ok(account_service::list_accounts(&user_db).await?)
    }

    async fn account(&self, ctx: &Context<'_>, id: String) -> Result<Option<Account>> {
        let user_db = get_user_db(ctx).await?;
        Ok(account_service::get_account(&user_db, &id).await?)
    }
}

#[derive(Default)]
pub struct AccountMutation;

#[Object]
impl AccountMutation {
    async fn create_account(
        &self,
        ctx: &Context<'_>,
        input: CreateAccountInput,
    ) -> Result<Account> {
        let user_db = get_user_db(ctx).await?;
        Ok(account_service::create_account(&user_db, input).await?)
    }

    async fn update_account(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdateAccountInput,
    ) -> Result<Account> {
        let user_db = get_user_db(ctx).await?;
        Ok(account_service::update_account(&user_db, &id, input).await?)
    }

    async fn delete_account(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(account_service::delete_account(&user_db, &id).await?)
    }
}
