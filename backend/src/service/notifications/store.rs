use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgConnection, PgPool, Row};
use std::time::Duration;
use uuid::Uuid;

use super::render::Rendered;

#[derive(Debug, Clone)]
pub struct UpsertResult {
    pub id: String,
    pub group_count: i64,
    pub created: bool,
}

#[derive(Debug, Clone)]
pub struct FeedRow {
    pub id: String,
    pub event_type: String,
    pub title: String,
    pub body: String,
    pub deep_link: Option<String>,
    pub payload: Value,
    pub group_count: i64,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Inserts a notification, or folds the event into the existing unread group for
/// this key. The conflict target must repeat the partial index's predicate, and
/// `xmax = 0` is how Postgres reports whether the row was inserted or updated.
///
/// Copy is applied afterwards by `apply_copy`, because the title depends on the
/// group count this statement returns.
pub async fn upsert_coalesced(
    conn: &mut PgConnection,
    user_id: &str,
    event_type: &str,
    coalesce_key: Option<&str>,
    payload: &Value,
) -> Result<UpsertResult> {
    let row = sqlx::query(
        "INSERT INTO notifications (id, user_id, event_type, title, body, payload, coalesce_key) \
         VALUES ($1, $2, $3, '', '', $4, $5) \
         ON CONFLICT (user_id, coalesce_key) WHERE read_at IS NULL AND coalesce_key IS NOT NULL \
         DO UPDATE SET group_count = notifications.group_count + 1, \
                       payload = EXCLUDED.payload, \
                       updated_at = now() \
         RETURNING id, group_count, (xmax = 0) AS created",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(event_type)
    .bind(payload)
    .bind(coalesce_key)
    .fetch_one(conn)
    .await
    .context("failed to upsert notification")?;

    Ok(UpsertResult {
        id: row.try_get("id")?,
        group_count: row.try_get::<i32, _>("group_count")? as i64,
        created: row.try_get("created")?,
    })
}

pub async fn apply_copy(conn: &mut PgConnection, id: &str, rendered: &Rendered) -> Result<()> {
    sqlx::query("UPDATE notifications SET title = $2, body = $3, deep_link = $4 WHERE id = $1")
        .bind(id)
        .bind(&rendered.title)
        .bind(&rendered.body)
        .bind(rendered.deep_link.as_deref())
        .execute(conn)
        .await
        .context("failed to apply notification copy")?;
    Ok(())
}

/// Claims the right to push this notification, stamping `last_pushed_at` in the
/// same statement that tests it so two workers cannot both decide to push.
pub async fn should_push(conn: &mut PgConnection, id: &str, throttle: Duration) -> Result<bool> {
    let updated = sqlx::query(
        "UPDATE notifications SET last_pushed_at = now() \
         WHERE id = $1 \
           AND (last_pushed_at IS NULL \
                OR last_pushed_at < now() - make_interval(secs => $2))",
    )
    .bind(id)
    .bind(throttle.as_secs() as f64)
    .execute(conn)
    .await
    .context("failed to evaluate push throttle")?;
    Ok(updated.rows_affected() == 1)
}

pub async fn feed(
    pool: &PgPool,
    user_id: &str,
    limit: i64,
    before: Option<DateTime<Utc>>,
) -> Result<Vec<FeedRow>> {
    let rows = sqlx::query(
        "SELECT id, event_type, title, body, deep_link, payload, group_count, \
                read_at, created_at, updated_at \
         FROM notifications \
         WHERE user_id = $1 AND ($2::timestamptz IS NULL OR updated_at < $2) \
         ORDER BY updated_at DESC \
         LIMIT $3",
    )
    .bind(user_id)
    .bind(before)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("failed to read notification feed")?;

    rows.into_iter()
        .map(|r| {
            Ok(FeedRow {
                id: r.try_get("id")?,
                event_type: r.try_get("event_type")?,
                title: r.try_get("title")?,
                body: r.try_get("body")?,
                deep_link: r.try_get("deep_link")?,
                payload: r.try_get("payload")?,
                group_count: r.try_get::<i32, _>("group_count")? as i64,
                read_at: r.try_get("read_at")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            })
        })
        .collect()
}

pub async fn unread_count(pool: &PgPool, user_id: &str) -> Result<i64> {
    let row: (i64,) =
        sqlx::query_as("SELECT count(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL")
            .bind(user_id)
            .fetch_one(pool)
            .await
            .context("failed to count unread notifications")?;
    Ok(row.0)
}

/// Returns false when the notification does not exist or belongs to someone else,
/// which is the ownership check — there is no separate lookup to forget.
pub async fn mark_read(pool: &PgPool, user_id: &str, id: &str) -> Result<bool> {
    let updated = sqlx::query(
        "UPDATE notifications SET read_at = now() \
         WHERE id = $1 AND user_id = $2 AND read_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("failed to mark notification read")?;
    Ok(updated.rows_affected() == 1)
}

pub async fn mark_all_read(pool: &PgPool, user_id: &str) -> Result<u64> {
    let updated = sqlx::query(
        "UPDATE notifications SET read_at = now() WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .context("failed to mark all notifications read")?;
    Ok(updated.rows_affected())
}
