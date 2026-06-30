mod pg_support;
use pg_support::{reset_schema, test_pool};

#[tokio::test]
async fn migrate_creates_all_tables_idempotently() {
    let pool = test_pool().await;
    reset_schema(&pool).await;

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
    ] {
        assert!(
            tables.contains(&expected.to_string()),
            "missing table {expected}"
        );
    }
    assert_eq!(
        tables.len(),
        24,
        "expected exactly 24 tables, got {tables:?}"
    );
}
