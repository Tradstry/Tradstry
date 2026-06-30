use anyhow::{Context, Result};
use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct User {
    pub id: String,
    pub clerk_uuid: String,
    pub full_name: String,
    pub email: String,
    pub created_at: String,
    pub updated_at: String,
}

const SELECT_COLS: &str = "id, clerk_uuid, full_name, email, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

fn row_to_user(row: &sqlx::postgres::PgRow) -> Result<User> {
    Ok(User {
        id: row.try_get::<String, _>(0)?,
        clerk_uuid: row.try_get::<String, _>(1)?,
        full_name: row.try_get::<String, _>(2)?,
        email: row.try_get::<String, _>(3)?,
        created_at: row.try_get::<String, _>(4)?,
        updated_at: row.try_get::<String, _>(5)?,
    })
}

pub async fn find_by_clerk_uuid(pool: &PgPool, clerk_uuid: &str) -> Result<Option<User>> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {SELECT_COLS} FROM users WHERE clerk_uuid = $1"
    )))
    .bind(clerk_uuid)
    .fetch_optional(pool)
    .await
    .context("Failed to query user by clerk_uuid")?;

    match row {
        Some(row) => Ok(Some(row_to_user(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_user(
    pool: &PgPool,
    clerk_uuid: &str,
    full_name: &str,
    email: &str,
) -> Result<User> {
    let id = Uuid::new_v4().to_string();

    sqlx::query("INSERT INTO users (id, clerk_uuid, full_name, email) VALUES ($1, $2, $3, $4)")
        .bind(id.as_str())
        .bind(clerk_uuid)
        .bind(full_name)
        .bind(email)
        .execute(pool)
        .await
        .context("Failed to insert user")?;

    find_by_clerk_uuid(pool, clerk_uuid)
        .await?
        .context("User not found after insert")
}

pub async fn find_or_create_user(
    pool: &PgPool,
    clerk_uuid: &str,
    full_name: &str,
    email: &str,
) -> Result<(User, bool)> {
    if let Some(user) = find_by_clerk_uuid(pool, clerk_uuid).await? {
        return Ok((user, false));
    }

    match create_user(pool, clerk_uuid, full_name, email).await {
        Ok(user) => Ok((user, true)),
        Err(_) => {
            // Race condition: another request created the user first. Fetch it.
            find_by_clerk_uuid(pool, clerk_uuid)
                .await?
                .context("User not found after concurrent insert")
                .map(|user| (user, false))
        }
    }
}
