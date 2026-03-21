use anyhow::{Context, Result};
use libsql::Connection;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub context_json: Option<String>,
    pub tool_name: Option<String>,
    pub created_at: String,
}

const SELECT_COLS: &str =
    "id, session_id, role, content, context_json, tool_name, created_at";

fn nullable_text(row: &libsql::Row, index: i32) -> Option<String> {
    row.get::<libsql::Value>(index)
        .ok()
        .and_then(|value| match value {
            libsql::Value::Text(text) => Some(text),
            _ => None,
        })
}

fn row_to_message(row: &libsql::Row) -> Result<ChatMessage> {
    Ok(ChatMessage {
        id: row.get::<String>(0)?,
        session_id: row.get::<String>(1)?,
        role: row.get::<String>(2)?,
        content: row.get::<String>(3)?,
        context_json: nullable_text(row, 4),
        tool_name: nullable_text(row, 5),
        created_at: row.get::<String>(6)?,
    })
}

async fn find_message_by_id(conn: &Connection, message_id: &str) -> Result<Option<ChatMessage>> {
    let mut rows = conn
        .query(
            &format!("SELECT {SELECT_COLS} FROM chat_messages WHERE id = ?1"),
            libsql::params![message_id],
        )
        .await
        .context("Failed to query chat message")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_message(&row)?)),
        None => Ok(None),
    }
}

pub async fn insert_message(
    conn: &Connection,
    session_id: &str,
    role: &str,
    content: &str,
    context_json: Option<&str>,
    tool_name: Option<&str>,
) -> Result<ChatMessage> {
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO chat_messages (id, session_id, role, content, context_json, tool_name) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        libsql::params![
            id.as_str(),
            session_id,
            role,
            content,
            context_json,
            tool_name,
        ],
    )
    .await
    .context("Failed to insert chat message")?;

    // Touch the session updated_at so list_sessions ordering stays current.
    conn.execute(
        "UPDATE chat_sessions SET updated_at = datetime('now') WHERE id = ?1",
        libsql::params![session_id],
    )
    .await
    .context("Failed to update chat session updated_at")?;

    find_message_by_id(conn, &id)
        .await?
        .context("Chat message not found after insert")
}

pub async fn get_message(conn: &Connection, message_id: &str) -> Result<ChatMessage> {
    find_message_by_id(conn, message_id)
        .await?
        .with_context(|| format!("Chat message '{message_id}' not found"))
}

pub async fn list_messages(
    conn: &Connection,
    session_id: &str,
    limit: i64,
    before: Option<&str>,
) -> Result<Vec<ChatMessage>> {
    let mut messages = if let Some(cursor_id) = before {
        // Look up the cursor message's created_at first.
        let mut cursor_rows = conn
            .query(
                "SELECT created_at FROM chat_messages WHERE id = ?1",
                libsql::params![cursor_id],
            )
            .await
            .context("Failed to look up cursor message")?;

        let cursor_created_at: String = match cursor_rows.next().await? {
            Some(row) => row.get::<String>(0)?,
            None => return Ok(Vec::new()),
        };

        let mut rows = conn
            .query(
                &format!(
                    "SELECT {SELECT_COLS} FROM chat_messages \
                     WHERE session_id = ?1 AND created_at < ?2 \
                     ORDER BY created_at DESC \
                     LIMIT ?3"
                ),
                libsql::params![session_id, cursor_created_at.as_str(), limit],
            )
            .await
            .context("Failed to list chat messages with cursor")?;

        let mut msgs = Vec::new();
        while let Some(row) = rows.next().await? {
            msgs.push(row_to_message(&row)?);
        }
        msgs
    } else {
        let mut rows = conn
            .query(
                &format!(
                    "SELECT {SELECT_COLS} FROM chat_messages \
                     WHERE session_id = ?1 \
                     ORDER BY created_at DESC \
                     LIMIT ?2"
                ),
                libsql::params![session_id, limit],
            )
            .await
            .context("Failed to list chat messages")?;

        let mut msgs = Vec::new();
        while let Some(row) = rows.next().await? {
            msgs.push(row_to_message(&row)?);
        }
        msgs
    };

    // Reverse to return in chronological order.
    messages.reverse();
    Ok(messages)
}

/// Load the last `max_user_turns` user messages and any associated tool/assistant
/// messages that form complete turn chains within the same session.
pub async fn load_recent_turns(
    conn: &Connection,
    session_id: &str,
    max_user_turns: i64,
) -> Result<Vec<ChatMessage>> {
    // Fetch the created_at of the Nth-oldest user message from the tail.
    let mut boundary_rows = conn
        .query(
            "SELECT created_at FROM chat_messages \
             WHERE session_id = ?1 AND role = 'user' \
             ORDER BY created_at DESC \
             LIMIT 1 OFFSET ?2",
            libsql::params![session_id, max_user_turns - 1],
        )
        .await
        .context("Failed to query turn boundary")?;

    let cutoff: Option<String> = match boundary_rows.next().await? {
        Some(row) => Some(row.get::<String>(0)?),
        None => None,
    };

    // Retrieve all messages from the cutoff onward (or all if no cutoff).
    let mut messages = if let Some(cutoff_at) = cutoff {
        let mut rows = conn
            .query(
                &format!(
                    "SELECT {SELECT_COLS} FROM chat_messages \
                     WHERE session_id = ?1 AND created_at >= ?2 \
                     ORDER BY created_at ASC"
                ),
                libsql::params![session_id, cutoff_at.as_str()],
            )
            .await
            .context("Failed to load recent turns")?;

        let mut msgs = Vec::new();
        while let Some(row) = rows.next().await? {
            msgs.push(row_to_message(&row)?);
        }
        msgs
    } else {
        // Fewer user turns than max — return everything in order.
        let mut rows = conn
            .query(
                &format!(
                    "SELECT {SELECT_COLS} FROM chat_messages \
                     WHERE session_id = ?1 \
                     ORDER BY created_at ASC"
                ),
                libsql::params![session_id],
            )
            .await
            .context("Failed to load all session messages")?;

        let mut msgs = Vec::new();
        while let Some(row) = rows.next().await? {
            msgs.push(row_to_message(&row)?);
        }
        msgs
    };

    // Ensure strictly chronological order.
    messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(messages)
}
