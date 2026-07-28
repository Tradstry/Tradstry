mod pg_support;
use pg_support::{reset_schema, seed_user_account, test_pool};
use serde_json::json;
use sqlx::PgPool;
use std::time::Duration;
use tradstry_backend::service::notifications::delivery_worker;
use tradstry_backend::service::notifications::push::{FakePushSender, PushOutcome};
use tradstry_backend::service::notifications::{deliveries, store, subscriptions};

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

async fn a_notification(pool: &PgPool, user_id: &str) -> String {
    let mut conn = pool.acquire().await.unwrap();
    store::upsert_coalesced(&mut conn, user_id, "FillsLanded", None, &json!({}))
        .await
        .expect("upsert")
        .id
}

#[tokio::test]
async fn subscribing_the_same_browser_twice_upserts() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_account(&pool).await;

    let a = subscriptions::upsert(
        &pool,
        &user_id,
        "https://push/1",
        "k1",
        "a1",
        Some("Chrome"),
    )
    .await
    .unwrap();
    let b = subscriptions::upsert(
        &pool,
        &user_id,
        "https://push/1",
        "k2",
        "a2",
        Some("Chrome"),
    )
    .await
    .unwrap();
    assert_eq!(a, b, "the same endpoint must reuse its row");

    let subs = subscriptions::list_for_user(&pool, &user_id).await.unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].p256dh, "k2", "re-subscribing refreshes the keys");
}

#[tokio::test]
async fn fan_out_creates_one_row_per_browser() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_account(&pool).await;

    subscriptions::upsert(&pool, &user_id, "https://push/1", "k", "a", None)
        .await
        .unwrap();
    subscriptions::upsert(&pool, &user_id, "https://push/2", "k", "a", None)
        .await
        .unwrap();

    let n = a_notification(&pool, &user_id).await;
    let mut conn = pool.acquire().await.unwrap();
    let created = deliveries::fan_out(&mut conn, &n, &user_id, None).await.unwrap();
    assert_eq!(created, 2);
}

#[tokio::test]
async fn a_user_with_no_browsers_gets_no_delivery_rows() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_account(&pool).await;

    let n = a_notification(&pool, &user_id).await;
    let mut conn = pool.acquire().await.unwrap();
    assert_eq!(
        deliveries::fan_out(&mut conn, &n, &user_id, None).await.unwrap(),
        0
    );
}

#[tokio::test]
async fn a_browser_subscribing_later_gets_no_row_for_old_news() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_account(&pool).await;

    let n = a_notification(&pool, &user_id).await;
    let mut conn = pool.acquire().await.unwrap();
    deliveries::fan_out(&mut conn, &n, &user_id, None).await.unwrap();

    subscriptions::upsert(&pool, &user_id, "https://push/late", "k", "a", None)
        .await
        .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let due = deliveries::claim_due(&mut conn, 10).await.unwrap();
    assert_eq!(due.len(), 0, "a new browser must not receive old pushes");
}

#[tokio::test]
async fn gone_deletes_the_subscription_and_leaves_others_pending() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_account(&pool).await;

    let dead = subscriptions::upsert(&pool, &user_id, "https://push/dead", "k", "a", None)
        .await
        .unwrap();
    subscriptions::upsert(&pool, &user_id, "https://push/live", "k", "a", None)
        .await
        .unwrap();
    let n = a_notification(&pool, &user_id).await;
    let mut conn = pool.acquire().await.unwrap();
    deliveries::fan_out(&mut conn, &n, &user_id, None).await.unwrap();

    deliveries::mark_gone(&pool, &n, &dead).await.unwrap();
    subscriptions::delete_by_id(&pool, &dead).await.unwrap();

    assert_eq!(
        subscriptions::list_for_user(&pool, &user_id)
            .await
            .unwrap()
            .len(),
        1
    );
    let mut conn = pool.acquire().await.unwrap();
    let due = deliveries::claim_due(&mut conn, 10).await.unwrap();
    assert_eq!(due.len(), 1, "the healthy browser is still owed its push");
}

#[tokio::test]
async fn retry_pushes_the_next_attempt_forward_then_fails_at_the_cap() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_account(&pool).await;

    let sub = subscriptions::upsert(&pool, &user_id, "https://push/1", "k", "a", None)
        .await
        .unwrap();
    let n = a_notification(&pool, &user_id).await;
    let mut conn = pool.acquire().await.unwrap();
    deliveries::fan_out(&mut conn, &n, &user_id, None).await.unwrap();

    deliveries::mark_retry(&pool, &n, &sub, 1, "503 upstream", 10)
        .await
        .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let due = deliveries::claim_due(&mut conn, 10).await.unwrap();
    assert_eq!(due.len(), 0, "a backed-off row is not due yet");

    let row: (String, i32, Option<String>) = sqlx::query_as(
        "SELECT status, attempts, last_error FROM notification_deliveries WHERE subscription_id = $1",
    )
    .bind(&sub)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "pending");
    assert_eq!(row.1, 1);
    assert_eq!(row.2.as_deref(), Some("503 upstream"));

    deliveries::mark_retry(&pool, &n, &sub, 10, "503 upstream", 10)
        .await
        .unwrap();
    let status: (String,) =
        sqlx::query_as("SELECT status FROM notification_deliveries WHERE subscription_id = $1")
            .bind(&sub)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status.0, "failed");
}

#[tokio::test]
async fn sent_rows_are_never_claimed_again() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_account(&pool).await;

    let sub = subscriptions::upsert(&pool, &user_id, "https://push/1", "k", "a", None)
        .await
        .unwrap();
    let n = a_notification(&pool, &user_id).await;
    let mut conn = pool.acquire().await.unwrap();
    deliveries::fan_out(&mut conn, &n, &user_id, None).await.unwrap();

    deliveries::mark_sent(&pool, &n, &sub).await.unwrap();

    let mut conn = pool.acquire().await.unwrap();
    assert_eq!(deliveries::claim_due(&mut conn, 10).await.unwrap().len(), 0);
}

#[test]
fn backoff_grows_and_is_capped() {
    assert_eq!(deliveries::backoff(1), Duration::from_secs(30));
    assert_eq!(deliveries::backoff(2), Duration::from_secs(60));
    assert_eq!(deliveries::backoff(3), Duration::from_secs(120));
    assert_eq!(deliveries::backoff(99), Duration::from_secs(6 * 60 * 60));
}

#[tokio::test]
async fn a_sent_push_marks_the_row_and_touches_the_subscription() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_account(&pool).await;
    let sub = subscriptions::upsert(&pool, &user_id, "https://push/1", "k", "a", None)
        .await
        .unwrap();
    let n = a_notification(&pool, &user_id).await;
    let mut conn = pool.acquire().await.unwrap();
    deliveries::fan_out(&mut conn, &n, &user_id, None).await.unwrap();

    let sender = FakePushSender::new(vec![PushOutcome::Sent]);
    let handled = delivery_worker::deliver_once(&pool, &sender).await.unwrap();
    assert_eq!(handled, 1);

    let row: (String,) =
        sqlx::query_as("SELECT status FROM notification_deliveries WHERE subscription_id = $1")
            .bind(&sub)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.0, "sent");

    let touched: (Option<chrono::DateTime<chrono::Utc>>,) =
        sqlx::query_as("SELECT last_success_at FROM push_subscriptions WHERE id = $1")
            .bind(&sub)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(touched.0.is_some());
}

#[tokio::test]
async fn a_gone_endpoint_is_deleted() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_account(&pool).await;
    subscriptions::upsert(&pool, &user_id, "https://push/dead", "k", "a", None)
        .await
        .unwrap();
    let n = a_notification(&pool, &user_id).await;
    let mut conn = pool.acquire().await.unwrap();
    deliveries::fan_out(&mut conn, &n, &user_id, None).await.unwrap();

    let sender = FakePushSender::new(vec![PushOutcome::Gone]);
    delivery_worker::deliver_once(&pool, &sender).await.unwrap();

    assert_eq!(
        subscriptions::list_for_user(&pool, &user_id)
            .await
            .unwrap()
            .len(),
        0,
        "a permanently dead endpoint must not be retried forever"
    );
    let row: (String,) = sqlx::query_as("SELECT status FROM notification_deliveries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0, "gone");
}

#[tokio::test]
async fn one_browser_failing_does_not_affect_another() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_account(&pool).await;
    subscriptions::upsert(&pool, &user_id, "https://push/1", "k", "a", None)
        .await
        .unwrap();
    subscriptions::upsert(&pool, &user_id, "https://push/2", "k", "a", None)
        .await
        .unwrap();
    let n = a_notification(&pool, &user_id).await;
    let mut conn = pool.acquire().await.unwrap();
    deliveries::fan_out(&mut conn, &n, &user_id, None).await.unwrap();

    let sender = FakePushSender::new(vec![PushOutcome::Sent, PushOutcome::Retry("503".into())]);
    delivery_worker::deliver_once(&pool, &sender).await.unwrap();

    let statuses: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM notification_deliveries ORDER BY status")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0].0, "pending");
    assert_eq!(statuses[1].0, "sent");
}

#[tokio::test]
async fn nothing_due_is_not_an_error() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;

    let sender = FakePushSender::new(vec![]);
    assert_eq!(
        delivery_worker::deliver_once(&pool, &sender).await.unwrap(),
        0
    );
}
