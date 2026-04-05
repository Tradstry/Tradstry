use anyhow::{Context, Result};
use async_graphql::SimpleObject;
use libsql::Connection;
use serde::{Deserialize, Serialize};
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

const SELECT_COLS: &str = "id, user_id, name, content, created_at, updated_at";

fn row_to_user_prompt(row: &libsql::Row) -> Result<UserPrompt> {
    Ok(UserPrompt {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        name: row.get::<String>(2)?,
        content: row.get::<String>(3)?,
        created_at: row.get::<String>(4)?,
        updated_at: row.get::<String>(5)?,
    })
}

pub async fn list_user_prompts(
    conn: &Connection,
    user_id: &str,
) -> Result<Vec<UserPrompt>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {SELECT_COLS} FROM user_prompts WHERE user_id = ?1 ORDER BY created_at DESC"
            ),
            libsql::params![user_id],
        )
        .await
        .context("Failed to list user prompts")?;

    let mut prompts = Vec::new();
    while let Some(row) = rows.next().await? {
        prompts.push(row_to_user_prompt(&row)?);
    }
    Ok(prompts)
}

pub async fn find_user_prompt(
    conn: &Connection,
    id: &str,
    user_id: &str,
) -> Result<Option<UserPrompt>> {
    let mut rows = conn
        .query(
            &format!("SELECT {SELECT_COLS} FROM user_prompts WHERE id = ?1 AND user_id = ?2"),
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to find user prompt")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_user_prompt(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_user_prompt(
    conn: &Connection,
    user_id: &str,
    name: &str,
    content: &str,
) -> Result<UserPrompt> {
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO user_prompts (id, user_id, name, content) VALUES (?1, ?2, ?3, ?4)",
        libsql::params![id.as_str(), user_id, name, content],
    )
    .await
    .context("Failed to insert user prompt")?;

    find_user_prompt(conn, &id, user_id)
        .await?
        .context("User prompt not found after insert")
}

pub async fn update_user_prompt(
    conn: &Connection,
    id: &str,
    user_id: &str,
    name: Option<&str>,
    content: Option<&str>,
) -> Result<UserPrompt> {
    let mut sets = Vec::new();
    let mut params: Vec<libsql::Value> = Vec::new();
    let mut idx = 1;

    if let Some(name) = name {
        sets.push(format!("name = ?{idx}"));
        params.push(libsql::Value::Text(name.to_string()));
        idx += 1;
    }
    if let Some(content) = content {
        sets.push(format!("content = ?{idx}"));
        params.push(libsql::Value::Text(content.to_string()));
        idx += 1;
    }

    if sets.is_empty() {
        return find_user_prompt(conn, id, user_id)
            .await?
            .context("User prompt not found");
    }

    params.push(libsql::Value::Text(id.to_string()));
    params.push(libsql::Value::Text(user_id.to_string()));

    let sql = format!(
        "UPDATE user_prompts SET {} WHERE id = ?{} AND user_id = ?{}",
        sets.join(", "),
        idx,
        idx + 1
    );

    conn.execute(&sql, libsql::params_from_iter(params))
        .await
        .context("Failed to update user prompt")?;

    find_user_prompt(conn, id, user_id)
        .await?
        .context("User prompt not found after update")
}

pub async fn delete_user_prompt(conn: &Connection, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM user_prompts WHERE id = ?1 AND user_id = ?2",
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to delete user prompt")?;

    Ok(rows_affected > 0)
}
