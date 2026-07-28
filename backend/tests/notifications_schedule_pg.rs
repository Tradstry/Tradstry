mod pg_support;
use chrono::{DateTime, TimeZone, Utc};
use pg_support::{reset_schema, seed_user_account, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::notifications::{schedule_worker, settings};

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
}

/// A BUY on the given day that no journal entry links to.
async fn seed_unjournaled_fill(pool: &PgPool, user_id: &str, account_id: &str, symbol: &str) {
    sqlx::query(
        "INSERT INTO brokerage_transactions \
           (id, user_id, account_id, snaptrade_id, symbol, transaction_type, price, units, \
            trade_date, settlement_date, institution, raw_json, dedup_key) \
         VALUES ($1, $2, $3, $1, $4, 'BUY', 10.0, 5.0, $5, $5, 'Webull', '{}', $1)",
    )
    .bind(format!("tx-{symbol}"))
    .bind(user_id)
    .bind(account_id)
    .bind(symbol)
    // 14:00 ET on 2026-07-28.
    .bind(utc(2026, 7, 28, 18, 0))
    .execute(pool)
    .await
    .expect("seed fill");
}

async fn set_recap_minute(pool: &PgPool, user_id: &str, minute: i16) {
    settings::upsert(
        pool,
        user_id,
        &settings::SettingsPatch {
            daily_recap_minute: Some(minute),
            ..Default::default()
        },
    )
    .await
    .expect("settings");
}

async fn outbox_count(pool: &PgPool) -> i64 {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notification_outbox")
        .fetch_one(pool)
        .await
        .expect("count");
    row.0
}

#[tokio::test]
async fn settings_default_when_no_row_exists() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_account(&pool).await;

    let s = settings::get(&pool, &user_id).await.expect("get");
    assert_eq!(s, settings::UserSettings::default());
}

#[tokio::test]
async fn settings_patch_leaves_untouched_fields_alone() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_account(&pool).await;

    settings::upsert(
        &pool,
        &user_id,
        &settings::SettingsPatch {
            timezone: Some("Asia/Tokyo".into()),
            ..Default::default()
        },
    )
    .await
    .expect("first");

    settings::upsert(
        &pool,
        &user_id,
        &settings::SettingsPatch {
            quiet_start_minute: Some(Some(1320)),
            ..Default::default()
        },
    )
    .await
    .expect("second");

    let s = settings::get(&pool, &user_id).await.expect("get");
    assert_eq!(s.timezone, "Asia/Tokyo");
    assert_eq!(s.quiet_start_minute, Some(1320));
    assert_eq!(s.daily_recap_minute, settings::DEFAULT_DAILY_RECAP_MINUTE);
}

#[tokio::test]
async fn recap_fires_once_and_only_once() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    seed_unjournaled_fill(&pool, &user_id, &account_id, "AAPL").await;
    // 14:00 ET so the tick below lands exactly on the slot.
    set_recap_minute(&pool, &user_id, 840).await;

    let now = utc(2026, 7, 28, 18, 0);
    schedule_worker::process_once(&pool, now).await.expect("tick");
    assert_eq!(outbox_count(&pool).await, 1);

    // A second tick inside the tolerance window must not double-send.
    schedule_worker::process_once(&pool, utc(2026, 7, 28, 18, 2))
        .await
        .expect("tick");
    assert_eq!(outbox_count(&pool).await, 1);
}

#[tokio::test]
async fn recap_is_silent_with_nothing_to_journal() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _account_id) = seed_user_account(&pool).await;

    set_recap_minute(&pool, &user_id, 840).await;

    schedule_worker::process_once(&pool, utc(2026, 7, 28, 18, 0))
        .await
        .expect("tick");
    assert_eq!(outbox_count(&pool).await, 0);
}

#[tokio::test]
async fn a_silent_slot_can_still_fire_later_the_same_day() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    set_recap_minute(&pool, &user_id, 840).await;

    // Nothing to say yet, so the slot must not be consumed.
    schedule_worker::process_once(&pool, utc(2026, 7, 28, 18, 0))
        .await
        .expect("tick");
    assert_eq!(outbox_count(&pool).await, 0);

    seed_unjournaled_fill(&pool, &user_id, &account_id, "TSLA").await;
    schedule_worker::process_once(&pool, utc(2026, 7, 28, 18, 3))
        .await
        .expect("tick");
    assert_eq!(outbox_count(&pool).await, 1);
}

#[tokio::test]
async fn nothing_fires_outside_the_slot() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    seed_unjournaled_fill(&pool, &user_id, &account_id, "AAPL").await;
    set_recap_minute(&pool, &user_id, 840).await;

    // 10:00 ET — hours before the slot.
    schedule_worker::process_once(&pool, utc(2026, 7, 28, 14, 0))
        .await
        .expect("tick");
    assert_eq!(outbox_count(&pool).await, 0);
}
