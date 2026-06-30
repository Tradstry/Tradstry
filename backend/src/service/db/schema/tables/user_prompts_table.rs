use anyhow::{Context, Result};
use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct UserPrompt {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

const SELECT_COLS: &str = "id, user_id, name, content, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

fn row_to_user_prompt(row: &sqlx::postgres::PgRow) -> Result<UserPrompt> {
    Ok(UserPrompt {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        name: row.try_get::<String, _>(2)?,
        content: row.try_get::<String, _>(3)?,
        created_at: row.try_get::<String, _>(4)?,
        updated_at: row.try_get::<String, _>(5)?,
    })
}

pub async fn list_user_prompts(pool: &PgPool, user_id: &str) -> Result<Vec<UserPrompt>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM user_prompts WHERE user_id = $1 ORDER BY created_at DESC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .fetch_all(pool)
        .await
        .context("Failed to list user prompts")?;

    let mut prompts = Vec::new();
    for row in &rows {
        prompts.push(row_to_user_prompt(row)?);
    }
    Ok(prompts)
}

pub async fn find_user_prompt(
    pool: &PgPool,
    id: &str,
    user_id: &str,
) -> Result<Option<UserPrompt>> {
    let sql = format!("SELECT {SELECT_COLS} FROM user_prompts WHERE id = $1 AND user_id = $2");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("Failed to find user prompt")?;

    match row {
        Some(row) => Ok(Some(row_to_user_prompt(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_user_prompt(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    content: &str,
) -> Result<UserPrompt> {
    let id = Uuid::new_v4().to_string();

    sqlx::query("INSERT INTO user_prompts (id, user_id, name, content) VALUES ($1, $2, $3, $4)")
        .bind(id.as_str())
        .bind(user_id)
        .bind(name)
        .bind(content)
        .execute(pool)
        .await
        .context("Failed to insert user prompt")?;

    find_user_prompt(pool, &id, user_id)
        .await?
        .context("User prompt not found after insert")
}

pub async fn update_user_prompt(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    name: Option<&str>,
    content: Option<&str>,
) -> Result<UserPrompt> {
    let mut sets = Vec::new();
    let mut params: Vec<String> = Vec::new();
    let mut idx = 1;

    if let Some(name) = name {
        sets.push(format!("name = ${idx}"));
        params.push(name.to_string());
        idx += 1;
    }
    if let Some(content) = content {
        sets.push(format!("content = ${idx}"));
        params.push(content.to_string());
        idx += 1;
    }

    if sets.is_empty() {
        return find_user_prompt(pool, id, user_id)
            .await?
            .context("User prompt not found");
    }

    let sql = format!(
        "UPDATE user_prompts SET {} WHERE id = ${} AND user_id = ${}",
        sets.join(", "),
        idx,
        idx + 1
    );

    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for param in &params {
        query = query.bind(param);
    }
    query
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to update user prompt")?;

    find_user_prompt(pool, id, user_id)
        .await?
        .context("User prompt not found after update")
}

pub async fn delete_user_prompt(pool: &PgPool, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = sqlx::query("DELETE FROM user_prompts WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to delete user prompt")?
        .rows_affected();

    Ok(rows_affected > 0)
}
