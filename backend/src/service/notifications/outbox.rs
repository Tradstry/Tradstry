use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde_json::Value;
use sqlx::{PgConnection, PgPool, Row};

use super::NotificationEvent;

/// A row that fails this many times is retired rather than blocking forever.
const MAX_ATTEMPTS: i32 = 5;

#[derive(Debug, Clone)]
pub struct OutboxRow {
    pub id: i64,
    pub user_id: String,
    pub event_type: String,
    pub payload: Value,
    pub coalesce_key: Option<String>,
    pub attempts: i32,
}

/// The only function a producer calls. Executor-generic so a producer holding a
/// transaction gets the event committed with its cause, while one holding only a
/// pool still works.
pub async fn record<'e, E>(
    executor: E,
    user_id: &str,
    event: &NotificationEvent,
    today: NaiveDate,
) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        "INSERT INTO notification_outbox (user_id, event_type, payload, coalesce_key) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(event.event_type())
    .bind(event.payload())
    .bind(event.coalesce_key(today))
    .execute(executor)
    .await
    .context("failed to record notification event")?;
    Ok(())
}

/// Claims pending rows for this worker tick. `SKIP LOCKED` so a second process
/// takes different rows rather than blocking on ours.
pub async fn claim_pending(conn: &mut PgConnection, limit: i64) -> Result<Vec<OutboxRow>> {
    let rows = sqlx::query(
        "SELECT id, user_id, event_type, payload, coalesce_key, attempts \
         FROM notification_outbox \
         WHERE processed_at IS NULL \
         ORDER BY id \
         LIMIT $1 \
         FOR UPDATE SKIP LOCKED",
    )
    .bind(limit)
    .fetch_all(conn)
    .await
    .context("failed to claim outbox rows")?;

    rows.into_iter()
        .map(|r| {
            Ok(OutboxRow {
                id: r.try_get("id")?,
                user_id: r.try_get("user_id")?,
                event_type: r.try_get("event_type")?,
                payload: r.try_get("payload")?,
                coalesce_key: r.try_get("coalesce_key")?,
                attempts: r.try_get("attempts")?,
            })
        })
        .collect()
}

pub async fn mark_processed<'e, E>(executor: E, id: i64) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query("UPDATE notification_outbox SET processed_at = now() WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await
        .context("failed to mark outbox row processed")?;
    Ok(())
}

/// Records a failure and retires the row once it has burned through its attempts,
/// so one unrenderable event cannot stall the queue behind it.
pub async fn mark_failed(pool: &PgPool, id: i64, error: &str) -> Result<()> {
    sqlx::query(
        "UPDATE notification_outbox \
         SET attempts = attempts + 1, \
             last_error = $2, \
             processed_at = CASE WHEN attempts + 1 >= $3 THEN now() ELSE processed_at END \
         WHERE id = $1",
    )
    .bind(id)
    .bind(error)
    .bind(MAX_ATTEMPTS)
    .execute(pool)
    .await
    .context("failed to record outbox failure")?;
    Ok(())
}
