mod pg_support;

use pg_support::{reset_schema, seed_user_account, test_pool};
use tradstry_backend::service::db::schema::tables::brokerage_table;

/// The gate's whole purpose: once we've recorded that SnapTrade synced through a
/// date, seeing that same date again must not trigger another fetch. SnapTrade
/// advances this at most once a day, so without the gate we re-fetch ~14x daily
/// for data that cannot have changed.
#[tokio::test]
async fn records_and_reads_back_the_watermark() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, account_id) = seed_user_account(&pool).await;

    assert_eq!(
        brokerage_table::transactions_synced_through(&pool, &user_id, &account_id, "st-acct-1")
            .await
            .unwrap(),
        None,
        "an account we have never synced must read as stale"
    );

    brokerage_table::record_transactions_synced_through(
        &pool,
        &user_id,
        &account_id,
        "st-acct-1",
        "2026-07-19",
    )
    .await
    .unwrap();

    assert_eq!(
        brokerage_table::transactions_synced_through(&pool, &user_id, &account_id, "st-acct-1")
            .await
            .unwrap()
            .as_deref(),
        Some("2026-07-19")
    );
}

/// One Tradstry account can front several SnapTrade accounts — a Webull margin
/// and cash account, say — each syncing on its own schedule. Sharing a watermark
/// between them would let one account's advance suppress the other's fetch.
#[tokio::test]
async fn watermarks_are_tracked_per_snaptrade_account() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, account_id) = seed_user_account(&pool).await;

    brokerage_table::record_transactions_synced_through(
        &pool,
        &user_id,
        &account_id,
        "margin",
        "2026-07-19",
    )
    .await
    .unwrap();

    assert_eq!(
        brokerage_table::transactions_synced_through(&pool, &user_id, &account_id, "cash")
            .await
            .unwrap(),
        None,
        "the cash account must still read as stale"
    );
}

/// Re-registration regenerates SnapTrade's account ids, so the new ids find no
/// stored watermark and a full re-sync happens without any special-casing.
#[tokio::test]
async fn new_snaptrade_account_id_reads_as_stale() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, account_id) = seed_user_account(&pool).await;

    brokerage_table::record_transactions_synced_through(
        &pool,
        &user_id,
        &account_id,
        "cf46a59a-before-rereg",
        "2026-07-19",
    )
    .await
    .unwrap();

    assert_eq!(
        brokerage_table::transactions_synced_through(
            &pool,
            &user_id,
            &account_id,
            "6df20235-after-rereg"
        )
        .await
        .unwrap(),
        None,
        "a post-re-registration account id must force a full re-sync"
    );
}

/// Re-recording must overwrite rather than accumulate rows.
#[tokio::test]
async fn advancing_the_watermark_overwrites_in_place() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, account_id) = seed_user_account(&pool).await;

    for date in ["2026-07-18", "2026-07-19", "2026-07-20"] {
        brokerage_table::record_transactions_synced_through(
            &pool,
            &user_id,
            &account_id,
            "st-acct-1",
            date,
        )
        .await
        .unwrap();
    }

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM brokerage_sync_state")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        rows, 1,
        "advancing must update the row, not insert a new one"
    );

    assert_eq!(
        brokerage_table::transactions_synced_through(&pool, &user_id, &account_id, "st-acct-1")
            .await
            .unwrap()
            .as_deref(),
        Some("2026-07-20")
    );
}
