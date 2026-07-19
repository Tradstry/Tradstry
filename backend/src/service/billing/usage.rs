//! Measuring what a user's data actually costs.
//!
//! Only the tables that dominate a user's footprint are counted —
//! `brokerage_transactions` (mostly `raw_json`), `notebook_note_updates` (CRDT
//! history), `notebook_note_crdt`, `journal_entries` and `notebook_notes`.
//! Summing every table would be slower and would move the number by percentage
//! points, not multiples.
//!
//! Media is deliberately excluded: it lives in R2 and is metered separately by
//! `media_bytes_used`, incrementally at the upload/delete path.

use anyhow::{Context, Result};
use sqlx::{PgPool, Row};

use crate::service::db::schema::tables::billing_table;

/// Sum of `pg_column_size` across the dominant per-user tables.
///
/// `pg_column_size` is the on-disk size *after* TOAST compression, so a
/// user storing highly compressible JSON is charged what they actually cost.
pub async fn user_bytes(pool: &PgPool, user_id: &str) -> Result<i64> {
    let row = sqlx::query(
        "SELECT \
            COALESCE((SELECT SUM(pg_column_size(t.*))::bigint FROM brokerage_transactions t \
                      WHERE t.user_id = $1), 0) \
          + COALESCE((SELECT SUM(pg_column_size(u.*))::bigint FROM notebook_note_updates u \
                      JOIN notebook_notes n ON n.id = u.note_id WHERE n.user_id = $1), 0) \
          + COALESCE((SELECT SUM(pg_column_size(c.*))::bigint FROM notebook_note_crdt c \
                      JOIN notebook_notes n ON n.id = c.note_id WHERE n.user_id = $1), 0) \
          + COALESCE((SELECT SUM(pg_column_size(j.*))::bigint FROM journal_entries j \
                      WHERE j.user_id = $1), 0) \
          + COALESCE((SELECT SUM(pg_column_size(n.*))::bigint FROM notebook_notes n \
                      WHERE n.user_id = $1), 0)",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Failed to measure user data bytes")?;

    Ok(row.try_get::<i64, _>(0).unwrap_or(0))
}

/// Recompute and store `data_bytes_used`, leaving `media_bytes_used` alone.
pub async fn recompute_user_bytes(pool: &PgPool, user_id: &str) -> Result<i64> {
    let data_bytes = user_bytes(pool, user_id).await?;

    let media_bytes = billing_table::plan_state(pool, user_id)
        .await?
        .map(|state| state.media_bytes_used)
        .unwrap_or(0);

    billing_table::set_usage_bytes(pool, user_id, data_bytes, media_bytes).await?;
    Ok(data_bytes)
}

/// Every user with an account, for the periodic sweep.
pub async fn users_with_accounts(pool: &PgPool) -> Result<Vec<String>> {
    let rows = sqlx::query("SELECT DISTINCT user_id FROM accounts")
        .fetch_all(pool)
        .await
        .context("Failed to list users for usage recompute")?;

    rows.into_iter()
        .map(|row| Ok(row.try_get::<String, _>(0)?))
        .collect()
}
