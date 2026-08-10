use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use base64::Engine;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use tokio::time::{Duration, sleep};

use super::client::BrokerageClient;
use super::db::decrypt_secret;
use super::transaction;
use crate::service::db::Db;
use crate::service::db::schema::tables::workspaces_table;
use crate::service::redis::brokerage as brokerage_cache;
use crate::service::redis::client::RedisClient;

#[derive(Debug, Deserialize, Serialize)]
pub struct WebhookEvent {
    pub event_id: String,
    pub event_type: String,
    pub event_timestamp: String,
    pub user_id: String,
    pub account_id: Option<String>,
    pub connection_id: Option<String>,
    pub details: Option<WebhookDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WebhookDetails {
    #[serde(alias = "totalValue")]
    pub total_value: Option<WebhookOperation>,
    pub positions: Option<WebhookOperation>,
    pub balances: Option<WebhookOperation>,
    pub orders: Option<WebhookOperation>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WebhookOperation {
    pub success: bool,
    pub error: Option<String>,
}

const MAX_WEBHOOK_AGE: Duration = Duration::from_secs(5 * 60);
const MAX_FUTURE_SKEW: Duration = Duration::from_secs(60);

pub fn verify_and_normalize(
    body: &[u8],
    signature: &str,
    consumer_key: &str,
    now: DateTime<Utc>,
) -> Result<WebhookEvent> {
    anyhow::ensure!(!signature.is_empty(), "missing Signature header");
    let value: Value = serde_json::from_slice(body).context("invalid webhook JSON")?;
    let canonical = serde_json::to_vec(&value).context("canonicalize webhook JSON")?;
    let provided = base64::engine::general_purpose::STANDARD
        .decode(signature)
        .context("invalid webhook signature encoding")?;
    let mut mac = Hmac::<Sha256>::new_from_slice(consumer_key.as_bytes())
        .map_err(|_| anyhow!("invalid SnapTrade consumer key"))?;
    mac.update(&canonical);
    mac.verify_slice(&provided)
        .map_err(|_| anyhow!("invalid webhook signature"))?;

    let raw = value
        .as_object()
        .context("webhook payload must be a JSON object")?;
    let event_timestamp = read_string(raw, &["eventTimestamp", "event_timestamp"])
        .context("webhook event timestamp is required")?;
    let sent_at = DateTime::parse_from_rfc3339(&event_timestamp)
        .context("invalid webhook event timestamp")?
        .with_timezone(&Utc);
    let age = now.signed_duration_since(sent_at);
    anyhow::ensure!(
        age <= chrono::Duration::from_std(MAX_WEBHOOK_AGE)?,
        "webhook timestamp outside allowed window"
    );
    anyhow::ensure!(
        age >= -chrono::Duration::from_std(MAX_FUTURE_SKEW)?,
        "webhook timestamp outside allowed window"
    );

    let event_type = read_string(raw, &["webhookType", "eventType", "event_type", "type"])
        .context("webhook event type is required")?
        .to_ascii_uppercase();
    let user_id =
        read_string(raw, &["userId", "user_id"]).context("webhook user ID is required")?;
    let event_id = read_string(raw, &["webhookId", "eventId", "event_id", "id"])
        .unwrap_or_else(|| hex::encode(Sha256::digest(&canonical)));
    let details = if event_type == "ACCOUNT_HOLDINGS_UPDATED" {
        raw.get("details")
            .filter(|value| !value.is_null())
            .map(|value| serde_json::from_value(value.clone()).context("invalid webhook details"))
            .transpose()?
    } else {
        None
    };
    Ok(WebhookEvent {
        event_id,
        event_type,
        event_timestamp,
        user_id,
        account_id: read_string(raw, &["accountId", "account_id"]),
        connection_id: read_string(
            raw,
            &[
                "brokerageAuthorizationId",
                "authorizationId",
                "connectionId",
                "connection_id",
            ],
        ),
        details,
    })
}

fn read_string(raw: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        raw.get(*key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

struct PendingEvent {
    event_id: String,
    event: WebhookEvent,
}

struct SyncTarget {
    user_id: String,
    workspace_id: String,
    encrypted_secret: String,
    connection_id: String,
    account_id: Option<String>,
    broker: String,
}

/// Persist before acknowledging the adapter. The event ID is supplied by
/// SnapTrade (or deterministically derived by the adapter), so redelivery is
/// idempotent.
pub async fn ingest(pool: &PgPool, event: &WebhookEvent) -> Result<bool> {
    anyhow::ensure!(!event.event_id.is_empty(), "webhook event ID is required");
    anyhow::ensure!(
        !event.event_type.is_empty(),
        "webhook event type is required"
    );
    anyhow::ensure!(!event.user_id.is_empty(), "webhook user ID is required");
    let timestamp = DateTime::parse_from_rfc3339(&event.event_timestamp)
        .context("invalid webhook event timestamp")?
        .with_timezone(&Utc);
    let inserted = sqlx::query(
        "INSERT INTO snaptrade_webhook_events \
         (event_id, event_type, snaptrade_user_id, snaptrade_account_id, \
          snaptrade_connection_id, event_timestamp, details, normalized_event) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(&event.event_id)
    .bind(&event.event_type)
    .bind(&event.user_id)
    .bind(event.account_id.as_deref())
    .bind(event.connection_id.as_deref())
    .bind(timestamp)
    .bind(
        event
            .details
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?,
    )
    .bind(serde_json::to_value(event)?)
    .execute(pool)
    .await
    .context("insert SnapTrade webhook event")?
    .rows_affected()
        == 1;
    Ok(inserted)
}

async fn claim(pool: &PgPool) -> Result<Option<PendingEvent>> {
    let row = sqlx::query(
        "WITH candidate AS ( \
             SELECT event_id FROM snaptrade_webhook_events \
             WHERE processed_at IS NULL AND attempts < 8 AND next_attempt_at <= now() \
             ORDER BY event_timestamp, created_at FOR UPDATE SKIP LOCKED LIMIT 1 \
         ) \
         UPDATE snaptrade_webhook_events event SET \
             attempts = event.attempts + 1, \
             next_attempt_at = now() + interval '5 minutes' \
         FROM candidate WHERE event.event_id = candidate.event_id \
         RETURNING event.event_id, event.event_type, event.event_timestamp, \
             event.snaptrade_user_id, event.snaptrade_account_id, \
             event.snaptrade_connection_id, event.details",
    )
    .fetch_optional(pool)
    .await
    .context("claim SnapTrade webhook event")?;

    row.map(|row| {
        let timestamp: DateTime<Utc> = row.try_get(2)?;
        Ok(PendingEvent {
            event_id: row.try_get(0)?,
            event: WebhookEvent {
                event_type: row.try_get(1)?,
                event_timestamp: timestamp.to_rfc3339(),
                user_id: row.try_get(3)?,
                account_id: row.try_get(4)?,
                connection_id: row.try_get(5)?,
                details: row
                    .try_get::<Option<Value>, _>(6)?
                    .map(serde_json::from_value)
                    .transpose()?,
                event_id: row.try_get(0)?,
            },
        })
    })
    .transpose()
}

fn holdings_refresh_succeeded(details: Option<&WebhookDetails>) -> bool {
    let Some(details) = details else {
        return true;
    };
    [
        details.total_value.as_ref(),
        details.positions.as_ref(),
        details.balances.as_ref(),
        details.orders.as_ref(),
    ]
    .into_iter()
    .all(|operation| operation.is_none_or(|operation| operation.success))
}

async fn targets(pool: &PgPool, event: &WebhookEvent) -> Result<Vec<SyncTarget>> {
    let rows = sqlx::query(
        "SELECT bc.user_id, bc.workspace_id, bc.snaptrade_user_secret_encrypted, \
                bc.snaptrade_connection_id, bc.snaptrade_account_id, \
                COALESCE(bc.broker, 'your brokerage') \
         FROM brokerage_connections bc \
         WHERE bc.snaptrade_user_id = $1 \
           AND bc.snaptrade_user_secret_encrypted IS NOT NULL \
           AND bc.snaptrade_connection_id IS NOT NULL \
           AND ($2::text IS NULL OR bc.snaptrade_account_id = $2) \
           AND ($3::text IS NULL OR bc.snaptrade_connection_id = $3)",
    )
    .bind(&event.user_id)
    .bind(event.account_id.as_deref())
    .bind(event.connection_id.as_deref())
    .fetch_all(pool)
    .await
    .context("find webhook sync target")?;

    rows.into_iter()
        .map(|row| {
            Ok(SyncTarget {
                user_id: row.try_get(0)?,
                workspace_id: row.try_get(1)?,
                encrypted_secret: row.try_get(2)?,
                connection_id: row.try_get(3)?,
                account_id: row.try_get(4)?,
                broker: row.try_get(5)?,
            })
        })
        .collect()
}

async fn process_target(
    db: &Db,
    brokerage: &BrokerageClient,
    redis: Option<&RedisClient>,
    event: &WebhookEvent,
    target: SyncTarget,
) -> Result<()> {
    let secret = decrypt_secret(&target.encrypted_secret).context("decrypt brokerage secret")?;
    let connection = brokerage
        .get_connection_status(&event.user_id, &secret, &target.connection_id)
        .await
        .context("read webhook connection status")?;
    workspaces_table::set_connection_disabled(
        db.pool(),
        &target.workspace_id,
        &target.user_id,
        connection.disabled.unwrap_or(false),
        connection.disabled_date.as_deref(),
    )
    .await?;
    workspaces_table::set_connection_freshness_mode(
        db.pool(),
        &target.workspace_id,
        &target.user_id,
        &connection.data_freshness_mode,
    )
    .await?;

    if matches!(
        event.event_type.as_str(),
        "CONNECTION_BROKEN" | "CONNECTION_FIXED"
    ) {
        return Ok(());
    }

    let accounts = brokerage
        .list_snaptrade_accounts(&event.user_id, &secret)
        .await
        .context("list webhook brokerage accounts")?;
    let requested_account = event.account_id.as_deref().or(target.account_id.as_deref());
    let account = accounts
        .iter()
        .find(|candidate| candidate.id.as_deref() == requested_account)
        .context("webhook brokerage account is not bound locally")?;
    let account_id = account
        .id
        .as_deref()
        .context("brokerage account ID missing")?;

    match event.event_type.as_str() {
        "ACCOUNT_HOLDINGS_UPDATED" => {
            if !holdings_refresh_succeeded(event.details.as_ref()) {
                log::warn!(
                    "SnapTrade holdings refresh reported an incomplete upstream update; preserving prior snapshot"
                );
                return Ok(());
            }
            transaction::sync_holdings_if_advanced(
                brokerage,
                db.pool(),
                &event.user_id,
                &secret,
                account_id,
                &target.user_id,
                &target.workspace_id,
                account
                    .sync_status
                    .as_ref()
                    .and_then(|status| status.holdings.as_ref()),
                &connection.data_freshness_mode,
                true,
            )
            .await?;
        }
        "ACCOUNT_TRANSACTIONS_UPDATED" | "ACCOUNT_TRANSACTIONS_INITIAL_UPDATE" => {
            transaction::sync_transactions_if_advanced(
                brokerage,
                db.pool(),
                &event.user_id,
                &secret,
                account_id,
                &target.user_id,
                &target.workspace_id,
                &target.broker,
                account
                    .sync_status
                    .as_ref()
                    .and_then(|status| status.transactions.as_ref()),
                false,
            )
            .await?;
        }
        _ => return Ok(()),
    }

    if let Some(redis) = redis {
        brokerage_cache::invalidate_account_cache(redis, &target.user_id, &target.workspace_id)
            .await;
    }
    Ok(())
}

async fn process(
    db: &Db,
    brokerage: &BrokerageClient,
    redis: Option<&RedisClient>,
    event: &WebhookEvent,
) -> Result<()> {
    let supported = matches!(
        event.event_type.as_str(),
        "ACCOUNT_HOLDINGS_UPDATED"
            | "ACCOUNT_TRANSACTIONS_UPDATED"
            | "ACCOUNT_TRANSACTIONS_INITIAL_UPDATE"
            | "CONNECTION_BROKEN"
            | "CONNECTION_FIXED"
    );
    if !supported {
        return Ok(());
    }
    let targets = targets(db.pool(), event).await?;
    anyhow::ensure!(
        !targets.is_empty(),
        "no local brokerage binding exists for webhook target"
    );
    for target in targets {
        process_target(db, brokerage, redis, event, target).await?;
    }
    Ok(())
}

async fn complete(pool: &PgPool, event_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE snaptrade_webhook_events SET processed_at = now(), last_error = NULL WHERE event_id = $1",
    )
    .bind(event_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn fail(pool: &PgPool, event_id: &str, error: &anyhow::Error) -> Result<()> {
    let message: String = error.to_string().chars().take(500).collect();
    sqlx::query(
        "UPDATE snaptrade_webhook_events SET last_error = $2, \
         next_attempt_at = now() + make_interval(secs => LEAST(300, (2 ^ LEAST(attempts, 8))::int)) \
         WHERE event_id = $1",
    )
        .bind(event_id)
        .bind(message)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn run_worker(
    db: Arc<Db>,
    brokerage: Arc<BrokerageClient>,
    redis: Option<Arc<RedisClient>>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    log::info!("SnapTrade webhook worker started");
    loop {
        if *shutdown.borrow() {
            return;
        }
        match claim(db.pool()).await {
            Ok(Some(pending)) => match process(
                &db,
                &brokerage,
                redis.as_ref().map(AsRef::as_ref),
                &pending.event,
            )
            .await
            {
                Ok(()) => {
                    if let Err(error) = complete(db.pool(), &pending.event_id).await {
                        log::error!("failed to complete SnapTrade webhook: {error}");
                    }
                }
                Err(error) => {
                    log::warn!(
                        "SnapTrade webhook processing failed for event {}: {}",
                        pending.event_id,
                        error
                    );
                    if let Err(store_error) = fail(db.pool(), &pending.event_id, &error).await {
                        log::error!("failed to record SnapTrade webhook failure: {store_error}");
                    }
                }
            },
            Ok(None) => {
                tokio::select! {
                    _ = sleep(Duration::from_secs(2)) => {}
                    _ = shutdown.changed() => {}
                }
            }
            Err(error) => {
                log::error!("SnapTrade webhook worker poll failed: {error}");
                tokio::select! {
                    _ = sleep(Duration::from_secs(5)) => {}
                    _ = shutdown.changed() => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(body: &[u8], key: &str) -> String {
        let value: Value = serde_json::from_slice(body).unwrap();
        let canonical = serde_json::to_vec(&value).unwrap();
        let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).unwrap();
        mac.update(&canonical);
        base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
    }

    #[test]
    fn verifies_and_normalizes_official_webhook_shape() {
        let now = "2026-08-09T12:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let body = br#"{
            "webhookId":"event-1",
            "eventTimestamp":"2026-08-09T12:00:00Z",
            "userId":"snaptrade-user",
            "eventType":"ACCOUNT_HOLDINGS_UPDATED",
            "accountId":"account-1",
            "brokerageAuthorizationId":"connection-1",
            "details":{"positions":{"success":true,"error":null}}
        }"#;
        let event =
            verify_and_normalize(body, &signature(body, "consumer-key"), "consumer-key", now)
                .unwrap();
        assert_eq!(event.event_id, "event-1");
        assert_eq!(event.user_id, "snaptrade-user");
        assert_eq!(event.connection_id.as_deref(), Some("connection-1"));
    }

    #[test]
    fn rejects_stale_or_tampered_webhooks() {
        let now = "2026-08-09T12:06:01Z".parse::<DateTime<Utc>>().unwrap();
        let body = br#"{"eventTimestamp":"2026-08-09T12:00:00Z"}"#;
        assert!(
            verify_and_normalize(body, &signature(body, "consumer-key"), "consumer-key", now)
                .is_err()
        );
        assert!(
            verify_and_normalize(body, &signature(body, "wrong-key"), "consumer-key", now).is_err()
        );
    }

    #[test]
    fn failed_holdings_component_preserves_previous_snapshot() {
        let details = serde_json::json!({
            "total_value": {"success": true, "error": null},
            "positions": {"success": false, "error": "upstream unavailable"},
            "balances": {"success": true, "error": null},
            "orders": {"success": true, "error": null}
        });
        let details: WebhookDetails = serde_json::from_value(details).unwrap();
        assert!(!holdings_refresh_succeeded(Some(&details)));
    }

    #[test]
    fn complete_holdings_refresh_is_accepted() {
        let details = serde_json::json!({
            "total_value": {"success": true},
            "positions": {"success": true},
            "balances": {"success": true},
            "orders": {"success": true}
        });
        let details: WebhookDetails = serde_json::from_value(details).unwrap();
        assert!(holdings_refresh_succeeded(Some(&details)));
    }
}
