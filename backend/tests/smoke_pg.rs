//! End-to-end smoke test: runs the converted sqlx/Postgres queries against a
//! real Postgres (docker-compose.test.yml) to catch SQL-level errors the Rust
//! compiler can't see — to_char timestamp formatting, ON CONFLICT targets,
//! $N bind counts, COUNT/SUM type decoding, and the bool/timestamptz columns.

mod pg_support;
use pg_support::{reset_schema, test_pool};

use tradstry_backend::service::db::schema::tables::{
    journal_table, tags_table, users_table, workspaces_table,
};

#[tokio::test]
async fn end_to_end_user_account_journal_tags() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .expect("migrate");

    // --- users: upsert path (ON CONFLICT clerk_uuid) ---
    let (user, created) = users_table::find_or_create_user(&pool, "clerk-smoke", "Jo", "jo@x.com")
        .await
        .expect("find_or_create_user");
    assert!(created, "first call creates the user");
    let (_again, created2) =
        users_table::find_or_create_user(&pool, "clerk-smoke", "Jo", "jo@x.com")
            .await
            .expect("find_or_create_user idempotent");
    assert!(!created2, "second call finds the existing user");

    // --- workspaces: create + read back the BOOLEAN + timestamptz columns ---
    let workspace = workspaces_table::create_default_workspace(&pool, &user.id)
        .await
        .expect("create_default_workspace");
    let workspaces = workspaces_table::list_workspaces(&pool, &user.id)
        .await
        .expect("list_workspaces");
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].id, workspace.id);
    assert!(
        !workspaces[0].snaptrade_connection_disabled,
        "bool column decodes and defaults false"
    );
    // created_at must come back as the ISO-8601 'Z' string from to_char.
    assert!(
        workspaces[0].created_at.ends_with('Z') && workspaces[0].created_at.contains('T'),
        "created_at formatted as ISO-8601 UTC, got {}",
        workspaces[0].created_at
    );

    // --- tags: ensure_default_categories (ON CONFLICT DO NOTHING), idempotent ---
    tags_table::ensure_default_categories(&pool, &user.id, &workspace.id)
        .await
        .expect("ensure_default_categories");
    tags_table::ensure_default_categories(&pool, &user.id, &workspace.id)
        .await
        .expect("ensure_default_categories idempotent");
    let cats = tags_table::list_categories(&pool, &user.id, &workspace.id)
        .await
        .expect("list_categories");
    assert!(!cats.is_empty(), "default categories seeded");

    // --- journal: create (timestamptz bind) + read back (to_char) + aggregate ---
    let input = journal_table::CreateJournalEntryInput {
        workspace_id: workspace.id.clone(),
        open_date: "2026-01-02T14:30:00Z".to_string(),
        close_date: "2026-01-05T15:00:00Z".to_string(),
        entry_price: 100.0,
        exit_price: 110.0,
        position_size: 10.0,
        symbol: "AAPL".to_string(),
        symbol_name: Some("Apple Inc.".to_string()), // provided => no network lookup
        stop_loss: 95.0,
        trade_type: "long".to_string(),
        playbook_id: None,
        notes: Some("smoke".to_string()),
        broke_30min_rule: None,
        pre_trade_conviction: None,
        market_regime: None,
        is_planned_pre_market: None,
        revenge_trade: None,
        rule_adherence_score: None,
        contract_multiplier: None,
        brokerage_transaction_ids: None,
        tag_ids: Vec::new(),
        violated_principle_ids: Vec::new(),
    };
    let entry = journal_table::create_journal_entry(&pool, &user.id, input)
        .await
        .expect("create_journal_entry");
    assert_eq!(entry.symbol, "AAPL");
    assert_eq!(entry.status, "profit");
    // open/close dates round-trip through TIMESTAMPTZ + to_char as ISO-8601 'Z'.
    assert_eq!(entry.open_date, "2026-01-02T14:30:00Z");
    assert_eq!(entry.close_date, "2026-01-05T15:00:00Z");

    let listed = journal_table::list_journal_entries(&pool, &user.id)
        .await
        .expect("list_journal_entries");
    assert_eq!(listed.len(), 1);

    // aggregate exercises COUNT(*)->i64, SUM(double)->f64, and the
    // `close_date >= $3` timestamptz comparison.
    let agg = journal_table::aggregate_journal_analytics(
        &pool,
        &user.id,
        &workspace.id,
        "2026-01-01T00:00:00Z",
        "2026-12-31T23:59:59Z",
    )
    .await
    .expect("aggregate_journal_analytics");
    assert_eq!(agg.total_trades, 1);
    assert_eq!(agg.winning_trades, 1);
    assert_eq!(agg.losing_trades, 0);
    assert!(agg.cumulative_profit > 0.0);
}
