//! Postgres-backed tests for the entitlement primitives.
//!
//! The reservation statement is the only thing standing between a plan limit and
//! a user who opens ten tabs, so most of this file hammers its boundary.

mod pg_support;

use chrono::{DateTime, TimeZone, Utc};
use pg_support::{seed_user_account, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::db::schema::tables::billing_table::{self, SubscriptionUpdate};
use uuid::Uuid;

const AI: &str = "ai_actions";

fn window(month: u32) -> (DateTime<Utc>, DateTime<Utc>) {
    let start = Utc.with_ymd_and_hms(2026, month, 1, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, month + 1, 1, 0, 0, 0).unwrap();
    (start, end)
}

async fn reserve(
    pool: &PgPool,
    user: &str,
    w: (DateTime<Utc>, DateTime<Utc>),
    limit: i32,
) -> Option<i32> {
    billing_table::reserve_counter(pool, user, AI, w.0, w.1, limit)
        .await
        .expect("reserve")
}

#[tokio::test]
async fn reservation_counts_up_then_refuses_at_the_limit() {
    let pool = test_pool().await;
    let (user_id, _) = seed_user_account(&pool).await;
    let w = window(1);

    for expected in 1..=3 {
        assert_eq!(reserve(&pool, &user_id, w, 3).await, Some(expected));
    }

    assert_eq!(reserve(&pool, &user_id, w, 3).await, None);
    assert_eq!(
        billing_table::counter_used(&pool, &user_id, AI, w.0)
            .await
            .expect("counter_used"),
        3,
        "a refused reservation must not increment"
    );
}

#[tokio::test]
async fn a_zero_limit_refuses_without_creating_a_row() {
    let pool = test_pool().await;
    let (user_id, _) = seed_user_account(&pool).await;
    let w = window(2);

    assert_eq!(reserve(&pool, &user_id, w, 0).await, None);
    assert_eq!(
        billing_table::counter_used(&pool, &user_id, AI, w.0)
            .await
            .expect("counter_used"),
        0
    );
}

#[tokio::test]
async fn concurrent_reservations_never_exceed_the_limit() {
    let pool = test_pool().await;
    let (user_id, _) = seed_user_account(&pool).await;
    let w = window(3);
    const LIMIT: i32 = 5;

    let mut handles = Vec::new();
    for _ in 0..20 {
        let pool = pool.clone();
        let user_id = user_id.clone();
        handles.push(tokio::spawn(async move {
            billing_table::reserve_counter(&pool, &user_id, AI, w.0, w.1, LIMIT)
                .await
                .expect("reserve")
        }));
    }

    let mut granted: Vec<i32> = Vec::new();
    for handle in handles {
        if let Some(used) = handle.await.expect("join") {
            granted.push(used);
        }
    }
    granted.sort_unstable();

    assert_eq!(
        granted,
        (1..=LIMIT).collect::<Vec<_>>(),
        "every grant must be a distinct slot from 1..=limit"
    );
    assert_eq!(
        billing_table::counter_used(&pool, &user_id, AI, w.0)
            .await
            .expect("counter_used"),
        LIMIT
    );
}

#[tokio::test]
async fn a_new_period_starts_a_fresh_counter() {
    let pool = test_pool().await;
    let (user_id, _) = seed_user_account(&pool).await;
    let (jan, feb) = (window(4), window(5));

    assert_eq!(reserve(&pool, &user_id, jan, 1).await, Some(1));
    assert_eq!(reserve(&pool, &user_id, jan, 1).await, None);

    assert_eq!(reserve(&pool, &user_id, feb, 1).await, Some(1));
    assert_eq!(
        billing_table::counter_used(&pool, &user_id, AI, jan.0)
            .await
            .expect("counter_used"),
        1,
        "the old window is left intact"
    );
}

#[tokio::test]
async fn counters_are_scoped_per_user_and_metric() {
    let pool = test_pool().await;
    let (user_a, _) = seed_user_account(&pool).await;
    let (user_b, _) = seed_user_account(&pool).await;
    let w = window(6);

    assert_eq!(reserve(&pool, &user_a, w, 1).await, Some(1));
    assert_eq!(reserve(&pool, &user_b, w, 1).await, Some(1));

    let other = billing_table::reserve_counter(&pool, &user_a, "other_metric", w.0, w.1, 1)
        .await
        .expect("reserve");
    assert_eq!(other, Some(1));
}

#[tokio::test]
async fn plan_limits_are_seeded_for_every_tier() {
    let pool = test_pool().await;

    let free = billing_table::plan_limits(&pool, "free")
        .await
        .expect("plan_limits")
        .expect("free row");
    assert_eq!(free.ai_actions_per_month, Some(25));
    assert_eq!(free.brokerage_connections, Some(1));
    assert_eq!(free.data_bytes, Some(100 * 1024 * 1024));
    assert_eq!(free.media_bytes, Some(50 * 1024 * 1024));

    let pro = billing_table::plan_limits(&pool, "pro")
        .await
        .expect("plan_limits")
        .expect("pro row");
    assert_eq!(pro.ai_actions_per_month, Some(300));
    assert_eq!(pro.brokerage_connections, Some(3));

    let pro_plus = billing_table::plan_limits(&pool, "pro_plus")
        .await
        .expect("plan_limits")
        .expect("pro_plus row");
    assert_eq!(pro_plus.ai_actions_per_month, Some(1500));
    assert_eq!(pro_plus.brokerage_connections, Some(10));

    assert!(
        billing_table::plan_limits(&pool, "nonexistent")
            .await
            .expect("plan_limits")
            .is_none()
    );
}

#[tokio::test]
async fn new_users_default_to_free_with_no_subscription() {
    let pool = test_pool().await;
    let (user_id, _) = seed_user_account(&pool).await;

    let state = billing_table::plan_state(&pool, &user_id)
        .await
        .expect("plan_state")
        .expect("state row");

    assert_eq!(state.plan, "free");
    assert!(state.paddle_subscription_id.is_none());
    assert!(state.subscription_status.is_none());
    assert_eq!(state.data_bytes_used, 0);
    assert_eq!(state.media_bytes_used, 0);
}

#[tokio::test]
async fn applying_a_subscription_round_trips_and_is_findable_by_customer() {
    let pool = test_pool().await;
    let (user_id, _) = seed_user_account(&pool).await;

    let customer = format!("ctm_{}", &user_id[..8]);
    let subscription = format!("sub_{}", &user_id[..8]);
    let occurred = Utc.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap();
    let (start, end) = window(7);

    billing_table::apply_subscription(
        &pool,
        &user_id,
        &SubscriptionUpdate {
            plan: "pro".into(),
            paddle_customer_id: Some(customer.clone()),
            paddle_subscription_id: Some(subscription.clone()),
            subscription_status: "active".into(),
            subscription_updated_at: occurred,
            current_period_start: Some(start),
            current_period_end: Some(end),
            grace_until: None,
        },
    )
    .await
    .expect("apply_subscription");

    let state = billing_table::plan_state(&pool, &user_id)
        .await
        .expect("plan_state")
        .expect("state row");

    assert_eq!(state.plan, "pro");
    assert_eq!(state.paddle_customer_id.as_deref(), Some(customer.as_str()));
    assert_eq!(state.subscription_status.as_deref(), Some("active"));
    assert_eq!(state.subscription_updated_at, Some(occurred));
    assert_eq!(state.current_period_end, Some(end));

    assert_eq!(
        billing_table::find_user_by_paddle_customer(&pool, &customer)
            .await
            .expect("find_user_by_paddle_customer"),
        Some(user_id)
    );
}

#[tokio::test]
async fn ids_survive_an_update_that_omits_them() {
    let pool = test_pool().await;
    let (user_id, _) = seed_user_account(&pool).await;
    let customer = format!("ctm_keep_{}", &user_id[..8]);

    let base = SubscriptionUpdate {
        plan: "pro".into(),
        paddle_customer_id: Some(customer.clone()),
        paddle_subscription_id: None,
        subscription_status: "active".into(),
        subscription_updated_at: Utc.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap(),
        current_period_start: None,
        current_period_end: None,
        grace_until: None,
    };
    billing_table::apply_subscription(&pool, &user_id, &base)
        .await
        .expect("apply first");

    // A later canceled event carries no customer id; it must not blank the stored one.
    billing_table::apply_subscription(
        &pool,
        &user_id,
        &SubscriptionUpdate {
            plan: "free".into(),
            paddle_customer_id: None,
            subscription_status: "canceled".into(),
            ..base
        },
    )
    .await
    .expect("apply second");

    let state = billing_table::plan_state(&pool, &user_id)
        .await
        .expect("plan_state")
        .expect("state row");
    assert_eq!(state.plan, "free");
    assert_eq!(state.paddle_customer_id.as_deref(), Some(customer.as_str()));
}

#[tokio::test]
async fn usage_bytes_are_writable_and_clamp_at_zero() {
    let pool = test_pool().await;
    let (user_id, _) = seed_user_account(&pool).await;

    billing_table::set_usage_bytes(&pool, &user_id, 4_096, 1_024)
        .await
        .expect("set_usage_bytes");
    billing_table::add_media_bytes(&pool, &user_id, 512)
        .await
        .expect("add_media_bytes");

    let state = billing_table::plan_state(&pool, &user_id)
        .await
        .expect("plan_state")
        .expect("state row");
    assert_eq!(state.data_bytes_used, 4_096);
    assert_eq!(state.media_bytes_used, 1_536);

    // Deleting more than is recorded must not produce a negative balance.
    billing_table::add_media_bytes(&pool, &user_id, -10_000)
        .await
        .expect("add_media_bytes");
    let state = billing_table::plan_state(&pool, &user_id)
        .await
        .expect("plan_state")
        .expect("state row");
    assert_eq!(state.media_bytes_used, 0);
}

// ── webhook queue ───────────────────────────────────────────────────────────

fn event_payload(event_id: &str) -> serde_json::Value {
    serde_json::json!({
        "event_id": event_id,
        "event_type": "subscription.updated",
        "data": { "id": "sub_01", "status": "active" }
    })
}

async fn queued_ids(pool: &PgPool, prefix: &str) -> Vec<String> {
    billing_table::claim_webhook_events(pool, 100)
        .await
        .expect("claim")
        .into_iter()
        .map(|e| e.event_id)
        .filter(|id| id.starts_with(prefix))
        .collect()
}

#[tokio::test]
async fn a_redelivered_webhook_is_stored_once() {
    let pool = test_pool().await;
    let id = format!("evt_dup_{}", Uuid::new_v4());
    let occurred = Utc.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap();

    let first = billing_table::record_webhook_event(
        &pool,
        &id,
        "subscription.updated",
        occurred,
        &event_payload(&id),
    )
    .await
    .expect("record");
    assert!(first, "the first delivery is stored");

    let second = billing_table::record_webhook_event(
        &pool,
        &id,
        "subscription.updated",
        occurred,
        &event_payload(&id),
    )
    .await
    .expect("record");
    assert!(!second, "a redelivery reports that it stored nothing");

    assert_eq!(queued_ids(&pool, &id).await.len(), 1);
}

#[tokio::test]
async fn processed_events_leave_the_queue_and_failed_ones_stay() {
    let pool = test_pool().await;
    let run = Uuid::new_v4().to_string();
    let (ok_id, bad_id) = (format!("evt_ok_{run}"), format!("evt_bad_{run}"));
    let occurred = Utc.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap();

    for id in [&ok_id, &bad_id] {
        billing_table::record_webhook_event(
            &pool,
            id,
            "subscription.updated",
            occurred,
            &event_payload(id),
        )
        .await
        .expect("record");
    }

    billing_table::mark_webhook_processed(&pool, &ok_id)
        .await
        .expect("mark processed");
    billing_table::mark_webhook_failed(&pool, &bad_id, "boom")
        .await
        .expect("mark failed");

    let remaining = queued_ids(&pool, "evt_").await;
    assert!(
        !remaining.contains(&ok_id),
        "a processed event leaves the queue"
    );
    assert!(
        remaining.contains(&bad_id),
        "a failed event stays queued for retry"
    );

    // A later retry that succeeds must clear the recorded error.
    billing_table::mark_webhook_processed(&pool, &bad_id)
        .await
        .expect("retry");
    let error: Option<String> =
        sqlx::query_scalar("SELECT error FROM paddle_webhook_events WHERE event_id = $1")
            .bind(&bad_id)
            .fetch_one(&pool)
            .await
            .expect("read error column");
    assert_eq!(error, None);
}

#[tokio::test]
async fn a_permanently_failing_event_stops_being_retried() {
    let pool = test_pool().await;
    let id = format!("evt_doomed_{}", Uuid::new_v4());
    let occurred = Utc.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap();

    billing_table::record_webhook_event(
        &pool,
        &id,
        "subscription.updated",
        occurred,
        &event_payload(&id),
    )
    .await
    .expect("record");

    // Fail it exactly MAX_WEBHOOK_ATTEMPTS times, as the worker would.
    for expected in 1..=billing_table::MAX_WEBHOOK_ATTEMPTS {
        assert!(
            queued_ids(&pool, &id).await.contains(&id),
            "still claimable before attempt {expected}"
        );
        let attempts = billing_table::mark_webhook_failed(&pool, &id, "unrecognised price_id")
            .await
            .expect("mark failed");
        assert_eq!(attempts, expected);
    }

    assert!(
        !queued_ids(&pool, &id).await.contains(&id),
        "an event past the attempt ceiling must stop being claimed"
    );

    // It stays put, with its error, rather than being deleted.
    let (processed, error): (Option<DateTime<Utc>>, Option<String>) =
        sqlx::query_as("SELECT processed_at, error FROM paddle_webhook_events WHERE event_id = $1")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .expect("row still present");
    assert!(processed.is_none());
    assert_eq!(error.as_deref(), Some("unrecognised price_id"));
}

#[tokio::test]
async fn a_successful_retry_clears_the_error() {
    let pool = test_pool().await;
    let id = format!("evt_flaky_{}", Uuid::new_v4());
    let occurred = Utc.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap();

    billing_table::record_webhook_event(
        &pool,
        &id,
        "subscription.updated",
        occurred,
        &event_payload(&id),
    )
    .await
    .expect("record");

    billing_table::mark_webhook_failed(&pool, &id, "transient")
        .await
        .expect("fail once");
    assert!(
        queued_ids(&pool, &id).await.contains(&id),
        "one failure must not retire the event"
    );

    billing_table::mark_webhook_processed(&pool, &id)
        .await
        .expect("succeed");
    assert!(!queued_ids(&pool, &id).await.contains(&id));
}

#[tokio::test]
async fn the_queue_hands_back_the_oldest_event_first() {
    let pool = test_pool().await;
    let run = Uuid::new_v4().to_string();
    let (old_id, new_id) = (format!("evt_a_{run}"), format!("evt_b_{run}"));

    // Inserted newest-first so ordering cannot come from insertion order.
    billing_table::record_webhook_event(
        &pool,
        &new_id,
        "subscription.updated",
        Utc.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap(),
        &event_payload(&new_id),
    )
    .await
    .expect("record");
    billing_table::record_webhook_event(
        &pool,
        &old_id,
        "subscription.updated",
        Utc.with_ymd_and_hms(2026, 7, 17, 12, 0, 0).unwrap(),
        &event_payload(&old_id),
    )
    .await
    .expect("record");

    let ordered: Vec<String> = queued_ids(&pool, "evt_")
        .await
        .into_iter()
        .filter(|id| id.ends_with(&run))
        .collect();
    assert_eq!(ordered, vec![old_id, new_id]);
}

#[tokio::test]
async fn only_live_brokerage_connections_count_toward_the_ceiling() {
    let pool = test_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    assert_eq!(
        billing_table::active_connection_count(&pool, &user_id)
            .await
            .expect("count"),
        0,
        "a manual account holds no connection"
    );

    sqlx::query("UPDATE accounts SET snaptrade_connection_id = $2 WHERE id = $1")
        .bind(&account_id)
        .bind("conn_live")
        .execute(&pool)
        .await
        .expect("link connection");
    assert_eq!(
        billing_table::active_connection_count(&pool, &user_id)
            .await
            .expect("count"),
        1
    );

    sqlx::query("UPDATE accounts SET snaptrade_connection_disabled = true WHERE id = $1")
        .bind(&account_id)
        .execute(&pool)
        .await
        .expect("disable connection");
    assert_eq!(
        billing_table::active_connection_count(&pool, &user_id)
            .await
            .expect("count"),
        0,
        "a disabled connection frees its slot"
    );
}
