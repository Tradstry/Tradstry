//! Applies queued Paddle webhooks to user rows.
//!
//! Split from the HTTP route so Paddle always gets its 200 inside 5 seconds
//! regardless of how slow the database is. An event that fails is left
//! unprocessed with its error recorded, and retried on the next sweep.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use log::{error, info, warn};

use super::{entitlements, paddle};
use crate::service::db::Db;
use crate::service::db::schema::tables::billing_table::{self, WebhookEvent};
use crate::service::redis::client::RedisClient;

/// Plan changes should feel immediate after checkout, so this is far tighter
/// than the other sweepers. It is cheap when the queue is empty.
const SWEEP_INTERVAL_SECS: u64 = 5;

/// Bounds how much one tick can do; the next tick picks up the rest.
const BATCH_SIZE: i64 = 20;

pub async fn run_paddle_webhook_worker(
    db: Arc<Db>,
    redis: Option<Arc<RedisClient>>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    info!("[paddle] webhook worker started");
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(SWEEP_INTERVAL_SECS));

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("[paddle] webhook worker stopped");
                    return;
                }
                continue;
            }
        }

        let events = match billing_table::claim_webhook_events(db.pool(), BATCH_SIZE).await {
            Ok(events) => events,
            Err(e) => {
                error!("[paddle] failed to claim webhook events: {e:#}");
                continue;
            }
        };

        for event in events {
            let event_id = event.event_id.clone();
            match process_event(&db, redis.as_deref(), &event).await {
                Ok(()) => {
                    if let Err(e) =
                        billing_table::mark_webhook_processed(db.pool(), &event_id).await
                    {
                        error!("[paddle] failed to stamp {event_id} processed: {e:#}");
                    }
                }
                Err(e) => {
                    // Left unprocessed on purpose: the next sweep retries it,
                    // up to MAX_WEBHOOK_ATTEMPTS.
                    error!("[paddle] failed to apply {event_id}: {e:#}");
                    match billing_table::mark_webhook_failed(
                        db.pool(),
                        &event_id,
                        &format!("{e:#}"),
                    )
                    .await
                    {
                        // Said once, at the point it stops being retried, so a
                        // permanently-broken event is visible without flooding.
                        Ok(attempts) if attempts >= billing_table::MAX_WEBHOOK_ATTEMPTS => {
                            error!(
                                "[paddle] giving up on {event_id} after {attempts} attempts; \
                                 it stays in paddle_webhook_events for inspection"
                            );
                        }
                        Ok(_) => {}
                        Err(e) => {
                            error!("[paddle] failed to record error for {event_id}: {e:#}");
                        }
                    }
                }
            }
        }
    }
}

async fn process_event(
    db: &Db,
    redis: Option<&RedisClient>,
    event: &WebhookEvent,
) -> anyhow::Result<()> {
    // Events we don't model are acknowledged rather than retried forever — an
    // extra subscription ticked in the Paddle dashboard must not wedge the queue.
    if !paddle::is_subscription_event(&event.event_type) {
        info!(
            "[paddle] ignoring {} ({}), not a subscription event",
            event.event_type, event.event_id
        );
        return Ok(());
    }

    let envelope: paddle::Envelope = serde_json::from_value(event.payload.clone())?;
    let mutation = paddle::apply_event(&envelope)?;

    let Some(user_id) = resolve_user(db, &mutation).await? else {
        // Nothing to apply this to. Acknowledged rather than retried: an unknown
        // customer will still be unknown in five seconds.
        warn!(
            "[paddle] no user for customer {} ({}); dropping",
            mutation.paddle_customer_id, event.event_id
        );
        return Ok(());
    };

    let applied = billing_table::plan_state(db.pool(), &user_id)
        .await?
        .and_then(|state| state.subscription_updated_at);

    if is_stale(applied, envelope.occurred_at) {
        info!(
            "[paddle] {} is older than the applied state for {user_id}; skipping",
            event.event_id
        );
        return Ok(());
    }

    billing_table::apply_subscription(db.pool(), &user_id, &mutation.update).await?;

    // The TTL alone would leave a downgraded user on a paid plan for minutes.
    entitlements::invalidate(redis, &user_id).await;

    info!(
        "[paddle] {user_id} -> {} ({}) via {}",
        mutation.update.plan, mutation.update.subscription_status, event.event_id
    );
    Ok(())
}

/// `custom_data.user_id` is set at checkout and is authoritative. The customer
/// lookup covers subscriptions created outside our checkout (dashboard, imports).
async fn resolve_user(db: &Db, mutation: &paddle::PlanMutation) -> anyhow::Result<Option<String>> {
    if let Some(user_id) = &mutation.user_id
        && billing_table::plan_state(db.pool(), user_id)
            .await?
            .is_some()
    {
        return Ok(Some(user_id.clone()));
    }

    billing_table::find_user_by_paddle_customer(db.pool(), &mutation.paddle_customer_id).await
}

/// Paddle does not guarantee delivery order, so an event older than the one
/// already applied would otherwise resurrect a stale plan.
///
/// Equal timestamps are *not* stale: a redelivery of the newest event writes
/// identical values, and refusing it would make an event that arrived exactly
/// once indistinguishable from one that never arrived.
fn is_stale(applied: Option<DateTime<Utc>>, occurred_at: DateTime<Utc>) -> bool {
    applied.is_some_and(|applied| occurred_at < applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, day, 12, 0, 0).unwrap()
    }

    #[test]
    fn an_older_event_is_stale() {
        assert!(is_stale(Some(at(18)), at(17)));
    }

    #[test]
    fn a_newer_event_applies() {
        assert!(!is_stale(Some(at(18)), at(19)));
    }

    #[test]
    fn a_redelivery_of_the_newest_event_still_applies() {
        assert!(!is_stale(Some(at(18)), at(18)));
    }

    #[test]
    fn the_first_event_for_a_user_applies() {
        assert!(!is_stale(None, at(18)));
    }
}
