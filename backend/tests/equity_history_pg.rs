mod pg_support;
use chrono::NaiveDate;
use pg_support::{reset_schema, seed_user_account, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::db::schema::tables::equity_table;
use tradstry_backend::service::equity::replay::{EquityPoint, ReplayHealth};

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn point(date: &str, cash: f64, positions_value: f64) -> EquityPoint {
    EquityPoint {
        date: d(date),
        cash,
        positions_value,
        equity: cash + positions_value,
        net_contributions: 1000.0,
        funding_adjusted_equity: cash + positions_value - 1000.0,
    }
}

#[tokio::test]
async fn replace_and_read_back_equity_history_is_idempotent() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    let pts = vec![
        point("2026-01-01", 1000.0, 0.0),
        point("2026-01-02", 500.0, 550.0),
    ];
    equity_table::replace_equity_history(&pool, &user_id, &account_id, &pts)
        .await
        .unwrap();

    let got = equity_table::equity_history(&pool, &user_id, &account_id, None)
        .await
        .unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(got[1].equity, 1050.0);
    assert_eq!(got[1].funding_adjusted_equity, 50.0);

    equity_table::replace_equity_history(&pool, &user_id, &account_id, &pts)
        .await
        .unwrap();
    let again = equity_table::equity_history(&pool, &user_id, &account_id, None)
        .await
        .unwrap();
    assert_eq!(again.len(), 2);
}

#[tokio::test]
async fn price_cache_upserts_and_reads_back() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;

    let rows = vec![
        ("AAPL".to_string(), d("2026-01-05"), 50.0),
        ("AAPL".to_string(), d("2026-01-06"), 55.0),
    ];
    equity_table::upsert_price_history(&pool, &rows)
        .await
        .unwrap();

    // A new split restates historical closes, so a re-write must overwrite, not skip.
    let restated = vec![("AAPL".to_string(), d("2026-01-06"), 110.0)];
    equity_table::upsert_price_history(&pool, &restated)
        .await
        .unwrap();

    let map = equity_table::cached_prices(
        &pool,
        &["AAPL".to_string()],
        d("2026-01-01"),
        d("2026-01-31"),
    )
    .await
    .unwrap();
    assert_eq!(map.len(), 2);
    assert_eq!(map[&("AAPL".to_string(), d("2026-01-06"))], 110.0);
}

#[tokio::test]
async fn rebuild_health_roundtrips_drift_and_unclassified_types() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    let health = ReplayHealth {
        unclassified_types: vec!["SOME_BROKER_THING".into()],
        excluded_option_txns: 2,
        foreign_currency_txns: 0,
        missing_price_days: 3,
    };
    equity_table::save_rebuild_health(
        &pool,
        &user_id,
        &account_id,
        Some(1050.0),
        Some(1000.0),
        &health,
        tradstry_backend::service::equity::REPLAY_VERSION,
    )
    .await
    .unwrap();

    let got = equity_table::rebuild_health(&pool, &user_id, &account_id)
        .await
        .unwrap()
        .expect("health row");
    assert_eq!(got.drift, Some(50.0));
    assert_eq!(got.health.unclassified_types, vec!["SOME_BROKER_THING"]);
    assert_eq!(got.health.missing_price_days, 3);

    // Manual account: no reported equity => drift is unknown, not zero.
    equity_table::save_rebuild_health(
        &pool,
        &user_id,
        &account_id,
        Some(1050.0),
        None,
        &health,
        tradstry_backend::service::equity::REPLAY_VERSION,
    )
    .await
    .unwrap();
    let got = equity_table::rebuild_health(&pool, &user_id, &account_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.drift, None);
}

#[tokio::test]
async fn a_dead_symbol_is_suppressed_after_repeated_misses_and_cleared_when_it_returns() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;

    let requested = vec!["AAPL".to_string(), "DEADCO".to_string()];
    let mut returned = std::collections::HashSet::new();
    returned.insert("AAPL".to_string());

    // Two misses is below the threshold, so DEADCO still gets retried.
    for _ in 0..2 {
        equity_table::record_fetch_outcome(&pool, &requested, &returned)
            .await
            .unwrap();
    }
    let suppressed = equity_table::suppressed_symbols(&pool, 3, 7).await.unwrap();
    assert!(suppressed.is_empty());

    // The third miss trips it.
    equity_table::record_fetch_outcome(&pool, &requested, &returned)
        .await
        .unwrap();
    let suppressed = equity_table::suppressed_symbols(&pool, 3, 7).await.unwrap();
    assert!(suppressed.contains("DEADCO"));
    // A symbol that keeps returning candles is never suppressed.
    assert!(!suppressed.contains("AAPL"));

    // If the feed starts serving it again, the suppression clears.
    returned.insert("DEADCO".to_string());
    equity_table::record_fetch_outcome(&pool, &requested, &returned)
        .await
        .unwrap();
    let suppressed = equity_table::suppressed_symbols(&pool, 3, 7).await.unwrap();
    assert!(suppressed.is_empty());
}
