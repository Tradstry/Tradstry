mod pg_support;
use pg_support::{reset_schema, test_pool};

#[tokio::test]
async fn migrate_creates_all_tables_idempotently() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;

    // First run creates everything.
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .expect("first migrate");
    // Second run must be a no-op (IF NOT EXISTS / CREATE OR REPLACE), not an error.
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .expect("second migrate idempotent");

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_name <> '_sqlx_migrations' \
         ORDER BY table_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    for expected in [
        "users",
        "accounts",
        "brokerage_transactions",
        "brokerage_holdings",
        "brokerage_balances",
        "journal_entries",
        "playbooks",
        "notebook_folders",
        "notebook_notes",
        "notebook_note_trades",
        "notebook_images",
        "notebook_client_mutations",
        "notebook_note_crdt",
        "notebook_note_updates",
        "ai_jobs",
        "ai_source_documents",
        "ai_artifacts",
        "ai_artifact_sources",
        "journal_brokerage_links",
        "user_agents",
        "user_prompts",
        "position_calculator_rules",
        "position_calculator_history",
        "position_calculator_plans",
        "tag_categories",
        "tags",
        "trade_tags",
        "trading_principles",
        "trade_principle_violations",
        "price_history",
        "price_fetch_failures",
        "account_equity_history",
        "account_equity_rebuild",
    ] {
        assert!(
            tables.contains(&expected.to_string()),
            "missing table {expected}"
        );
    }
    assert_eq!(
        tables.len(),
        33,
        "expected exactly 33 tables, got {tables:?}"
    );
}

#[tokio::test]
async fn migration_adds_notebook_sync_columns() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .expect("migrate");

    let row: (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns
         WHERE table_name='notebook_notes' AND column_name='deleted_at')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(row.0, "notebook_notes.deleted_at missing");

    let row: (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables
         WHERE table_name='notebook_client_mutations')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(row.0, "notebook_client_mutations missing");
}
