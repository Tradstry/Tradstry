use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

use super::push::{PushOutcome, PushSender};
use super::{deliveries, subscriptions};
use crate::service::db::Db;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const BATCH: i64 = 100;
const MAX_ATTEMPTS: i32 = 10;

pub async fn deliver_once(pool: &PgPool, sender: &dyn PushSender) -> Result<usize> {
    let mut conn = pool.acquire().await?;
    let due = deliveries::claim_due(&mut conn, BATCH).await?;
    drop(conn);

    let mut handled = 0usize;
    for target in due {
        match sender.send(&target).await {
            PushOutcome::Sent => {
                deliveries::mark_sent(pool, &target.notification_id, &target.subscription_id)
                    .await?;
                subscriptions::touch_success(pool, &target.subscription_id).await?;
            }
            PushOutcome::Gone => {
                deliveries::mark_gone(pool, &target.notification_id, &target.subscription_id)
                    .await?;
                subscriptions::delete_by_id(pool, &target.subscription_id).await?;
            }
            PushOutcome::Retry(error) => {
                deliveries::mark_retry(
                    pool,
                    &target.notification_id,
                    &target.subscription_id,
                    target.attempts + 1,
                    &error,
                    MAX_ATTEMPTS,
                )
                .await?;
            }
        }
        handled += 1;
    }
    Ok(handled)
}

pub async fn run_delivery_worker(
    db: Arc<Db>,
    sender: Arc<dyn PushSender>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    log::info!("[notifications] delivery worker started");
    loop {
        tokio::select! {
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
            _ = shutdown.changed() => {}
        }
        if *shutdown.borrow() {
            log::info!("[notifications] shutdown requested; exiting delivery worker");
            return;
        }
        if let Err(e) = deliver_once(db.pool(), sender.as_ref()).await {
            log::error!("[notifications] delivery tick failed: {e:#}");
        }
    }
}
