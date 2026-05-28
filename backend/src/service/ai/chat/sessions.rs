use anyhow::{Context, Result};
use libsql::Connection;
use uuid::Uuid;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ChatSession {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

const SELECT_COLS: &str = "id, user_id, account_id, title, created_at, updated_at";

fn nullable_text(row: &libsql::Row, index: i32) -> Option<String> {
    row.get::<libsql::Value>(index)
        .ok()
        .and_then(|value| match value {
            libsql::Value::Text(text) => Some(text),
            _ => None,
        })
}

fn row_to_session(row: &libsql::Row) -> Result<ChatSession> {
    Ok(ChatSession {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        account_id: row.get::<String>(2)?,
        title: nullable_text(row, 3),
        created_at: row.get::<String>(4)?,
        updated_at: row.get::<String>(5)?,
    })
}

async fn find_session_by_id(conn: &Connection, session_id: &str) -> Result<Option<ChatSession>> {
    let mut rows = conn
        .query(
            &format!("SELECT {SELECT_COLS} FROM chat_sessions WHERE id = ?1"),
            libsql::params![session_id],
        )
        .await
        .context("Failed to query chat session")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_session(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_session(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
) -> Result<ChatSession> {
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO chat_sessions (id, user_id, account_id) VALUES (?1, ?2, ?3)",
        libsql::params![id.as_str(), user_id, account_id],
    )
    .await
    .context("Failed to insert chat session")?;

    find_session_by_id(conn, &id)
        .await?
        .context("Chat session not found after insert")
}

pub async fn get_session(conn: &Connection, session_id: &str) -> Result<ChatSession> {
    find_session_by_id(conn, session_id)
        .await?
        .with_context(|| format!("Chat session '{session_id}' not found"))
}

pub async fn list_sessions(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
    limit: i64,
) -> Result<Vec<ChatSession>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {SELECT_COLS} FROM chat_sessions \
                 WHERE user_id = ?1 AND account_id = ?2 \
                 ORDER BY updated_at DESC \
                 LIMIT ?3"
            ),
            libsql::params![user_id, account_id, limit],
        )
        .await
        .context("Failed to list chat sessions")?;

    let mut sessions = Vec::new();
    while let Some(row) = rows.next().await? {
        sessions.push(row_to_session(&row)?);
    }

    Ok(sessions)
}

pub async fn update_session_title(
    conn: &Connection,
    session_id: &str,
    title: &str,
) -> Result<ChatSession> {
    conn.execute(
        "UPDATE chat_sessions SET title = ?1, updated_at = datetime('now') WHERE id = ?2",
        libsql::params![title, session_id],
    )
    .await
    .context("Failed to update chat session title")?;

    find_session_by_id(conn, session_id)
        .await?
        .with_context(|| format!("Chat session '{session_id}' not found after update"))
}

pub async fn touch_session_updated_at(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE chat_sessions SET updated_at = datetime('now') WHERE id = ?1",
        libsql::params![session_id],
    )
    .await
    .context("Failed to touch session updated_at")?;
    Ok(())
}

pub async fn delete_session(conn: &Connection, session_id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM chat_sessions WHERE id = ?1 AND user_id = ?2",
            libsql::params![session_id, user_id],
        )
        .await
        .context("Failed to delete chat session")?;

    Ok(rows_affected > 0)
}
