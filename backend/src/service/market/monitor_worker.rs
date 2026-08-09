use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use futures_util::stream::{self, StreamExt};
use log::{info, warn};
use sqlx::Row;
use std::collections::HashSet;

use crate::service::db::Db;
use crate::service::notifications::{NotificationEvent, outbox};

use super::research;

const POLL_INTERVAL: Duration = Duration::from_secs(60);

pub async fn evaluate_once(db: &Db) -> anyhow::Result<usize> {
    let rows = sqlx::query(
        "SELECT id, workspace_id, user_id, symbol, name, condition, threshold \
         FROM market_monitors WHERE enabled \
         AND (last_triggered_at IS NULL OR last_triggered_at < now() - interval '1 hour') \
         ORDER BY symbol",
    )
    .fetch_all(db.pool())
    .await?;
    evaluate_rows(db, rows).await
}

pub async fn evaluate_workspace_once(
    db: &Db,
    user_id: &str,
    workspace_id: &str,
) -> anyhow::Result<usize> {
    let rows = sqlx::query(
        "SELECT id, workspace_id, user_id, symbol, name, condition, threshold \
         FROM market_monitors WHERE enabled AND user_id=$1 AND workspace_id=$2 \
         AND (last_triggered_at IS NULL OR last_triggered_at < now() - interval '1 hour') \
         ORDER BY symbol",
    )
    .bind(user_id)
    .bind(workspace_id)
    .fetch_all(db.pool())
    .await?;
    evaluate_rows(db, rows).await
}

async fn evaluate_rows(db: &Db, rows: Vec<sqlx::postgres::PgRow>) -> anyhow::Result<usize> {
    let symbols: HashSet<String> = rows
        .iter()
        .map(|row| row.try_get("symbol"))
        .collect::<Result<_, _>>()?;
    let fetched = stream::iter(symbols.into_iter().map(|symbol| async move {
        let price = research::price(&symbol).await.ok();
        (symbol, price)
    }))
    .buffer_unordered(8)
    .collect::<Vec<_>>()
    .await;
    let prices: HashMap<String, Option<f64>> = fetched.into_iter().collect();

    let mut candidates = Vec::new();
    for row in rows {
        let symbol: String = row.try_get("symbol")?;
        let price = prices.get(&symbol).copied().flatten();
        let Some(price) = price else { continue };
        let condition: String = row.try_get("condition")?;
        let threshold: f64 = row.try_get("threshold")?;
        let matched = (condition == "ABOVE" && price >= threshold)
            || (condition == "BELOW" && price <= threshold);
        if !matched {
            continue;
        }

        let workspace_id: String = row.try_get("workspace_id")?;
        let user_id: String = row.try_get("user_id")?;
        let event = NotificationEvent::MarketMonitorTriggered {
            workspace_id,
            symbol,
            monitor_name: row.try_get("name")?,
            price,
        };
        candidates.push((row.try_get::<String, _>("id")?, user_id, event));
    }
    if candidates.is_empty() {
        return Ok(0);
    }

    let ids: Vec<String> = candidates.iter().map(|(id, _, _)| id.clone()).collect();
    let mut tx = db.begin().await?;
    let updated: HashSet<String> = sqlx::query_scalar(
        "UPDATE market_monitors SET last_triggered_at = now() \
         WHERE id = ANY($1) \
           AND (last_triggered_at IS NULL OR last_triggered_at < now() - interval '1 hour') \
         RETURNING id",
    )
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .collect();
    let events: Vec<(String, NotificationEvent)> = candidates
        .into_iter()
        .filter(|(id, _, _)| updated.contains(id))
        .map(|(_, user_id, event)| (user_id, event))
        .collect();
    outbox::record_many_for_users(&mut tx, &events, Utc::now().date_naive()).await?;
    tx.commit().await?;
    Ok(events.len())
}

pub async fn run_monitor_worker(db: Arc<Db>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    info!("[market] monitor worker started");
    loop {
        tokio::select! {
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
            _ = shutdown.changed() => {}
        }
        if *shutdown.borrow() {
            info!("[market] monitor worker stopped");
            return;
        }
        match evaluate_once(&db).await {
            Ok(count) if count > 0 => info!("[market] triggered {count} monitor(s)"),
            Ok(_) => {}
            Err(error) => warn!("[market] monitor tick failed: {error:#}"),
        }
    }
}
