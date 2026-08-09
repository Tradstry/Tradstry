use crate::service::db::schema::tables::users_table::User;
use async_graphql::{Context, Object, Result};

#[derive(Default)]
pub struct UserQuery;

#[Object]
impl UserQuery {
    /// Returns the current authenticated user.
    /// Creates the user record and a default account on first access.
    async fn me(&self, ctx: &Context<'_>) -> Result<User> {
        crate::graphql::auth::current_user(ctx).await
    }
}
