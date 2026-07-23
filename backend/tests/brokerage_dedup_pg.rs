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

/// Three rows sharing one `external_reference_id` and identical in every other
/// broker-provided field, arriving together in a single fetch.
///
/// Prod had a group like this that turned out to be one fill imported three
/// times by three separate sync runs, which migration 0027 collapses. Arriving
/// in *one* batch is the opposite case: the brokerage really is reporting three
/// rows, and nothing here can tell them apart, so all three must persist —
/// dropping two would silently destroy real fills. A resync must still find
/// three, not six.
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

/// The signature exists twice — once in Rust for the sync path, once in SQL for
/// migration 0027's backfill — and the two must produce byte-identical keys. If
/// they ever diverge, every backfilled row silently fails to match on the next
/// sync and the whole table duplicates, with no error to notice.
///
/// Recomputing the SQL expression here rather than asserting a frozen hash means
/// this catches drift on *either* side, and keeps catching it as fills gain
/// fields. The awkward values are the point: negative units, a padded option
/// symbol, mixed-case type, and a fill with no reference id all format
/// differently under `{:.8}` than under `to_char`, which is where the two
/// implementations would part ways first.
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

    let mut whole_units = fill("round-fill", Some("26197500726036"));
    whole_units.units = 1000.0;
    whole_units.price = 4.0;

    let mut refless = fill("refless-fill", None);
    refless.price = 0.0001;
    refless.transaction_type = "DiViDeNd".into();

    let mut no_trade_date = fill("undated-fill", Some("26197500726037"));
    no_trade_date.trade_date = None;

    brokerage_table::upsert_transactions(
        &pool,
        &user_id,
        &account_id,
        &[option_fill, whole_units, refless, no_trade_date],
        &mut counts(),
    )
    .await
    .unwrap();

    let mismatches: Vec<(String, String, String)> = sqlx::query(
        "SELECT snaptrade_id, dedup_key, \
                md5( \
                    COALESCE(lower(symbol), '') || '|' || \
                    COALESCE(to_char(trade_date AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS'), '') || '|' || \
                    to_char(COALESCE(units, 0), 'FM9999999999990.00000000') || '|' || \
                    to_char(COALESCE(price, 0), 'FM9999999999990.00000000') || '|' || \
                    COALESCE(lower(transaction_type), '') || '|' || \
                    COALESCE(external_reference_id, '') \
                ) || ':0' AS sql_key \
           FROM brokerage_transactions WHERE account_id = $1",
    )
    .bind(&account_id)
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| {
        (
            r.get::<String, _>("snaptrade_id"),
            r.get::<String, _>("dedup_key"),
            r.get::<String, _>("sql_key"),
        )
    })
    .filter(|(_, rust, sql)| rust != sql)
    .collect();

    assert!(
        mismatches.is_empty(),
        "Rust signature drifted from migration 0027's SQL: {mismatches:?}"
    );
}

/// Two fills alike in every attribute but their reference id are two trades, and
/// each must keep its own row across a resync that returns them in the opposite
/// order.
///
/// Under the pre-0027 key they shared a signature and were told apart only by a
/// `:0`/`:1` ordinal assigned in arrival order, so a reordered batch swapped
/// which row each fill upserted onto — quietly trading their amounts, fees and
/// reference ids, and leaving any journal link pointed at the other trade.
#[tokio::test]
async fn fills_differing_only_by_reference_keep_their_own_rows() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, account_id) = seed_user_account(&pool).await;

    let aaa = || {
        let mut f = fill("id-one", Some("order-aaa"));
        f.amount = Some(-425.0);
        f
    };
    let bbb = || {
        let mut f = fill("id-two", Some("order-bbb"));
        f.amount = Some(-999.0);
        f
    };

    brokerage_table::upsert_transactions(
        &pool,
        &user_id,
        &account_id,
        &[aaa(), bbb()],
        &mut counts(),
    )
    .await
    .unwrap();
    assert_eq!(row_count(&pool, &account_id).await, 2);

    // Same two fills, opposite order — the arrival-order swap that used to
    // scramble which row held which fill's data.
    brokerage_table::upsert_transactions(
        &pool,
        &user_id,
        &account_id,
        &[bbb(), aaa()],
        &mut counts(),
    )
    .await
    .unwrap();

    assert_eq!(
        row_count(&pool, &account_id).await,
        2,
        "a reordered resync must land on the same two rows"
    );

    let pairs: Vec<(String, f64)> = sqlx::query(
        "SELECT external_reference_id, amount FROM brokerage_transactions \
         WHERE account_id = $1 ORDER BY external_reference_id",
    )
    .bind(&account_id)
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| (r.get("external_reference_id"), r.get("amount")))
    .collect();

    assert_eq!(
        pairs,
        vec![
            ("order-aaa".to_string(), -425.0),
            ("order-bbb".to_string(), -999.0)
        ],
        "each fill's amount must stay with its own reference id"
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

/// Migration 0027's repair, against the shape prod was actually in: one fill
/// imported three times by three separate sync runs, each under a fresh
/// SnapTrade id, all three rows journal-linked to the same entry.
///
/// Replaying the migration over rows inserted this way must leave one row, move
/// the surviving link onto it, archive the two it removed, and leave a
/// same-signature group that carries *no* reference id alone — that group is
/// indistinguishable by attributes, so collapsing it would destroy real fills.
#[tokio::test]
async fn migration_collapses_cross_run_duplicates_and_repoints_links() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, account_id) = seed_user_account(&pool).await;

    let entry_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO journal_entries \
         (id, user_id, account_id, open_date, close_date, entry_price, exit_price, position_size, \
          symbol, symbol_name, status, total_pl, net_roi, duration, trade_type, mistakes, \
          entry_tactics, edges_spotted) \
         VALUES ($1, $2, $3, now(), now(), 4.25, 4.75, 1.0, 'BLZE','BLZE','profit', \
                 50.0, 11.7, 60, 'long','','','')",
    )
    .bind(&entry_id)
    .bind(&user_id)
    .bind(&account_id)
    .execute(&pool)
    .await
    .unwrap();

    // Inserted directly: these rows predate 0027, so they carry the old-style
    // key that the upsert path no longer produces.
    let mut dup_ids = Vec::new();
    for (n, snaptrade_id) in ["run-1", "run-2", "run-3"].iter().enumerate() {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO brokerage_transactions \
             (id, user_id, account_id, snaptrade_id, symbol, currency, transaction_type, \
              price, units, amount, fee, trade_date, settlement_date, institution, \
              external_reference_id, raw_json, contract_multiplier, dedup_key, created_at) \
             VALUES ($1, $2, $3, $4, 'BLZE', 'USD', 'BUY', 4.25, 100.0, -425.0, 0.0, \
                     '2026-07-15T14:30:00Z', '2026-07-17T00:00:00Z', 'Webull', \
                     'order-shared', '{}', 1.0, $5, now() + ($6 || ' minutes')::interval)",
        )
        .bind(&id)
        .bind(&user_id)
        .bind(&account_id)
        .bind(snaptrade_id)
        .bind(format!("legacy-signature:{n}"))
        .bind(n.to_string())
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO journal_brokerage_links \
             (id, journal_entry_id, brokerage_transaction_id, user_id) VALUES ($1, $2, $3, $4)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&entry_id)
        .bind(&id)
        .bind(&user_id)
        .execute(&pool)
        .await
        .unwrap();

        dup_ids.push(id);
    }

    // Two identical fills with no reference id: the case the ordinal still exists for.
    for (n, snaptrade_id) in ["refless-1", "refless-2"].iter().enumerate() {
        sqlx::query(
            "INSERT INTO brokerage_transactions \
             (id, user_id, account_id, snaptrade_id, symbol, currency, transaction_type, \
              price, units, amount, fee, trade_date, settlement_date, institution, \
              external_reference_id, raw_json, contract_multiplier, dedup_key) \
             VALUES ($1, $2, $3, $4, 'CASH', 'USD', 'INTEREST', 0.0, 1.0, 1.0, 0.0, \
                     '2026-07-15T00:00:00Z', '2026-07-15T00:00:00Z', 'Webull', \
                     '', '{}', 1.0, $5)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&user_id)
        .bind(&account_id)
        .bind(snaptrade_id)
        .bind(format!("legacy-refless:{n}"))
        .execute(&pool)
        .await
        .unwrap();
    }

    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/0027_brokerage_ref_dedup_key.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let surviving: Vec<String> = sqlx::query(
        "SELECT id FROM brokerage_transactions \
         WHERE external_reference_id = 'order-shared' AND account_id = $1",
    )
    .bind(&account_id)
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.get("id"))
    .collect();
    assert_eq!(
        surviving,
        vec![dup_ids[0].clone()],
        "the earliest row must survive and its two duplicates must be gone"
    );

    let refless_rows: i64 = sqlx::query(
        "SELECT count(*) FROM brokerage_transactions \
         WHERE external_reference_id = '' AND account_id = $1",
    )
    .bind(&account_id)
    .fetch_one(&pool)
    .await
    .unwrap()
    .get(0);
    assert_eq!(
        refless_rows, 2,
        "fills with no reference id are indistinguishable and must not be collapsed"
    );

    let links: Vec<String> = sqlx::query(
        "SELECT brokerage_transaction_id FROM journal_brokerage_links WHERE user_id = $1",
    )
    .bind(&user_id)
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.get("brokerage_transaction_id"))
    .collect();
    assert_eq!(
        links,
        vec![dup_ids[0].clone()],
        "the three links must collapse to one pointed at the surviving row"
    );

    let archived: i64 =
        sqlx::query("SELECT count(*) FROM brokerage_transactions_dedup_archive WHERE user_id = $1")
            .bind(&user_id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
    assert_eq!(
        archived, 2,
        "removed rows must be recoverable, not just gone"
    );

    let keys: Vec<String> =
        sqlx::query("SELECT dedup_key FROM brokerage_transactions WHERE account_id = $1")
            .bind(&account_id)
            .fetch_all(&pool)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.get("dedup_key"))
            .collect();
    assert!(
        keys.iter().all(|k| !k.starts_with("legacy-")),
        "every row must be re-keyed onto the new signature"
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
