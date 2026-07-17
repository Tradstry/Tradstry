//! Sync-protocol table access. The delta reads here deliberately DO NOT filter
//! `deleted_at IS NULL`: a client that never sees a tombstone cannot distinguish
//! "deleted on the server" from "created locally and not yet pushed". This is the
//! exact opposite of the guarded reads elsewhere in the notebook tables, and it
//! is intentional.

use anyhow::{Context, Result};
use sqlx::{PgConnection, PgPool, Row};

// Microsecond precision on updated_at is load-bearing: it is the sync cursor,
// so a truncated stamp would re-emit or skip rows across pulls.
const NOTE_DELTA_COLS: &str = "id, folder_id, title, document_json, sort_order, hlc, \
    to_char(deleted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS deleted_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS updated_at";

const FOLDER_DELTA_COLS: &str = "id, parent_folder_id, name, sort_order, is_system, hlc, \
    to_char(deleted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS deleted_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS updated_at";

#[derive(Debug, Clone)]
pub struct NotebookNoteDelta {
    pub id: String,
    pub folder_id: Option<String>,
    pub title: String,
    pub document_json: String,
    pub sort_order: i64,
    pub trade_ids: Vec<String>,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NotebookFolderDelta {
    pub id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub is_system: bool,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

pub async fn last_mutation_id(
    tx: &mut PgConnection,
    client_id: &str,
    user_id: &str,
) -> Result<i64> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT last_mutation_id FROM notebook_client_mutations
         WHERE client_id = $1 AND user_id = $2",
    )
    .bind(client_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .context("Failed to read last_mutation_id")?;

    Ok(row.map(|r| r.0).unwrap_or(0))
}

/// Monotonic: a lower id is ignored via `GREATEST`, so a retried push batch can
/// never rewind the watermark.
pub async fn advance_mutation_id(
    tx: &mut PgConnection,
    client_id: &str,
    user_id: &str,
    id: i64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO notebook_client_mutations (client_id, user_id, last_mutation_id)
         VALUES ($1, $2, $3)
         ON CONFLICT (client_id) DO UPDATE
         SET last_mutation_id = GREATEST(notebook_client_mutations.last_mutation_id, EXCLUDED.last_mutation_id),
             updated_at = now()",
    )
    .bind(client_id)
    .bind(user_id)
    .bind(id)
    .execute(&mut *tx)
    .await
    .context("Failed to advance last_mutation_id")?;

    Ok(())
}

/// The watermark for ONE client. It must never be a user-wide `MAX`: with two
/// devices, device A at mutation 50 and device B at 90, a user-wide max tells A
/// that 90 is applied. A client that trusts a pull's ack would then truncate its
/// outbox and discard mutations 51..90, which the server never saw.
pub async fn last_mutation_id_for_client(
    pool: &PgPool,
    client_id: &str,
    user_id: &str,
) -> Result<i64> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT last_mutation_id FROM notebook_client_mutations
         WHERE client_id = $1 AND user_id = $2",
    )
    .bind(client_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to read last_mutation_id for client")?;
    Ok(row.map(|r| r.0).unwrap_or(0))
}

/// `>=`, not `>`: a strict cursor permanently skips a row committed at the same
/// microsecond as one already delivered (concurrent tx, invisible at pull time).
/// Re-sending the boundary row is harmless — the client's merge is idempotent.
pub async fn notes_since(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    cookie: Option<&str>,
) -> Result<Vec<NotebookNoteDelta>> {
    let cookie = cookie.filter(|c| !c.is_empty());
    let sql = format!(
        "SELECT {NOTE_DELTA_COLS} FROM notebook_notes
         WHERE user_id = $1 AND account_id = $2
           AND ($3::text IS NULL OR updated_at >= $3::timestamptz)
         ORDER BY updated_at ASC"
    );

    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .bind(cookie)
        .fetch_all(pool)
        .await
        .context("Failed to read note deltas")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let id: String = row.try_get("id")?;
        let trade_ids = super::notes::list_trade_ids_for_note(pool, &id, user_id).await?;
        out.push(NotebookNoteDelta {
            id,
            folder_id: row.try_get("folder_id")?,
            title: row.try_get("title")?,
            document_json: row.try_get("document_json")?,
            sort_order: row.try_get("sort_order")?,
            trade_ids,
            hlc: row.try_get("hlc")?,
            deleted_at: row.try_get("deleted_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(out)
}

pub async fn folders_since(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    cookie: Option<&str>,
) -> Result<Vec<NotebookFolderDelta>> {
    let cookie = cookie.filter(|c| !c.is_empty());
    let sql = format!(
        "SELECT {FOLDER_DELTA_COLS} FROM notebook_folders
         WHERE user_id = $1 AND account_id = $2
           AND ($3::text IS NULL OR updated_at >= $3::timestamptz)
         ORDER BY updated_at ASC"
    );

    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .bind(cookie)
        .fetch_all(pool)
        .await
        .context("Failed to read folder deltas")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(NotebookFolderDelta {
            id: row.try_get("id")?,
            parent_folder_id: row.try_get("parent_folder_id")?,
            name: row.try_get("name")?,
            sort_order: row.try_get("sort_order")?,
            is_system: row.try_get("is_system")?,
            hlc: row.try_get("hlc")?,
            deleted_at: row.try_get("deleted_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(out)
}
