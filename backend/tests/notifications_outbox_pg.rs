mod pg_support;
use chrono::NaiveDate;
use pg_support::{reset_schema, seed_user_workspace, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::notifications::{NotificationEvent, outbox, subscriptions};

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 7, 28).unwrap()
}

fn fills(workspace_id: &str) -> NotificationEvent {
    NotificationEvent::FillsLanded {
        workspace_id: workspace_id.to_string(),
        broker: "Webull".into(),
        count: 3,
    }
}

#[tokio::test]
async fn record_writes_one_pending_row() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    outbox::record(&pool, &user_id, &fills(&workspace_id), day())
        .await
        .expect("record");

    let rows: Vec<(
        String,
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = sqlx::query_as("SELECT event_type, coalesce_key, processed_at FROM notification_outbox")
        .fetch_all(&pool)
        .await
        .expect("select");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "FillsLanded");
    assert_eq!(
        rows[0].1.as_deref(),
        Some(&*format!("fills:{workspace_id}:2026-07-28"))
    );
    assert!(rows[0].2.is_none(), "a fresh row must be pending");
}

#[tokio::test]
async fn a_rolled_back_producer_leaves_no_event() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let mut tx = pool.begin().await.expect("begin");
    outbox::record(&mut *tx, &user_id, &fills(&workspace_id), day())
        .await
        .expect("record inside tx");
    tx.rollback().await.expect("rollback");

    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM notification_outbox")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(
        count.0, 0,
        "the event must die with the transaction that caused it"
    );
}

#[tokio::test]
async fn claim_skips_already_processed_rows() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    outbox::record(&pool, &user_id, &fills(&workspace_id), day())
        .await
        .expect("record");
    outbox::record(&pool, &user_id, &fills(&workspace_id), day())
        .await
        .expect("record");

    let mut conn = pool.acquire().await.expect("acquire");
    let claimed = outbox::claim_pending(&mut conn, 10).await.expect("claim");
    assert_eq!(claimed.len(), 2);

    outbox::mark_processed(&pool, claimed[0].id)
        .await
        .expect("mark");

    let mut conn = pool.acquire().await.expect("acquire");
    let again = outbox::claim_pending(&mut conn, 10).await.expect("claim");
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].id, claimed[1].id);
}

#[tokio::test]
async fn mark_failed_increments_attempts_and_records_why() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    outbox::record(&pool, &user_id, &fills(&workspace_id), day())
        .await
        .expect("record");
    let id: (i64,) = sqlx::query_as("SELECT id FROM notification_outbox")
        .fetch_one(&pool)
        .await
        .expect("id");

    outbox::mark_failed(&pool, id.0, "render blew up")
        .await
        .expect("fail");

    let row: (i32, Option<String>, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT attempts, last_error, processed_at FROM notification_outbox WHERE id = $1",
    )
    .bind(id.0)
    .fetch_one(&pool)
    .await
    .expect("select");
    assert_eq!(row.0, 1);
    assert_eq!(row.1.as_deref(), Some("render blew up"));
    assert!(row.2.is_none(), "one failure must not retire the row");

    for _ in 0..4 {
        outbox::mark_failed(&pool, id.0, "again")
            .await
            .expect("fail");
    }
    let row: (i32, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT attempts, processed_at FROM notification_outbox WHERE id = $1")
            .bind(id.0)
            .fetch_one(&pool)
            .await
            .expect("select");
    assert_eq!(row.0, 5);
    assert!(
        row.1.is_some(),
        "a poison row must retire at the attempt cap instead of spinning forever"
    );
}

use serde_json::json;
use std::time::Duration;
use tradstry_backend::service::notifications::render::Rendered;
use tradstry_backend::service::notifications::store;

async fn upsert(pool: &PgPool, user_id: &str, key: Option<&str>) -> store::UpsertResult {
    let mut conn = pool.acquire().await.expect("acquire");
    store::upsert_coalesced(&mut conn, user_id, "FillsLanded", key, &json!({"count": 1}))
        .await
        .expect("upsert")
}

#[tokio::test]
async fn twelve_events_with_one_key_make_one_notification() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _account_id) = seed_user_workspace(&pool).await;

    let first = upsert(&pool, &user_id, Some("fills:acc1:2026-07-28")).await;
    assert!(first.created);
    assert_eq!(first.group_count, 1);

    let mut last = first.clone();
    for _ in 0..11 {
        last = upsert(&pool, &user_id, Some("fills:acc1:2026-07-28")).await;
        assert!(!last.created);
    }
    assert_eq!(last.id, first.id);
    assert_eq!(last.group_count, 12);

    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM notifications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}

#[tokio::test]
async fn reading_a_group_starts_a_new_one() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _account_id) = seed_user_workspace(&pool).await;

    let first = upsert(&pool, &user_id, Some("fills:acc1:2026-07-28")).await;
    store::mark_read(&pool, &user_id, &first.id)
        .await
        .expect("mark read");

    let second = upsert(&pool, &user_id, Some("fills:acc1:2026-07-28")).await;
    assert!(second.created, "a read group must not absorb new events");
    assert_ne!(second.id, first.id);
    assert_eq!(second.group_count, 1);
}

#[tokio::test]
async fn a_null_key_never_groups() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _account_id) = seed_user_workspace(&pool).await;

    let a = upsert(&pool, &user_id, None).await;
    let b = upsert(&pool, &user_id, None).await;
    assert!(a.created && b.created);
    assert_ne!(a.id, b.id);
}

#[tokio::test]
async fn two_users_do_not_share_a_group() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_a, _) = seed_user_workspace(&pool).await;
    let (user_b, _) = seed_user_workspace(&pool).await;

    let a = upsert(&pool, &user_a, Some("fills:acc1:2026-07-28")).await;
    let b = upsert(&pool, &user_b, Some("fills:acc1:2026-07-28")).await;
    assert_ne!(a.id, b.id);
    assert!(b.created);
}

#[tokio::test]
async fn feed_and_unread_count_track_read_state() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _account_id) = seed_user_workspace(&pool).await;

    let a = upsert(&pool, &user_id, None).await;
    let _b = upsert(&pool, &user_id, None).await;
    let mut conn = pool.acquire().await.unwrap();
    store::apply_copy(
        &mut conn,
        &a.id,
        &Rendered {
            title: "New fill on Webull".into(),
            body: "body".into(),
            deep_link: Some("/dashboard/brokerage".into()),
        },
    )
    .await
    .expect("apply copy");

    assert_eq!(store::unread_count(&pool, &user_id).await.unwrap(), 2);
    let feed = store::feed(&pool, &user_id, 50, None).await.unwrap();
    assert_eq!(feed.len(), 2);
    let rendered = feed.iter().find(|r| r.id == a.id).unwrap();
    assert_eq!(rendered.title, "New fill on Webull");

    assert!(store::mark_read(&pool, &user_id, &a.id).await.unwrap());
    assert_eq!(store::unread_count(&pool, &user_id).await.unwrap(), 1);

    assert_eq!(store::mark_all_read(&pool, &user_id).await.unwrap(), 1);
    assert_eq!(store::unread_count(&pool, &user_id).await.unwrap(), 0);
}

#[tokio::test]
async fn mark_read_refuses_another_users_notification() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (owner, _) = seed_user_workspace(&pool).await;
    let (stranger, _) = seed_user_workspace(&pool).await;

    let n = upsert(&pool, &owner, None).await;
    assert!(
        !store::mark_read(&pool, &stranger, &n.id).await.unwrap(),
        "a stranger must not be able to mark someone else's notification read"
    );
    assert_eq!(store::unread_count(&pool, &owner).await.unwrap(), 1);
}

#[tokio::test]
async fn push_throttle_allows_the_first_and_blocks_the_immediate_second() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _account_id) = seed_user_workspace(&pool).await;

    let n = upsert(&pool, &user_id, Some("k")).await;
    let mut conn = pool.acquire().await.unwrap();
    assert!(
        store::should_push(&mut conn, &n.id, Duration::from_secs(900))
            .await
            .unwrap(),
        "a never-pushed notification always pushes"
    );
    let mut conn = pool.acquire().await.unwrap();
    assert!(
        !store::should_push(&mut conn, &n.id, Duration::from_secs(900))
            .await
            .unwrap(),
        "a second push inside the throttle window is suppressed"
    );

    sqlx::query(
        "UPDATE notifications SET last_pushed_at = now() - interval '20 minutes' WHERE id = $1",
    )
    .bind(&n.id)
    .execute(&pool)
    .await
    .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    assert!(
        store::should_push(&mut conn, &n.id, Duration::from_secs(900))
            .await
            .unwrap(),
        "once the window has passed, an update pushes again"
    );
}

use tradstry_backend::service::notifications::outbox_worker;
use tradstry_backend::service::notifications::preferences;

#[tokio::test]
async fn a_tick_turns_events_into_one_coalesced_notification() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    for _ in 0..12 {
        outbox::record(&pool, &user_id, &fills(&workspace_id), day())
            .await
            .unwrap();
    }

    let handled = outbox_worker::process_once(&pool, day())
        .await
        .expect("tick");
    assert_eq!(handled, 12);

    let rows = store::feed(&pool, &user_id, 50, None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].group_count, 12);
    assert_eq!(rows[0].title, "12 new fills on Webull");

    let pending: (i64,) =
        sqlx::query_as("SELECT count(*) FROM notification_outbox WHERE processed_at IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pending.0, 0);
}

#[tokio::test]
async fn a_muted_type_is_consumed_but_produces_nothing() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    preferences::set(&pool, &user_id, "FillsLanded", false)
        .await
        .unwrap();
    outbox::record(&pool, &user_id, &fills(&workspace_id), day())
        .await
        .unwrap();

    outbox_worker::process_once(&pool, day()).await.unwrap();

    assert_eq!(
        store::feed(&pool, &user_id, 50, None).await.unwrap().len(),
        0
    );
    let pending: (i64,) =
        sqlx::query_as("SELECT count(*) FROM notification_outbox WHERE processed_at IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pending.0, 0, "a dropped event must still be retired");
}

#[tokio::test]
async fn the_first_event_fans_out_and_the_immediate_second_does_not() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    subscriptions::upsert(&pool, &user_id, "https://push/1", "k", "a", None)
        .await
        .unwrap();

    outbox::record(&pool, &user_id, &fills(&workspace_id), day())
        .await
        .unwrap();
    outbox_worker::process_once(&pool, day()).await.unwrap();
    let after_first: (i64,) = sqlx::query_as("SELECT count(*) FROM notification_deliveries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after_first.0, 1);

    outbox::record(&pool, &user_id, &fills(&workspace_id), day())
        .await
        .unwrap();
    outbox_worker::process_once(&pool, day()).await.unwrap();
    let after_second: (i64,) = sqlx::query_as("SELECT count(*) FROM notification_deliveries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        after_second.0, 1,
        "a group update inside the throttle window must not buzz the browser again"
    );
}

#[tokio::test]
async fn an_unparseable_event_does_not_block_the_queue_behind_it() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    sqlx::query(
        "INSERT INTO notification_outbox (user_id, event_type, payload) \
         VALUES ($1, 'NotARealEvent', '{}'::jsonb)",
    )
    .bind(&user_id)
    .execute(&pool)
    .await
    .unwrap();
    outbox::record(&pool, &user_id, &fills(&workspace_id), day())
        .await
        .unwrap();

    outbox_worker::process_once(&pool, day()).await.unwrap();

    let feed = store::feed(&pool, &user_id, 50, None).await.unwrap();
    assert_eq!(feed.len(), 1, "the healthy event still lands");

    let poison: (i32, Option<String>) = sqlx::query_as(
        "SELECT attempts, last_error FROM notification_outbox WHERE event_type = 'NotARealEvent'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(poison.0, 1);
    assert!(poison.1.is_some());
}

#[tokio::test]
async fn a_disabled_connection_records_one_ungrouped_event() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let event = NotificationEvent::BrokerageConnectionDisabled {
        workspace_id: workspace_id.clone(),
        broker: "Webull".into(),
    };
    outbox::record(&pool, &user_id, &event, day())
        .await
        .unwrap();
    outbox::record(&pool, &user_id, &event, day())
        .await
        .unwrap();

    outbox_worker::process_once(&pool, day()).await.unwrap();

    let feed = store::feed(&pool, &user_id, 50, None).await.unwrap();
    assert_eq!(
        feed.len(),
        2,
        "each broken connection needs its own dismissable item"
    );
    assert_eq!(feed[0].title, "Reconnect Webull");
}

use tradstry_backend::service::notifications::prune;

#[tokio::test]
async fn prune_removes_old_rows_and_keeps_recent_ones() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    outbox::record(&pool, &user_id, &fills(&workspace_id), day())
        .await
        .unwrap();
    outbox_worker::process_once(&pool, day()).await.unwrap();

    sqlx::query("UPDATE notification_outbox SET processed_at = now() - interval '30 days'")
        .execute(&pool)
        .await
        .unwrap();

    let (outbox_deleted, notifications_deleted, _) = prune::prune_once(&pool).await.unwrap();
    assert_eq!(outbox_deleted, 1);
    assert_eq!(
        notifications_deleted, 0,
        "a notification inside the retention window survives"
    );

    sqlx::query("UPDATE notifications SET created_at = now() - interval '200 days'")
        .execute(&pool)
        .await
        .unwrap();
    let (_, notifications_deleted, _) = prune::prune_once(&pool).await.unwrap();
    assert_eq!(notifications_deleted, 1);
}

#[tokio::test]
async fn prune_never_touches_a_pending_outbox_row() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    outbox::record(&pool, &user_id, &fills(&workspace_id), day())
        .await
        .unwrap();
    sqlx::query("UPDATE notification_outbox SET created_at = now() - interval '90 days'")
        .execute(&pool)
        .await
        .unwrap();

    let (deleted, _, _) = prune::prune_once(&pool).await.unwrap();
    assert_eq!(deleted, 0, "an unprocessed event is still owed to the user");
}
