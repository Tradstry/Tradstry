use anyhow::Result;
use sqlx::PgPool;

use crate::service::db::schema::tables::accounts_table;
use crate::service::db::schema::tables::tags_table;
use crate::service::db::schema::tables::users_table::{self, User};

/// Find or create a user from Clerk JWT claims.
/// On first sign-in, creates the user and a default "Main Portfolio" account.
pub async fn ensure_user(
    pool: &PgPool,
    clerk_uuid: &str,
    full_name: &str,
    email: &str,
) -> Result<User> {
    let (user, created) =
        users_table::find_or_create_user(pool, clerk_uuid, full_name, email).await?;

    // Provisioning only on first sign-in.
    if created {
        accounts_table::create_default_account(pool, &user.id).await?;
        tags_table::ensure_default_categories(pool, &user.id).await?;
    }

    Ok(user)
}
