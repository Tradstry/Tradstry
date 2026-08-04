pub mod worker;

use anyhow::{Context, Result};
use chrono::{Datelike, Timelike, Utc};
use log::{error, warn};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

use crate::service::redis::client::RedisClient;

pub const QUEUE_KEY: &str = "countly:queue";
pub const PROCESSING_KEY: &str = "countly:processing";
/// Shed rather than let a Countly outage fill Redis.
pub const MAX_QUEUE_LEN: usize = 100_000;
/// A slow Redis must never hold up a user's request.
const ENQUEUE_TIMEOUT: Duration = Duration::from_millis(50);

#[derive(Clone)]
pub struct Countly {
    redis: Arc<RedisClient>,
    app_key: String,
    host: String,
    http: reqwest::Client,
}

pub fn build_event(device_id: &str, event: &str, props: Value) -> Value {
    json!({
        "device_id": device_id,
        "event": {
            "key": event,
            "count": 1,
            "segmentation": props,
        },
        "timestamp": Utc::now().timestamp_millis(),
    })
}

/// Countly's `/i/bulk` endpoint accepts an array of ordinary `/i` requests.
pub fn build_requests(app_key: &str, events: Vec<Value>) -> Vec<Value> {
    events
        .into_iter()
        .filter_map(|payload| {
            let device_id = payload.get("device_id")?.as_str()?;
            let event = payload.get("event")?.clone();
            let timestamp = payload.get("timestamp")?.as_i64()?;
            let timestamp = chrono::DateTime::from_timestamp_millis(timestamp)?;
            Some(json!({
                "app_key": app_key,
                "device_id": device_id,
                "sdk_name": "tradstry-backend",
                "sdk_version": env!("CARGO_PKG_VERSION"),
                "t": 0,
                "timestamp": timestamp.timestamp_millis(),
                "hour": timestamp.hour(),
                "dow": timestamp.weekday().num_days_from_sunday(),
                "tz": 0,
                "events": [event],
            }))
        })
        .collect()
}

impl Countly {
    pub fn from_env(redis: Arc<RedisClient>) -> Result<Self> {
        let app_key = std::env::var("COUNTLY_APP_KEY")
            .context("COUNTLY_APP_KEY environment variable not set")?;
        let host =
            std::env::var("COUNTLY_HOST").context("COUNTLY_HOST environment variable not set")?;
        Ok(Self {
            redis,
            app_key,
            host,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .context("failed to build the Countly HTTP client")?,
        })
    }

    /// Enqueue one event. Never returns an error: analytics must not be able to
    /// fail a trade save. Drops are logged so they stay visible.
    pub async fn capture(&self, device_id: &str, event: &str, props: Value) {
        if self.redis.llen(QUEUE_KEY).await >= MAX_QUEUE_LEN {
            warn!("[countly] queue at capacity; dropping {event}");
            return;
        }
        let payload = build_event(device_id, event, props);
        let encoded = match serde_json::to_string(&payload) {
            Ok(s) => s,
            Err(e) => {
                error!("[countly] could not encode {event}: {e}");
                return;
            }
        };
        match tokio::time::timeout(ENQUEUE_TIMEOUT, self.redis.rpush(QUEUE_KEY, &encoded)).await {
            Ok(true) => {}
            Ok(false) => warn!("[countly] redis rejected {event}"),
            Err(_) => warn!("[countly] redis enqueue timed out; dropping {event}"),
        }
    }

    pub(crate) fn app_key(&self) -> &str {
        &self.app_key
    }

    pub(crate) fn redis(&self) -> &RedisClient {
        &self.redis
    }

    pub(crate) async fn send_batch(&self, requests: Vec<Value>) -> bool {
        if requests.is_empty() {
            return true;
        }
        let url = format!("{}/i/bulk", self.host.trim_end_matches('/'));
        let requests = match serde_json::to_string(&requests) {
            Ok(value) => value,
            Err(error) => {
                error!("[countly] could not encode bulk requests: {error}");
                return false;
            }
        };
        match self
            .http
            .post(&url)
            .form(&[("requests", requests)])
            .send()
            .await
        {
            Ok(res) if res.status().is_success() => true,
            Ok(res) => {
                error!("[countly] bulk rejected with status {}", res.status());
                false
            }
            Err(e) => {
                error!("[countly] bulk POST failed: {e}");
                false
            }
        }
    }
}

/// Background jobs hold only the internal id. Every event must carry the Clerk id
/// so browser and server events resolve to the same Countly device profile.
pub async fn clerk_id_for_user(pool: &PgPool, internal_user_id: &str) -> Option<String> {
    match sqlx::query_scalar::<_, String>("SELECT clerk_uuid FROM users WHERE id = $1")
        .bind(internal_user_id)
        .fetch_optional(pool)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            error!("[countly] clerk id lookup failed for {internal_user_id}: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_countly_bulk_requests_with_identity_and_segments() {
        let event = build_event(
            "user_abc",
            "brokerage_sync_completed",
            json!({ "broker": "Webull" }),
        );
        let requests = build_requests("countly-test", vec![event]);
        assert_eq!(requests[0]["app_key"], "countly-test");
        assert_eq!(requests[0]["device_id"], "user_abc");
        assert_eq!(requests[0]["events"][0]["key"], "brokerage_sync_completed");
        assert_eq!(requests[0]["events"][0]["segmentation"]["broker"], "Webull");
        assert_eq!(requests[0]["t"], 0);
    }
}
