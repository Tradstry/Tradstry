mod pg_support;
use pg_support::{reset_schema, seed_user_workspace, test_pool};

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
        "workspaces",
        "brokerage_connections",
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
        "plan_limits",
        "usage_counters",
        "paddle_webhook_events",
        "brokerage_sync_state",
        "brokerage_transactions_dedup_archive",
        "notification_outbox",
        "notifications",
        "notification_preferences",
        "push_subscriptions",
        "notification_deliveries",
        "notification_user_settings",
        "notification_schedule_runs",
        "market_watchlists",
        "market_watchlist_symbols",
        "market_reports",
        "market_monitors",
    ] {
        assert!(
            tables.contains(&expected.to_string()),
            "missing table {expected}"
        );
    }
    assert_eq!(
        tables.len(),
        50,
        "expected exactly 50 tables, got {tables:?}"
    );

    let indexes: Vec<String> = sqlx::query_scalar(
        "SELECT indexname FROM pg_indexes WHERE schemaname = 'public' AND indexname = ANY($1)",
    )
    .bind([
        "idx_journal_entries_user_workspace_created_live",
        "idx_journal_entries_user_workspace_close_live",
        "idx_brokerage_tx_user_workspace_date_id",
        "idx_brokerage_tx_workspace_type_date",
        "idx_brokerage_tx_symbol_search_trgm",
        "idx_notebook_notes_user_workspace_order_live",
        "idx_playbooks_user_workspace_created_live",
        "idx_principles_user_workspace_priority_live",
        "idx_position_history_user_workspace_created",
        "idx_position_plans_user_workspace_created",
    ])
    .fetch_all(&pool)
    .await
    .expect("query performance indexes");
    assert_eq!(indexes.len(), 10, "missing performance index: {indexes:?}");
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

#[tokio::test]
async fn notification_tables_exist_with_expected_columns() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .expect("migrate");

    for (table, column) in [
        ("notification_outbox", "coalesce_key"),
        ("notification_outbox", "attempts"),
        ("notification_outbox", "last_error"),
        ("notifications", "group_count"),
        ("notifications", "last_pushed_at"),
        ("notifications", "read_at"),
        ("notification_preferences", "enabled"),
        ("push_subscriptions", "p256dh"),
        ("notification_deliveries", "next_attempt_at"),
        ("notification_deliveries", "status"),
    ] {
        let found: Option<(String,)> = sqlx::query_as(
            "SELECT column_name FROM information_schema.columns \
             WHERE table_name = $1 AND column_name = $2",
        )
        .bind(table)
        .bind(column)
        .fetch_optional(&pool)
        .await
        .expect("query information_schema");
        assert!(found.is_some(), "{table}.{column} is missing");
    }
}

#[tokio::test]
async fn unread_coalesce_key_is_unique_per_user() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .expect("migrate");
    let (user_id, _account_id) = seed_user_workspace(&pool).await;

    let insert = |id: &'static str| {
        let pool = pool.clone();
        let user_id = user_id.clone();
        async move {
            sqlx::query(
                "INSERT INTO notifications (id, user_id, event_type, title, body, payload, coalesce_key) \
                 VALUES ($1, $2, 'FillsLanded', 't', 'b', '{}'::jsonb, 'fills:a:2026-07-28')",
            )
            .bind(id)
            .bind(&user_id)
            .execute(&pool)
            .await
        }
    };

    insert("n1").await.expect("first unread row inserts");
    assert!(
        insert("n2").await.is_err(),
        "a second unread row with the same coalesce key must be rejected"
    );

    sqlx::query("UPDATE notifications SET read_at = now() WHERE id = 'n1'")
        .execute(&pool)
        .await
        .expect("mark read");
    insert("n3")
        .await
        .expect("once the first is read, a new group may start");
}
