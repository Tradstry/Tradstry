mod pg_support;

use pg_support::{reset_schema, seed_user_account, test_pool};
use sqlx::{PgPool, Row};
use tradstry_backend::service::db::schema::tables::brokerage_table::{
    self, NewBrokerageTransaction, SignatureCounts,
};

fn counts() -> SignatureCounts {
    SignatureCounts::new()
}
use uuid::Uuid;

fn fill(snaptrade_id: &str, external_reference_id: Option<&str>) -> NewBrokerageTransaction {
    NewBrokerageTransaction {
        snaptrade_id: snaptrade_id.to_string(),
        symbol: Some("BLZE".into()),
        symbol_description: Some("Backblaze".into()),
        raw_symbol: Some("BLZE".into()),
        currency: "USD".into(),
        transaction_type: "BUY".into(),
        option_type: None,
        price: 4.25,
        units: 100.0,
        amount: Some(-425.0),
        fee: 0.0,
        fx_rate: Some(1.0),
        description: Some("Bought 100 BLZE".into()),
        trade_date: Some("2026-07-15T14:30:00Z".into()),
        settlement_date: "2026-07-17T00:00:00Z".into(),
        institution: "Webull".into(),
        external_reference_id: external_reference_id.map(str::to_string),
        raw_json: "{}".into(),
        contract_multiplier: 1.0,
        underlying_symbol: None,
        option_kind: None,
        strike_price: None,
        option_expiration: None,
    }
}

async fn row_count(pool: &PgPool, account_id: &str) -> i64 {
    sqlx::query("SELECT count(*) FROM brokerage_transactions WHERE account_id = $1")
        .bind(account_id)
        .fetch_one(pool)
        .await
        .unwrap()
        .get::<i64, _>(0)
}

/// The regression this whole change exists for: SnapTrade regenerates
/// `snaptrade_id` when a user is deleted and re-registered. Keying the upsert on
/// it duplicated every fill and orphaned the journal links pointing at the old
/// rows.
#[tokio::test]
async fn reregistration_updates_in_place_and_preserves_journal_links() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, account_id) = seed_user_account(&pool).await;

    let tx = fill("snaptrade-uuid-before", Some("webull-order-6a579ff9"));
    brokerage_table::upsert_transactions(&pool, &user_id, &account_id, &[tx], &mut counts())
        .await
        .unwrap();

    let stored_id: String =
        sqlx::query("SELECT id FROM brokerage_transactions WHERE account_id = $1")
            .bind(&account_id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);

    // Journal the fill, exactly as the merge-trade flow does.
    let entry_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO journal_entries \
         (id, user_id, account_id, open_date, close_date, entry_price, exit_price, position_size, \
          symbol, symbol_name, status, total_pl, net_roi, duration, trade_type, mistakes, \
          entry_tactics, edges_spotted) \
         VALUES ($1, $2, $3, now(), now(), 4.25, 4.75, 100.0, 'BLZE','BLZE','profit', \
                 50.0, 11.7, 60, 'long','','','')",
    )
    .bind(&entry_id)
    .bind(&user_id)
    .bind(&account_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO journal_brokerage_links \
         (id, journal_entry_id, brokerage_transaction_id, user_id) VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&entry_id)
    .bind(&stored_id)
    .bind(&user_id)
    .execute(&pool)
    .await
    .unwrap();

    // Re-registration: same fill, brand new SnapTrade id.
    let after = fill("snaptrade-uuid-AFTER", Some("webull-order-6a579ff9"));
    brokerage_table::upsert_transactions(&pool, &user_id, &account_id, &[after], &mut counts())
        .await
        .unwrap();

    assert_eq!(
        row_count(&pool, &account_id).await,
        1,
        "re-registration must update the fill in place, not duplicate it"
    );

    let row =
        sqlx::query("SELECT id, snaptrade_id FROM brokerage_transactions WHERE account_id = $1")
            .bind(&account_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        row.get::<String, _>("id"),
        stored_id,
        "row identity must survive so journal links stay valid"
    );
    assert_eq!(
        row.get::<String, _>("snaptrade_id"),
        "snaptrade-uuid-AFTER",
        "the new SnapTrade id should still be recorded"
    );

    let linked: i64 = sqlx::query(
        "SELECT count(*) FROM journal_brokerage_links l \
         JOIN brokerage_transactions t ON t.id = l.brokerage_transaction_id \
         WHERE l.user_id = $1",
    )
    .bind(&user_id)
    .fetch_one(&pool)
    .await
    .unwrap()
    .get(0);
    assert_eq!(
        linked, 1,
        "the journal link must still resolve to a live row"
    );
}

/// Real prod data: three partial fills of one 3-contract SPY order, sharing one
/// `external_reference_id` and identical in every other broker-provided field
/// (same units, price, and trade_date to the microsecond). Only SnapTrade's
/// unstable `id` separated them.
///
/// All three must persist — collapsing them would silently destroy two real
/// fills — and a resync under new SnapTrade ids must still find three, not six.
#[tokio::test]
async fn identical_partial_fills_survive_and_resync_idempotently() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, account_id) = seed_user_account(&pool).await;

    let order_ref = Some("26197500726035");
    let batch: Vec<_> = ["fill-a", "fill-b", "fill-c"]
        .iter()
        .map(|id| fill(id, order_ref))
        .collect();
    brokerage_table::upsert_transactions(&pool, &user_id, &account_id, &batch, &mut counts())
        .await
        .unwrap();

    assert_eq!(
        row_count(&pool, &account_id).await,
        3,
        "three indistinguishable partial fills are three real trades"
    );

    // Re-registration: same three fills, all new SnapTrade ids.
    let after: Vec<_> = ["new-a", "new-b", "new-c"]
        .iter()
        .map(|id| fill(id, order_ref))
        .collect();
    brokerage_table::upsert_transactions(&pool, &user_id, &account_id, &after, &mut counts())
        .await
        .unwrap();

    assert_eq!(
        row_count(&pool, &account_id).await,
        3,
        "resync must map the three fills onto the existing rows, not duplicate them"
    );
}

/// The Rust signature and migration 0025's SQL expression must produce byte-identical
/// keys. If they ever diverge, every backfilled row silently fails to match on the
/// next sync and the whole table duplicates — with no error to notice.
///
/// The expected hash is md5 of the canonical signature string for this fill:
///   spy   260821c00754000|2026-07-16T16:15:15|-1.00000000|13.29000000|sell
#[tokio::test]
async fn rust_signature_matches_migration_sql() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, account_id) = seed_user_account(&pool).await;

    let mut option_fill = fill("spy-fill", Some("26197500726035"));
    option_fill.symbol = Some("SPY   260821C00754000".into());
    option_fill.trade_date = Some("2026-07-16T16:15:15Z".into());
    option_fill.units = -1.0;
    option_fill.price = 13.29;
    option_fill.transaction_type = "SELL".into();

    brokerage_table::upsert_transactions(
        &pool,
        &user_id,
        &account_id,
        &[option_fill],
        &mut counts(),
    )
    .await
    .unwrap();

    let key: String =
        sqlx::query("SELECT dedup_key FROM brokerage_transactions WHERE account_id = $1")
            .bind(&account_id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);

    assert_eq!(
        key, "410d14c3057f551b5139641f88443cea:0",
        "Rust signature drifted from the SQL in migration 0025"
    );
}

/// Pagination must not restart the ordinal: two fills of one order arriving in
/// separate pages would otherwise both claim `:0` and collide.
#[tokio::test]
async fn ordinals_stay_distinct_across_pages() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, account_id) = seed_user_account(&pool).await;

    let mut seen = counts();
    for id in ["page1-fill", "page2-fill"] {
        brokerage_table::upsert_transactions(
            &pool,
            &user_id,
            &account_id,
            &[fill(id, Some("shared-order"))],
            &mut seen,
        )
        .await
        .unwrap();
    }

    assert_eq!(
        row_count(&pool, &account_id).await,
        2,
        "a counter shared across pages must give the second fill ordinal :1"
    );
}

/// A repeated sync of the same single fill must not duplicate it.
#[tokio::test]
async fn repeated_sync_of_one_fill_stays_one_row() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, account_id) = seed_user_account(&pool).await;

    for snaptrade_id in ["id-one", "id-two"] {
        brokerage_table::upsert_transactions(
            &pool,
            &user_id,
            &account_id,
            &[fill(snaptrade_id, None)],
            &mut counts(),
        )
        .await
        .unwrap();
    }

    assert_eq!(
        row_count(&pool, &account_id).await,
        1,
        "re-syncing the same fill must update it, not duplicate it"
    );
}

/// The signature must not be so coarse that genuinely different fills merge —
/// that would silently delete a trade.
#[tokio::test]
async fn distinct_fills_remain_separate_rows() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, account_id) = seed_user_account(&pool).await;

    let first = fill("id-one", None);
    let mut second = fill("id-two", None);
    second.price = 4.26;

    brokerage_table::upsert_transactions(
        &pool,
        &user_id,
        &account_id,
        &[first, second],
        &mut counts(),
    )
    .await
    .unwrap();

    assert_eq!(
        row_count(&pool, &account_id).await,
        2,
        "fills differing in price are different trades and must both persist"
    );
}
