use anyhow::Result;
use libsql::Connection;

use crate::service::turso::schema::tables::accounts_table;
use crate::service::turso::schema::tables::users_table::{self, User};

/// Find or create a user from Clerk JWT claims.
/// On first sign-in, creates the user and a default "Main Portfolio" account.
pub async fn ensure_user(
    conn: &Connection,
    clerk_uuid: &str,
    full_name: &str,
    email: &str,
) -> Result<User> {
    let (user, created) =
        users_table::find_or_create_user(conn, clerk_uuid, full_name, email).await?;

    if created {
        accounts_table::create_default_account(conn, &user.id).await?;
    }

    Ok(user)
}
