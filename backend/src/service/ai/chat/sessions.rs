use anyhow::{Context, Result};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgRow};
use sqlx::{PgPool, Row};
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

// Timestamps are surfaced as ISO-8601 UTC strings (e.g. 2026-05-29T12:00:00Z) so
// the frontend's `new Date(...)` parses them consistently across browsers.
const SELECT_COLS: &str = "id, user_id, account_id, title, \
     to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
     to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

/// Chat session metadata, stored in Postgres alongside the LangGraph message
/// checkpoints (joined by session id). Moved off Turso so session reads/writes
/// don't pay remote round-trips and live next to the conversation history.
#[derive(Clone)]
pub struct ChatSessionStore {
    pool: PgPool,
}

impl ChatSessionStore {
    pub fn from_env() -> Result<Self> {
        let url = std::env::var("POSTGRES_URL").context("POSTGRES_URL not set")?;
        let opts: PgConnectOptions = url.parse().context("Failed to parse POSTGRES_URL")?;
        let pool = PgPoolOptions::new()
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    use sqlx::Executor;
                    conn.execute("SET client_min_messages = WARNING").await?;
                    Ok(())
                })
            })
            .connect_lazy_with(opts);
        Ok(Self { pool })
    }

    /// Create the table + supporting index if they don't exist. Called at boot.
    pub async fn ensure_table(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chat_sessions (\
                 id TEXT PRIMARY KEY, \
                 user_id TEXT NOT NULL, \
                 account_id TEXT NOT NULL, \
                 title TEXT, \
                 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), \
                 updated_at TIMESTAMPTZ NOT NULL DEFAULT now()\
             )",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create chat_sessions table")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_chat_sessions_user_account_updated \
             ON chat_sessions (user_id, account_id, updated_at DESC)",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create chat_sessions index")?;
        Ok(())
    }

    fn row_to_session(row: &PgRow) -> ChatSession {
        ChatSession {
            id: row.get("id"),
            user_id: row.get("user_id"),
            account_id: row.get("account_id"),
            title: row.get("title"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    pub async fn create_session(&self, user_id: &str, account_id: &str) -> Result<ChatSession> {
        let id = Uuid::new_v4().to_string();
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO chat_sessions (id, user_id, account_id) \
             VALUES ($1, $2, $3) RETURNING {SELECT_COLS}"
        )))
        .bind(&id)
        .bind(user_id)
        .bind(account_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to insert chat session")?;
        Ok(Self::row_to_session(&row))
    }

    pub async fn get_session(&self, session_id: &str) -> Result<ChatSession> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {SELECT_COLS} FROM chat_sessions WHERE id = $1"
        )))
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query chat session")?
        .with_context(|| format!("Chat session '{session_id}' not found"))?;
        Ok(Self::row_to_session(&row))
    }

    pub async fn list_sessions(
        &self,
        user_id: &str,
        account_id: &str,
        limit: i64,
    ) -> Result<Vec<ChatSession>> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {SELECT_COLS} FROM chat_sessions \
             WHERE user_id = $1 AND account_id = $2 \
             ORDER BY updated_at DESC LIMIT $3"
        )))
        .bind(user_id)
        .bind(account_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list chat sessions")?;
        Ok(rows.iter().map(Self::row_to_session).collect())
    }

    pub async fn update_session_title(&self, session_id: &str, title: &str) -> Result<ChatSession> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE chat_sessions SET title = $1, updated_at = now() \
             WHERE id = $2 RETURNING {SELECT_COLS}"
        )))
        .bind(title)
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to update chat session title")?
        .with_context(|| format!("Chat session '{session_id}' not found after update"))?;
        Ok(Self::row_to_session(&row))
    }

    pub async fn touch_session_updated_at(&self, session_id: &str) -> Result<()> {
        sqlx::query("UPDATE chat_sessions SET updated_at = now() WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .context("Failed to touch session updated_at")?;
        Ok(())
    }

    pub async fn delete_session(&self, session_id: &str, user_id: &str) -> Result<bool> {
        let res = sqlx::query("DELETE FROM chat_sessions WHERE id = $1 AND user_id = $2")
            .bind(session_id)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .context("Failed to delete chat session")?;
        Ok(res.rows_affected() > 0)
    }
}
