use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use log::{error, info};
use sqlx::PgPool;
use std::sync::Arc;

use super::schedule::{ScheduleKind, due};
use super::settings::{self, UserSettings};
use super::{NotificationEvent, metrics, outbox};
use crate::service::db::Db;

const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);
const ACTIVE_WINDOW_DAYS: i64 = 30;

fn dry_run() -> bool {
    std::env::var("SCHEDULED_FEEDBACK_DRY_RUN").is_ok_and(|v| v == "1")
}

/// Users worth ticking: those who could actually receive the result. Scanning
/// every row forever would grow the tick cost with total signups rather than
/// with active use.
///
/// Recent *fills* count, not just recent journal entries. A trader who has
/// traded but never journaled is the one the recap exists for; gating on journal
/// history would lock them out of the prompt that would start the habit.
async fn candidate_users(pool: &PgPool, now: DateTime<Utc>) -> Result<Vec<String>> {
    let since = now - Duration::days(ACTIVE_WINDOW_DAYS);
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT u.id FROM users u WHERE \
             EXISTS (SELECT 1 FROM push_subscriptions s WHERE s.user_id = u.id) \
          OR EXISTS (SELECT 1 FROM journal_entries j \
                     WHERE j.user_id = u.id AND j.close_date >= $1) \
          OR EXISTS (SELECT 1 FROM brokerage_transactions t \
                     WHERE t.user_id = u.id AND t.trade_date >= $1)",
    )
    .bind(since)
    .fetch_all(pool)
    .await
    .context("failed to list candidate users")?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// The account a scheduled notification deep-links to. Scheduled events are
/// per-user, but the notification schema carries an account, so the oldest one
/// stands in as the user's primary.
async fn primary_account(pool: &PgPool, user_id: &str) -> Result<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM accounts WHERE user_id = $1 ORDER BY created_at LIMIT 1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .context("failed to read primary account")?;
    Ok(row.map(|r| r.0))
}

fn local_day_bounds(
    settings: &UserSettings,
    date: NaiveDate,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let tz = settings.tz();
    let start = date.and_hms_opt(0, 0, 0)?;
    let end = date.succ_opt()?.and_hms_opt(0, 0, 0)?;
    let to_utc = |naive| {
        tz.from_local_datetime(&naive)
            .earliest()
            .or_else(|| tz.from_local_datetime(&naive).latest())
            .map(|dt| dt.with_timezone(&Utc))
    };
    Some((to_utc(start)?, to_utc(end)?))
}

async fn build_recap(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    settings: &UserSettings,
    local_date: NaiveDate,
) -> Result<Option<NotificationEvent>> {
    let Some((start, end)) = local_day_bounds(settings, local_date) else {
        return Ok(None);
    };
    let symbol_count = metrics::symbols_to_journal(pool, user_id, start, end).await?;

    // Silence is the point. A prompt that fires with nothing to do trains the
    // user to dismiss it unread, which costs the weekly review its audience too.
    if symbol_count == 0 {
        return Ok(None);
    }

    Ok(Some(NotificationEvent::DailyRecap {
        account_id: account_id.to_string(),
        local_date,
        symbol_count,
    }))
}

async fn build_review(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    settings: &UserSettings,
    local_date: NaiveDate,
) -> Result<Option<NotificationEvent>> {
    let Some((day_start, _)) = local_day_bounds(settings, local_date) else {
        return Ok(None);
    };
    let week_start = day_start - Duration::days(7);
    let since = day_start - Duration::days(metrics::DISPOSITION_WINDOW_DAYS);

    let stats = metrics::WeeklyStats {
        counts: Some(metrics::weekly_counts(pool, user_id, week_start, day_start).await?),
        asymmetry: metrics::holding_asymmetry(pool, user_id, since).await?,
        setups: metrics::setup_progress(pool, user_id).await?,
    };

    if stats.is_empty() {
        return Ok(None);
    }

    Ok(Some(NotificationEvent::WeeklyReview {
        account_id: account_id.to_string(),
        iso_week: format!(
            "{}-W{:02}",
            local_date.iso_week().year(),
            local_date.iso_week().week()
        ),
        stats,
    }))
}

/// Claims the slot and records the event in one transaction. The claim is what
/// makes a restart or a late tick safe: only the insert that actually creates
/// the row goes on to produce a notification.
async fn fire(
    pool: &PgPool,
    user_id: &str,
    kind: ScheduleKind,
    local_date: NaiveDate,
    event: NotificationEvent,
) -> Result<bool> {
    let mut tx = pool.begin().await?;

    let claimed = sqlx::query(
        "INSERT INTO notification_schedule_runs (user_id, kind, local_date) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(kind.as_str())
    .bind(local_date)
    .execute(&mut *tx)
    .await
    .context("failed to claim schedule slot")?
    .rows_affected();

    if claimed == 0 {
        tx.rollback().await.ok();
        return Ok(false);
    }

    outbox::record(&mut *tx, user_id, &event, local_date).await?;
    tx.commit().await?;
    Ok(true)
}

async fn handle_user(pool: &PgPool, user_id: &str, now: DateTime<Utc>) -> Result<()> {
    let settings = settings::get(pool, user_id).await?;

    for kind in [ScheduleKind::DailyRecap, ScheduleKind::WeeklyReview] {
        let Some(local_date) = due(now, &settings, kind) else {
            continue;
        };
        let Some(account_id) = primary_account(pool, user_id).await? else {
            continue;
        };

        let event = match kind {
            ScheduleKind::DailyRecap => {
                build_recap(pool, user_id, &account_id, &settings, local_date).await?
            }
            ScheduleKind::WeeklyReview => {
                build_review(pool, user_id, &account_id, &settings, local_date).await?
            }
        };

        let Some(event) = event else { continue };

        if dry_run() {
            info!(
                "[notifications] dry run: would send {} to user={user_id} for {local_date}",
                kind.as_str()
            );
            continue;
        }

        if fire(pool, user_id, kind, local_date, event).await? {
            info!(
                "[notifications] scheduled {} for user={user_id} on {local_date}",
                kind.as_str()
            );
        }
    }

    Ok(())
}

pub async fn process_once(pool: &PgPool, now: DateTime<Utc>) -> Result<usize> {
    let users = candidate_users(pool, now).await?;
    let mut handled = 0;
    for user_id in &users {
        match handle_user(pool, user_id, now).await {
            Ok(()) => handled += 1,
            Err(e) => error!("[notifications] schedule tick failed for user={user_id}: {e:#}"),
        }
    }
    Ok(handled)
}

pub async fn run_schedule_worker(db: Arc<Db>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    info!("[notifications] schedule worker started");
    loop {
        tokio::select! {
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
            _ = shutdown.changed() => {}
        }
        if *shutdown.borrow() {
            info!("[notifications] shutdown requested; exiting schedule worker");
            return;
        }

        if let Err(e) = process_once(db.pool(), Utc::now()).await {
            error!("[notifications] schedule tick failed: {e:#}");
        }
    }
}
