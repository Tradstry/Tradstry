//! Plan state, plan limits, and usage counters.
//!
//! Limits are read from the `plan_limits` table rather than hardcoded, so
//! changing a tier is an UPDATE and not a deploy. `None` on a limit means
//! unlimited.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};

/// Subscription + usage state for one user, read off the `users` row.
#[derive(Debug, Clone)]
pub struct PlanState {
    pub plan: String,
    pub paddle_customer_id: Option<String>,
    pub paddle_subscription_id: Option<String>,
    pub subscription_status: Option<String>,
    /// `occurred_at` of the last applied Paddle event (out-of-order guard).
    pub subscription_updated_at: Option<DateTime<Utc>>,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub grace_until: Option<DateTime<Utc>>,
    pub quota_anchor_day: Option<i16>,
    pub data_bytes_used: i64,
    pub media_bytes_used: i64,
    /// Signup time — the anchor the free-tier quota window is derived from.
    pub created_at: DateTime<Utc>,
}

/// One row of the `plan_limits` catalog. `None` means unlimited.
#[derive(Debug, Clone)]
pub struct PlanLimits {
    pub plan: String,
    pub ai_actions_per_month: Option<i32>,
    pub brokerage_connections: Option<i32>,
    pub data_bytes: Option<i64>,
    pub media_bytes: Option<i64>,
}

/// The full subscription state to write after applying a Paddle event.
#[derive(Debug, Clone)]
pub struct SubscriptionUpdate {
    pub plan: String,
    pub paddle_customer_id: Option<String>,
    pub paddle_subscription_id: Option<String>,
    pub subscription_status: String,
    pub subscription_updated_at: DateTime<Utc>,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    /// `Some` only while a past_due grace window is running.
    pub grace_until: Option<DateTime<Utc>>,
}

const PLAN_STATE_COLS: &str = "plan, paddle_customer_id, paddle_subscription_id, \
    subscription_status, subscription_updated_at, current_period_start, current_period_end, \
    grace_until, quota_anchor_day, data_bytes_used, media_bytes_used, created_at";

fn row_to_plan_state(row: &sqlx::postgres::PgRow) -> Result<PlanState> {
    Ok(PlanState {
        plan: row.try_get::<String, _>(0)?,
        paddle_customer_id: row.try_get::<Option<String>, _>(1)?,
        paddle_subscription_id: row.try_get::<Option<String>, _>(2)?,
        subscription_status: row.try_get::<Option<String>, _>(3)?,
        subscription_updated_at: row.try_get::<Option<DateTime<Utc>>, _>(4)?,
        current_period_start: row.try_get::<Option<DateTime<Utc>>, _>(5)?,
        current_period_end: row.try_get::<Option<DateTime<Utc>>, _>(6)?,
        grace_until: row.try_get::<Option<DateTime<Utc>>, _>(7)?,
        quota_anchor_day: row.try_get::<Option<i16>, _>(8)?,
        data_bytes_used: row.try_get::<i64, _>(9).unwrap_or(0),
        media_bytes_used: row.try_get::<i64, _>(10).unwrap_or(0),
        created_at: row.try_get::<DateTime<Utc>, _>(11)?,
    })
}

pub async fn plan_state(pool: &PgPool, user_id: &str) -> Result<Option<PlanState>> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {PLAN_STATE_COLS} FROM users WHERE id = $1"
    )))
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to read plan state")?;

    row.as_ref().map(row_to_plan_state).transpose()
}

pub async fn plan_limits(pool: &PgPool, plan: &str) -> Result<Option<PlanLimits>> {
    let row = sqlx::query(
        "SELECT plan, ai_actions_per_month, brokerage_connections, data_bytes, media_bytes \
         FROM plan_limits WHERE plan = $1",
    )
    .bind(plan)
    .fetch_optional(pool)
    .await
    .context("Failed to read plan limits")?;

    Ok(match row {
        Some(row) => Some(PlanLimits {
            plan: row.try_get::<String, _>(0)?,
            ai_actions_per_month: row.try_get::<Option<i32>, _>(1)?,
            brokerage_connections: row.try_get::<Option<i32>, _>(2)?,
            data_bytes: row.try_get::<Option<i64>, _>(3)?,
            media_bytes: row.try_get::<Option<i64>, _>(4)?,
        }),
        None => None,
    })
}

/// Resolve the user a Paddle event belongs to when `custom_data.user_id` is
/// absent — falls back to the stored customer id.
pub async fn find_user_by_paddle_customer(
    pool: &PgPool,
    paddle_customer_id: &str,
) -> Result<Option<String>> {
    let row = sqlx::query("SELECT id FROM users WHERE paddle_customer_id = $1")
        .bind(paddle_customer_id)
        .fetch_optional(pool)
        .await
        .context("Failed to find user by paddle customer id")?;

    Ok(match row {
        Some(row) => Some(row.try_get::<String, _>(0)?),
        None => None,
    })
}

/// Write the whole subscription state in one statement. Called only by the
/// webhook worker, after it has decided the event is not stale.
pub async fn apply_subscription(
    pool: &PgPool,
    user_id: &str,
    update: &SubscriptionUpdate,
) -> Result<()> {
    sqlx::query(
        "UPDATE users SET \
            plan = $2, \
            paddle_customer_id = COALESCE($3, paddle_customer_id), \
            paddle_subscription_id = COALESCE($4, paddle_subscription_id), \
            subscription_status = $5, \
            subscription_updated_at = $6, \
            current_period_start = $7, \
            current_period_end = $8, \
            grace_until = $9 \
         WHERE id = $1",
    )
    .bind(user_id)
    .bind(update.plan.as_str())
    .bind(update.paddle_customer_id.as_deref())
    .bind(update.paddle_subscription_id.as_deref())
    .bind(update.subscription_status.as_str())
    .bind(update.subscription_updated_at)
    .bind(update.current_period_start)
    .bind(update.current_period_end)
    .bind(update.grace_until)
    .execute(pool)
    .await
    .context("Failed to apply subscription update")?;

    Ok(())
}

pub async fn set_usage_bytes(
    pool: &PgPool,
    user_id: &str,
    data_bytes: i64,
    media_bytes: i64,
) -> Result<()> {
    sqlx::query(
        "UPDATE users SET data_bytes_used = $2, media_bytes_used = $3, \
         usage_recomputed_at = now() WHERE id = $1",
    )
    .bind(user_id)
    .bind(data_bytes)
    .bind(media_bytes)
    .execute(pool)
    .await
    .context("Failed to set usage bytes")?;

    Ok(())
}

/// Incremental media accounting for the upload/delete path, so a full recompute
/// isn't needed on every attachment.
pub async fn add_media_bytes(pool: &PgPool, user_id: &str, delta: i64) -> Result<()> {
    sqlx::query(
        "UPDATE users SET media_bytes_used = GREATEST(0, media_bytes_used + $2) WHERE id = $1",
    )
    .bind(user_id)
    .bind(delta)
    .execute(pool)
    .await
    .context("Failed to adjust media bytes")?;

    Ok(())
}

/// Atomically reserve one unit of a metered resource.
///
/// Returns the new usage on success, `None` when the reservation would exceed
/// `limit`. Check-and-increment in one statement is the point: two concurrent
/// requests can never both squeeze past the last unit.
///
/// The `WHERE` guard only applies to the UPDATE branch, so a `limit` of 0 is
/// special-cased — otherwise the first INSERT would create a row with `used = 1`
/// regardless.
pub async fn reserve_counter(
    pool: &PgPool,
    user_id: &str,
    metric: &str,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    limit: i32,
) -> Result<Option<i32>> {
    if limit <= 0 {
        return Ok(None);
    }

    let row = sqlx::query(
        "INSERT INTO usage_counters (user_id, metric, period_start, period_end, used) \
         VALUES ($1, $2, $3, $4, 1) \
         ON CONFLICT (user_id, metric, period_start) \
         DO UPDATE SET used = usage_counters.used + 1 \
         WHERE usage_counters.used < $5 \
         RETURNING used",
    )
    .bind(user_id)
    .bind(metric)
    .bind(period_start)
    .bind(period_end)
    .bind(limit)
    .fetch_optional(pool)
    .await
    .context("Failed to reserve usage counter")?;

    Ok(match row {
        Some(row) => Some(row.try_get::<i32, _>(0)?),
        None => None,
    })
}

/// Read-only usage for the current window (0 when the window has no row yet).
pub async fn counter_used(
    pool: &PgPool,
    user_id: &str,
    metric: &str,
    period_start: DateTime<Utc>,
) -> Result<i32> {
    let row = sqlx::query(
        "SELECT used FROM usage_counters \
         WHERE user_id = $1 AND metric = $2 AND period_start = $3",
    )
    .bind(user_id)
    .bind(metric)
    .bind(period_start)
    .fetch_optional(pool)
    .await
    .context("Failed to read usage counter")?;

    Ok(match row {
        Some(row) => row.try_get::<i32, _>(0)?,
        None => 0,
    })
}

// ── paddle webhook queue ────────────────────────────────────────────────────

/// One stored webhook awaiting processing.
#[derive(Debug, Clone)]
pub struct WebhookEvent {
    pub event_id: String,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
    pub payload: serde_json::Value,
}

/// Persist a delivery. Returns false when this `event_id` was already stored —
/// Paddle retries up to 60 times, and a redelivery must be a no-op.
pub async fn record_webhook_event(
    pool: &PgPool,
    event_id: &str,
    event_type: &str,
    occurred_at: DateTime<Utc>,
    payload: &serde_json::Value,
) -> Result<bool> {
    let result = sqlx::query(
        "INSERT INTO paddle_webhook_events (event_id, event_type, occurred_at, payload) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(event_id)
    .bind(event_type)
    .bind(occurred_at)
    .bind(payload)
    .execute(pool)
    .await
    .context("Failed to record paddle webhook event")?;

    Ok(result.rows_affected() > 0)
}

/// How many times a failing event is retried before it is left alone.
///
/// Some failures are permanent — an unrecognised price, a payload we don't
/// model. Retrying those forever buries the transient failures that would
/// actually succeed on a second attempt.
pub const MAX_WEBHOOK_ATTEMPTS: i32 = 10;

/// The oldest claimable events, oldest first.
///
/// There is deliberately no lease. Applying an event twice writes the same row
/// values, and the worker's out-of-order guard drops anything older than what
/// is stored — so a second replica processing the same event concurrently is a
/// no-op rather than a corruption. Ordering by `occurred_at` also means a burst
/// of events converges on the newest state even if two are applied at once.
///
/// Events past `MAX_WEBHOOK_ATTEMPTS` stay in the table, unprocessed, with
/// their last error — they are just no longer picked up.
pub async fn claim_webhook_events(pool: &PgPool, limit: i64) -> Result<Vec<WebhookEvent>> {
    let rows = sqlx::query(
        "SELECT event_id, event_type, occurred_at, payload FROM paddle_webhook_events \
         WHERE processed_at IS NULL AND attempts < $2 \
         ORDER BY occurred_at \
         LIMIT $1",
    )
    .bind(limit)
    .bind(MAX_WEBHOOK_ATTEMPTS)
    .fetch_all(pool)
    .await
    .context("Failed to claim paddle webhook events")?;

    rows.into_iter()
        .map(|row| {
            Ok(WebhookEvent {
                event_id: row.try_get::<String, _>(0)?,
                event_type: row.try_get::<String, _>(1)?,
                occurred_at: row.try_get::<DateTime<Utc>, _>(2)?,
                payload: row.try_get::<serde_json::Value, _>(3)?,
            })
        })
        .collect()
}

/// Mark an event done. Clears any previous error so a retry that succeeds does
/// not leave a stale failure behind.
pub async fn mark_webhook_processed(pool: &PgPool, event_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE paddle_webhook_events SET processed_at = now(), error = NULL WHERE event_id = $1",
    )
    .bind(event_id)
    .execute(pool)
    .await
    .context("Failed to mark paddle webhook processed")?;

    Ok(())
}

/// Record a failure and leave `processed_at` NULL so the next sweep retries.
///
/// Returns the new attempt count so the caller can say something once when an
/// event is being given up on, rather than on every retry.
pub async fn mark_webhook_failed(pool: &PgPool, event_id: &str, error: &str) -> Result<i32> {
    let row = sqlx::query(
        "UPDATE paddle_webhook_events SET error = $2, attempts = attempts + 1 \
         WHERE event_id = $1 RETURNING attempts",
    )
    .bind(event_id)
    .bind(error)
    .fetch_optional(pool)
    .await
    .context("Failed to record paddle webhook error")?;

    Ok(match row {
        Some(row) => row.try_get::<i32, _>(0)?,
        None => 0,
    })
}

/// Non-disabled brokerage connections, for the connection ceiling.
pub async fn active_connection_count(pool: &PgPool, user_id: &str) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) FROM accounts \
         WHERE user_id = $1 AND snaptrade_connection_id IS NOT NULL \
           AND COALESCE(snaptrade_connection_disabled, false) = false",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Failed to count active brokerage connections")?;

    Ok(row.try_get::<i64, _>(0).unwrap_or(0))
}
