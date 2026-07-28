use anyhow::{Result, anyhow};
use chrono::{NaiveDate, Utc};
use chrono_tz::US::Eastern;
use log::{error, info, warn};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

use super::{NotificationEvent, deliveries, outbox, preferences, render, store};
use crate::service::db::Db;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const BATCH: i64 = 100;
/// A group update inside this window reuses the first push instead of buzzing again.
const PUSH_THROTTLE: Duration = Duration::from_secs(15 * 60);

fn field<'a>(payload: &'a Value, key: &str) -> Result<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("outbox payload is missing {key}"))
}

pub fn event_from_row(event_type: &str, payload: &Value) -> Result<NotificationEvent> {
    Ok(match event_type {
        "FillsLanded" => NotificationEvent::FillsLanded {
            account_id: field(payload, "account_id")?.to_string(),
            broker: field(payload, "broker")?.to_string(),
            count: payload.get("count").and_then(Value::as_i64).unwrap_or(1),
        },
        "BrokerageConnectionDisabled" => NotificationEvent::BrokerageConnectionDisabled {
            account_id: field(payload, "account_id")?.to_string(),
            broker: field(payload, "broker")?.to_string(),
        },
        "ArtifactReady" => NotificationEvent::ArtifactReady {
            account_id: field(payload, "account_id")?.to_string(),
            kind: field(payload, "kind")?.to_string(),
            artifact_id: field(payload, "artifact_id")?.to_string(),
        },
        "PrincipleViolated" => NotificationEvent::PrincipleViolated {
            account_id: field(payload, "account_id")?.to_string(),
            trade_id: field(payload, "trade_id")?.to_string(),
            principle_id: field(payload, "principle_id")?.to_string(),
        },
        other => return Err(anyhow!("unknown notification event type {other}")),
    })
}

/// One tick. Each row is handled in its own transaction so a poison row blocks
/// only itself, and returns the number of rows retired.
pub async fn process_once(pool: &PgPool, today: NaiveDate) -> Result<usize> {
    let mut claim_conn = pool.acquire().await?;
    let rows = outbox::claim_pending(&mut claim_conn, BATCH).await?;
    drop(claim_conn);

    let mut handled = 0usize;
    for row in rows {
        match handle_row(pool, &row, today).await {
            Ok(()) => handled += 1,
            Err(e) => {
                warn!("[notifications] outbox row {} failed: {e:#}", row.id);
                outbox::mark_failed(pool, row.id, &e.to_string()).await?;
            }
        }
    }
    Ok(handled)
}

async fn handle_row(pool: &PgPool, row: &outbox::OutboxRow, today: NaiveDate) -> Result<()> {
    let event = event_from_row(&row.event_type, &row.payload)?;

    if !preferences::is_enabled(pool, &row.user_id, &row.event_type).await? {
        outbox::mark_processed(pool, row.id).await?;
        return Ok(());
    }

    let mut tx = pool.begin().await?;

    let upserted = store::upsert_coalesced(
        &mut tx,
        &row.user_id,
        &row.event_type,
        event.coalesce_key(today).as_deref(),
        &row.payload,
    )
    .await?;

    let rendered = render::render(&event, upserted.group_count);
    store::apply_copy(&mut tx, &upserted.id, &rendered).await?;

    if store::should_push(&mut tx, &upserted.id, PUSH_THROTTLE).await? {
        deliveries::fan_out(&mut tx, &upserted.id, &row.user_id).await?;
    }

    outbox::mark_processed(&mut *tx, row.id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn run_outbox_worker(db: Arc<Db>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    info!("[notifications] outbox worker started");
    loop {
        tokio::select! {
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
            _ = shutdown.changed() => {}
        }
        if *shutdown.borrow() {
            info!("[notifications] shutdown requested; exiting outbox worker");
            return;
        }

        // ET, matching every other calendar boundary in the app, so a fill at
        // 20:00 ET does not open tomorrow's group.
        let today = Utc::now().with_timezone(&Eastern).date_naive();
        if let Err(e) = process_once(db.pool(), today).await {
            error!("[notifications] outbox tick failed: {e:#}");
        }
    }
}
