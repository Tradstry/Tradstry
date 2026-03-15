use anyhow::{anyhow, Context, Result};
use libsql::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatThread {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatMessage {
    pub id: String,
    pub thread_id: String,
    pub user_id: String,
    pub request_id: Option<String>,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

fn row_to_thread(row: &libsql::Row) -> Result<AiChatThread> {
    Ok(AiChatThread {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        title: row.get::<String>(2)?,
        created_at: row.get::<String>(3)?,
        updated_at: row.get::<String>(4)?,
    })
}

fn row_to_message(row: &libsql::Row) -> Result<AiChatMessage> {
    Ok(AiChatMessage {
        id: row.get::<String>(0)?,
        thread_id: row.get::<String>(1)?,
        user_id: row.get::<String>(2)?,
        request_id: row.get::<Option<String>>(3)?,
        role: row.get::<String>(4)?,
        content: row.get::<String>(5)?,
        created_at: row.get::<String>(6)?,
    })
}

fn derive_thread_title(message: &str) -> String {
    let collapsed = message.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim();
    if trimmed.is_empty() {
        return "New chat".to_string();
    }

    let mut title = String::new();
    for ch in trimmed.chars().take(80) {
        title.push(ch);
    }

    if trimmed.chars().count() > 80 {
        title.push_str("...");
    }

    title
}

pub async fn find_thread_by_id(
    conn: &Connection,
    user_id: &str,
    thread_id: &str,
) -> Result<Option<AiChatThread>> {
    let mut rows = conn
        .query(
            "SELECT id, user_id, title, created_at, updated_at FROM ai_chat_threads WHERE id = ?1 AND user_id = ?2",
            libsql::params![thread_id, user_id],
        )
        .await
        .context("Failed to query AI chat thread")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_thread(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_thread(
    conn: &Connection,
    user_id: &str,
    initial_message: &str,
) -> Result<AiChatThread> {
    let id = Uuid::new_v4().to_string();
    let title = derive_thread_title(initial_message);

    conn.execute(
        "INSERT INTO ai_chat_threads (id, user_id, title) VALUES (?1, ?2, ?3)",
        libsql::params![id.as_str(), user_id, title.as_str()],
    )
    .await
    .context("Failed to insert AI chat thread")?;

    find_thread_by_id(conn, user_id, &id)
        .await?
        .context("AI chat thread not found after insert")
}

pub async fn get_or_create_thread(
    conn: &Connection,
    user_id: &str,
    requested_thread_id: Option<&str>,
    initial_message: &str,
) -> Result<AiChatThread> {
    match requested_thread_id {
        Some(thread_id) => find_thread_by_id(conn, user_id, thread_id)
            .await?
            .ok_or_else(|| anyhow!("AI chat thread not found")),
        None => create_thread(conn, user_id, initial_message).await,
    }
}

pub async fn insert_message(
    conn: &Connection,
    thread_id: &str,
    user_id: &str,
    request_id: Option<&str>,
    role: &str,
    content: &str,
) -> Result<AiChatMessage> {
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO ai_chat_messages (id, thread_id, user_id, request_id, role, content) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        libsql::params![id.as_str(), thread_id, user_id, request_id, role, content],
    )
    .await
    .context("Failed to insert AI chat message")?;

    conn.execute(
        "UPDATE ai_chat_threads SET updated_at = datetime('now') WHERE id = ?1 AND user_id = ?2",
        libsql::params![thread_id, user_id],
    )
    .await
    .context("Failed to update AI chat thread timestamp")?;

    let mut rows = conn
        .query(
            "SELECT id, thread_id, user_id, request_id, role, content, created_at FROM ai_chat_messages WHERE id = ?1",
            libsql::params![id.as_str()],
        )
        .await
        .context("Failed to reload AI chat message")?;

    match rows.next().await? {
        Some(row) => Ok(row_to_message(&row)?),
        None => Err(anyhow!("AI chat message not found after insert")),
    }
}

pub async fn delete_thread(conn: &Connection, user_id: &str, thread_id: &str) -> Result<bool> {
    if find_thread_by_id(conn, user_id, thread_id).await?.is_none() {
        return Ok(false);
    }

    conn.execute(
        "DELETE FROM ai_chat_threads WHERE id = ?1 AND user_id = ?2",
        libsql::params![thread_id, user_id],
    )
    .await
    .context("Failed to delete AI chat thread")?;

    Ok(true)
}

pub async fn list_threads(conn: &Connection, user_id: &str) -> Result<Vec<AiChatThread>> {
    let mut rows = conn
        .query(
            "SELECT id, user_id, title, created_at, updated_at FROM ai_chat_threads WHERE user_id = ?1 ORDER BY updated_at DESC, created_at DESC",
            libsql::params![user_id],
        )
        .await
        .context("Failed to list AI chat threads")?;

    let mut threads = Vec::new();
    while let Some(row) = rows.next().await? {
        threads.push(row_to_thread(&row)?);
    }

    Ok(threads)
}

pub async fn list_thread_messages(
    conn: &Connection,
    user_id: &str,
    thread_id: &str,
) -> Result<Vec<AiChatMessage>> {
    if find_thread_by_id(conn, user_id, thread_id).await?.is_none() {
        return Ok(Vec::new());
    }

    let mut rows = conn
        .query(
            "SELECT id, thread_id, user_id, request_id, role, content, created_at FROM ai_chat_messages WHERE user_id = ?1 AND thread_id = ?2 ORDER BY created_at ASC, id ASC",
            libsql::params![user_id, thread_id],
        )
        .await
        .context("Failed to list AI chat messages")?;

    let mut messages = Vec::new();
    while let Some(row) = rows.next().await? {
        messages.push(row_to_message(&row)?);
    }

    Ok(messages)
}
