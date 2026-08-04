use log::info;
use std::sync::Arc;
use std::time::Duration;

use super::{Countly, PROCESSING_KEY, QUEUE_KEY, build_requests};

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const BATCH: usize = 100;

/// Send at most one batch. Returns how many events were delivered.
///
/// Reclaims `countly:processing` first: if the previous request failed, its
/// batch is retried before anything new is claimed. Delivery is at-least-once.
pub async fn drain_once(countly: &Countly) -> usize {
    let redis = countly.redis();

    if redis.llen(PROCESSING_KEY).await == 0 {
        redis.lmove_batch(QUEUE_KEY, PROCESSING_KEY, BATCH).await;
    }

    let raw = redis.lrange(PROCESSING_KEY, 0, -1).await;
    if raw.is_empty() {
        return 0;
    }

    let events: Vec<serde_json::Value> = raw
        .iter()
        .filter_map(|s| serde_json::from_str(s).ok())
        .collect();
    if events.is_empty() {
        // Every entry was unparseable; drop them rather than retry forever.
        redis.del_key(PROCESSING_KEY).await;
        return 0;
    }

    let sent = events.len();
    if countly
        .send_batch(build_requests(countly.app_key(), events))
        .await
    {
        redis.del_key(PROCESSING_KEY).await;
        return sent;
    }
    // Leave the batch in place; the next tick retries it.
    0
}

pub async fn run_countly_worker(
    countly: Arc<Countly>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    info!("[countly] drain worker started");
    loop {
        tokio::select! {
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
            _ = shutdown.changed() => {}
        }
        if *shutdown.borrow() {
            info!("[countly] shutdown requested; exiting drain worker");
            return;
        }
        let sent = drain_once(&countly).await;
        if sent > 0 {
            info!("[countly] delivered {sent} events");
        }
    }
}
