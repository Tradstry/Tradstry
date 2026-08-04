use std::sync::Arc;
use tradstry_backend::service::countly::{Countly, QUEUE_KEY, worker};
use tradstry_backend::service::redis::client::RedisClient;

#[tokio::test]
async fn smoke_capture_then_drain_reaches_countly() {
    if std::env::var("REDIS_URL").is_err() || std::env::var("COUNTLY_APP_KEY").is_err() {
        eprintln!("skipping: REDIS_URL / COUNTLY_APP_KEY not set");
        return;
    }
    let redis = Arc::new(RedisClient::from_env().await.expect("redis"));
    let countly = Countly::from_env(redis.clone()).expect("countly");

    let before = redis.llen(QUEUE_KEY).await;
    countly
        .capture(
            "user_backend_smoke_test",
            "backend_smoke_test",
            serde_json::json!({ "source": "local_verification" }),
        )
        .await;
    let after = redis.llen(QUEUE_KEY).await;
    assert_eq!(
        after,
        before + 1,
        "capture should enqueue exactly one event"
    );

    let sent = worker::drain_once(&countly).await;
    assert!(sent >= 1, "drain should deliver at least the smoke event");
    assert_eq!(redis.llen(QUEUE_KEY).await, 0, "queue should be empty");
}
