use anyhow::{Context, Result};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

use crate::service::db::Db;

const INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Returns `(outbox, notifications, deliveries)` row counts deleted. Without this
/// the outbox becomes the largest table in the database within a year.
pub async fn prune_once(pool: &PgPool) -> Result<(u64, u64, u64)> {
    let outbox = sqlx::query(
        "DELETE FROM notification_outbox \
         WHERE processed_at IS NOT NULL AND processed_at < now() - interval '7 days'",
    )
    .execute(pool)
    .await
    .context("failed to prune notification outbox")?
    .rows_affected();

    let deliveries = sqlx::query(
        "DELETE FROM notification_deliveries \
         WHERE status IN ('sent', 'gone', 'failed') \
           AND COALESCE(sent_at, next_attempt_at) < now() - interval '30 days'",
    )
    .execute(pool)
    .await
    .context("failed to prune notification deliveries")?
    .rows_affected();

    let notifications =
        sqlx::query("DELETE FROM notifications WHERE created_at < now() - interval '90 days'")
            .execute(pool)
            .await
            .context("failed to prune notifications")?
            .rows_affected();

    Ok((outbox, notifications, deliveries))
}

pub async fn run_prune(db: Arc<Db>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(INTERVAL) => {}
            _ = shutdown.changed() => {}
        }
        if *shutdown.borrow() {
            return;
        }
        match prune_once(db.pool()).await {
            Ok((o, n, d)) => {
                log::info!("[notifications] pruned {o} outbox, {n} notifications, {d} deliveries")
            }
            Err(e) => log::error!("[notifications] prune failed: {e:#}"),
        }
    }
}
