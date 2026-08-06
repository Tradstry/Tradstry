use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use log::{info, warn};
use sqlx::Row;

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

    let mut prices: HashMap<String, Option<f64>> = HashMap::new();
    let mut triggered = 0;
    for row in rows {
        let symbol: String = row.try_get("symbol")?;
        let price = match prices.get(&symbol) {
            Some(price) => *price,
            None => {
                let price = research::price(&symbol).await.ok();
                prices.insert(symbol.clone(), price);
                price
            }
        };
        let Some(price) = price else { continue };
        let condition: String = row.try_get("condition")?;
        let threshold: f64 = row.try_get("threshold")?;
        let matched = (condition == "ABOVE" && price >= threshold)
            || (condition == "BELOW" && price <= threshold);
        if !matched {
            continue;
        }

        let id: String = row.try_get("id")?;
        let update = sqlx::query(
            "UPDATE market_monitors SET last_triggered_at = now() WHERE id = $1 \
             AND (last_triggered_at IS NULL OR last_triggered_at < now() - interval '1 hour')",
        )
        .bind(&id)
        .execute(db.pool())
        .await?;
        if update.rows_affected() == 0 {
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
        outbox::record(db.pool(), &user_id, &event, Utc::now().date_naive()).await?;
        triggered += 1;
    }
    Ok(triggered)
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
