use async_graphql::{Context, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;
use tokio::sync::OnceCell;

use crate::service::db::Db;
use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::users_table::User;
use crate::service::read_service::users::ensure_user;

/// Request-local authenticated principal. A single GraphQL document can execute
/// many root resolvers, but resolving the Clerk subject must hit Postgres once.
#[derive(Default)]
pub(crate) struct RequestUser(OnceCell<AuthenticatedUser>);

struct AuthenticatedUser {
    user: User,
    db: UserDb,
}

async fn load_user(ctx: &Context<'_>) -> Result<AuthenticatedUser> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let db = ctx.data::<Arc<Db>>()?;
    let full_name = jwt
        .other
        .get("full_name")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let email = jwt
        .other
        .get("email")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let user = ensure_user(db.pool(), &jwt.sub, full_name, email).await?;

    let user_db = db.get_user_db(&user.id);
    Ok(AuthenticatedUser { user, db: user_db })
}

async fn authenticated_user(ctx: &Context<'_>) -> Result<AuthenticatedUser> {
    if let Ok(request_user) = ctx.data::<RequestUser>() {
        let user = request_user.0.get_or_try_init(|| load_user(ctx)).await?;
        return Ok(AuthenticatedUser {
            user: user.user.clone(),
            db: user.db.clone(),
        });
    }

    load_user(ctx).await
}

pub(crate) async fn user_db(ctx: &Context<'_>) -> Result<UserDb> {
    Ok(authenticated_user(ctx).await?.db)
}

pub(crate) async fn current_user(ctx: &Context<'_>) -> Result<User> {
    Ok(authenticated_user(ctx).await?.user)
}

pub(crate) async fn resolve_user(ctx: &Context<'_>) -> Result<(Arc<Db>, String)> {
    let db = ctx.data::<Arc<Db>>()?.clone();
    let user = user_db(ctx).await?;
    Ok((db, user.user_id().to_string()))
}
