mod pg_support;

use std::collections::HashMap;
use std::path::PathBuf;

use pg_support::{reset_schema, test_pool};
use sqlx::migrate::Migrator;
use sqlx::{AssertSqlSafe, PgPool};

const PRESERVED_TABLES: &[&str] = &[
    "journal_entries",
    "brokerage_transactions",
    "brokerage_holdings",
    "brokerage_balances",
    "notebook_folders",
    "notebook_notes",
    "notebook_images",
    "ai_jobs",
    "ai_source_documents",
    "ai_artifacts",
    "user_agents",
    "position_calculator_rules",
    "trading_principles",
    "account_equity_history",
    "account_equity_rebuild",
    "brokerage_sync_state",
    "position_calculator_history",
    "position_calculator_plans",
    "tag_categories",
    "tags",
    "trade_tags",
    "trade_principle_violations",
    "notebook_note_trades",
    "journal_brokerage_links",
    "ai_artifact_sources",
];

async fn table_count(pool: &PgPool, table: &str) -> i64 {
    sqlx::query_scalar(AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("count {table}: {error}"))
}

async fn snapshot_counts(pool: &PgPool) -> HashMap<&'static str, i64> {
    let mut counts = HashMap::new();
    for &table in PRESERVED_TABLES {
        counts.insert(table, table_count(pool, table).await);
    }
    counts
}

async fn seed_legacy_data(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO users (id, clerk_uuid, full_name, email, created_at) VALUES
            ('user-legacy', 'clerk-legacy', 'Legacy Trader', 'legacy@example.com', '2024-01-01'),
            ('user-empty', 'clerk-empty', 'Empty Trader', 'empty@example.com', '2024-01-02');

        INSERT INTO accounts (
            id, user_id, name, broker, snaptrade_user_id,
            snaptrade_user_secret_encrypted, snaptrade_connection_id,
            snaptrade_account_id, total_value, total_value_currency,
            snaptrade_connection_disabled, snaptrade_connection_disabled_at, created_at
        ) VALUES
            (
                'account-a', 'user-legacy', 'Primary Account', 'Webull', 'snap-user',
                'encrypted-secret-value', 'connection-a', 'snap-account-a',
                125000.50, 'USD', true, '2025-01-15', '2024-01-01'
            ),
            (
                'account-b', 'user-legacy', 'Options Account', NULL, NULL,
                NULL, NULL, NULL, NULL, NULL, false, NULL, '2024-02-01'
            );

        INSERT INTO playbooks (
            id, user_id, name, edge_name, entry_rules, exit_rules,
            position_sizing_rules, additional_rules
        ) VALUES
            ('playbook-shared', 'user-legacy', 'Shared Setup', 'Momentum', 'enter', 'exit', 'size', 'notes'),
            ('playbook-unused', 'user-legacy', 'Unused Setup', 'Reversal', 'enter', 'exit', 'size', NULL);

        INSERT INTO journal_entries (
            id, user_id, account_id, open_date, close_date, entry_price,
            exit_price, position_size, symbol, symbol_name, status, total_pl,
            net_roi, duration, stop_loss, risk_reward, trade_type, mistakes,
            entry_tactics, edges_spotted, playbook_id, notes
        ) VALUES
            (
                'trade-a', 'user-legacy', 'account-a', '2025-01-02', '2025-01-03', 100,
                110, 10, 'AAPL', 'Apple', 'profit', 100, 10, 1, 95, 2, 'long', '',
                'breakout', 'momentum', 'playbook-shared', 'primary trade'
            ),
            (
                'trade-b', 'user-legacy', 'account-b', '2025-02-02', '2025-02-03', 50,
                45, 5, 'SPY', 'SPDR S&P 500 ETF', 'loss', -25, -10, 1, 55, -1, 'short', 'late',
                'reversal', 'mean reversion', 'playbook-shared', 'options trade'
            );

        INSERT INTO brokerage_transactions (
            id, user_id, account_id, snaptrade_id, symbol, currency,
            transaction_type, price, units, fee, settlement_date, institution,
            external_reference_id, raw_json, dedup_key, trade_date
        ) VALUES (
            'brokerage-tx-a', 'user-legacy', 'account-a', 'snap-tx-a', 'AAPL', 'USD',
            'BUY', 100, 10, 1, '2025-01-04', 'Webull', 'ref-a', '{}', 'dedup-a', '2025-01-02'
        );
        INSERT INTO brokerage_holdings (
            id, user_id, account_id, symbol, currency, units, price, raw_json
        ) VALUES (
            'holding-a', 'user-legacy', 'account-a', 'AAPL', 'USD', 10, 110, '{}'
        );
        INSERT INTO brokerage_balances (
            id, user_id, account_id, currency, cash, buying_power
        ) VALUES ('balance-a', 'user-legacy', 'account-a', 'USD', 5000, 10000);

        INSERT INTO notebook_folders (id, user_id, account_id, name, sort_order)
        VALUES ('folder-a', 'user-legacy', 'account-a', 'Trade Reviews', 1);
        INSERT INTO notebook_notes (
            id, user_id, account_id, folder_id, sort_order, title, document_json
        ) VALUES (
            'note-a', 'user-legacy', 'account-a', 'folder-a', 1, 'AAPL Review', '{"type":"doc","content":[]}'
        );
        INSERT INTO notebook_images (
            id, note_id, user_id, account_id, cloudinary_asset_id,
            cloudinary_public_id, secure_url, width, height, format, bytes,
            original_filename, media_type, content_type, duration_seconds
        ) VALUES (
            'image-a', 'note-a', 'user-legacy', 'account-a', 'asset-a',
            'public-a', 'https://example.com/image.png', 800, 600, 'png', 1024,
            'chart.png', 'image', 'image/png', 0
        );
        INSERT INTO notebook_note_trades (note_id, trade_id)
        VALUES ('note-a', 'trade-a');

        INSERT INTO ai_jobs (
            id, user_id, account_id, job_type, payload_json, status
        ) VALUES ('job-a', 'user-legacy', 'account-a', 'reindex', '{}', 'queued');
        INSERT INTO ai_source_documents (
            id, user_id, account_id, source_type, source_id, title,
            body_text, metadata_json, content_hash
        ) VALUES (
            'source-a', 'user-legacy', 'account-a', 'trade', 'trade-a',
            'AAPL trade', 'body', '{}', 'hash-a'
        );
        INSERT INTO ai_artifacts (
            id, user_id, account_id, artifact_type, status, payload_json
        ) VALUES ('artifact-a', 'user-legacy', 'account-a', 'analysis', 'complete', '{}');
        INSERT INTO ai_artifact_sources (
            id, artifact_id, source_document_id, source_type, source_id, title, excerpt
        ) VALUES (
            'artifact-source-a', 'artifact-a', 'source-a', 'trade', 'trade-a', 'AAPL trade', 'excerpt'
        );
        INSERT INTO user_agents (
            id, user_id, account_id, name, goal, steps_json
        ) VALUES ('agent-a', 'user-legacy', 'account-a', 'Review Agent', 'Review trades', '[]');

        INSERT INTO position_calculator_rules (
            id, user_id, account_id, account_balance, account_risk, max_stop_loss_pct
        ) VALUES ('rule-a', 'user-legacy', 'account-a', 100000, 1, 5);
        INSERT INTO position_calculator_history (
            id, user_id, symbol, position_type, entry_price, stop_loss,
            account_balance, account_risk, shares, position_value,
            account_pct, stop_loss_pct
        ) VALUES (
            'history-a', 'user-legacy', 'AAPL', 'long', 100, 95,
            100000, 1, 200, 20000, 20, 5
        );
        INSERT INTO position_calculator_plans (
            id, user_id, symbol, position_type, entry_price, stop_loss,
            account_balance, account_risk, total_shares, position_value
        ) VALUES (
            'plan-a', 'user-legacy', 'MSFT', 'long', 400, 380,
            100000, 1, 50, 20000
        );

        INSERT INTO trading_principles (
            id, user_id, account_id, playbook_id, evidence_note_id,
            title, the_rule, why, priority, is_active
        ) VALUES (
            'principle-b', 'user-legacy', 'account-b', 'playbook-shared', 'note-a',
            'Wait for confirmation', 'Wait for close', 'Avoid false breakouts', 1, true
        );
        INSERT INTO trade_principle_violations (journal_entry_id, principle_id)
        VALUES ('trade-b', 'principle-b');

        INSERT INTO tag_categories (
            id, user_id, name, role, color, created_at, updated_at
        ) VALUES ('category-a', 'user-legacy', 'Mistakes', 'mistake', '#ff0000', now(), now());
        INSERT INTO tags (
            id, user_id, category_id, name, color, created_at, updated_at
        ) VALUES ('tag-a', 'user-legacy', 'category-a', 'Late entry', '#ff0000', now(), now());
        INSERT INTO trade_tags (journal_entry_id, tag_id) VALUES ('trade-a', 'tag-a');

        INSERT INTO journal_brokerage_links (
            id, journal_entry_id, brokerage_transaction_id, user_id
        ) VALUES ('link-a', 'trade-a', 'brokerage-tx-a', 'user-legacy');

        INSERT INTO account_equity_history (
            user_id, account_id, date, cash, positions_value, equity,
            net_contributions, funding_adjusted_equity
        ) VALUES ('user-legacy', 'account-a', '2025-01-03', 5000, 120000, 125000, 100000, 25000);
        INSERT INTO account_equity_rebuild (
            account_id, user_id, reconstructed_equity, reported_equity, drift, health_json
        ) VALUES ('account-a', 'user-legacy', 125000, 125000.50, 0.50, '{}');
        INSERT INTO brokerage_sync_state (
            user_id, account_id, snaptrade_account_id, transactions_last_successful_sync
        ) VALUES ('user-legacy', 'account-a', 'snap-account-a', '2025-01-03');
        "#,
    )
    .execute(pool)
    .await
    .expect("seed representative legacy data");
}

#[tokio::test]
async fn workspace_migration_preserves_legacy_production_data() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;

    let migrations_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let all_migrations = Migrator::new(migrations_path.as_path())
        .await
        .expect("load migrations");
    let legacy_migrations = Migrator::with_migrations(
        all_migrations
            .iter()
            .filter(|migration| migration.version <= 32)
            .cloned()
            .collect(),
    );
    legacy_migrations
        .run(&pool)
        .await
        .expect("migrate through legacy schema 0032");

    seed_legacy_data(&pool).await;
    let before = snapshot_counts(&pool).await;
    assert_eq!(table_count(&pool, "accounts").await, 2);
    assert_eq!(table_count(&pool, "playbooks").await, 2);

    all_migrations
        .run(&pool)
        .await
        .expect("apply workspace migration 0033");

    for (&table, &count) in &before {
        assert_eq!(
            table_count(&pool, table).await,
            count,
            "migration changed the row count for {table}"
        );
    }

    assert_eq!(table_count(&pool, "workspaces").await, 3);
    assert_eq!(table_count(&pool, "brokerage_connections").await, 1);
    assert_eq!(table_count(&pool, "playbooks").await, 3);

    let workspace_ids: Vec<String> = sqlx::query_scalar("SELECT id FROM workspaces ORDER BY id")
        .fetch_all(&pool)
        .await
        .expect("load workspace ids");
    assert!(workspace_ids.contains(&"account-a".to_string()));
    assert!(workspace_ids.contains(&"account-b".to_string()));
    assert_eq!(
        workspace_ids.len(),
        3,
        "the user without an account should receive one default workspace"
    );

    let brokerage: (String, String, String, String, f64, bool) = sqlx::query_as(
        "SELECT workspace_id, broker, snaptrade_user_id, \
         snaptrade_user_secret_encrypted, total_value, connection_disabled \
         FROM brokerage_connections WHERE workspace_id='account-a'",
    )
    .fetch_one(&pool)
    .await
    .expect("load migrated brokerage connection");
    assert_eq!(
        brokerage,
        (
            "account-a".into(),
            "Webull".into(),
            "snap-user".into(),
            "encrypted-secret-value".into(),
            125000.50,
            true,
        )
    );

    let renamed_scopes: Vec<(String, String)> =
        sqlx::query_as("SELECT id, workspace_id FROM journal_entries ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("load migrated trade scopes");
    assert_eq!(
        renamed_scopes,
        vec![
            ("trade-a".into(), "account-a".into()),
            ("trade-b".into(), "account-b".into()),
        ]
    );

    let global_scopes: (String, String, String, String) = sqlx::query_as(
        "SELECT \
            (SELECT workspace_id FROM position_calculator_history WHERE id='history-a'), \
            (SELECT workspace_id FROM position_calculator_plans WHERE id='plan-a'), \
            (SELECT workspace_id FROM tag_categories WHERE id='category-a'), \
            (SELECT workspace_id FROM tags WHERE id='tag-a')",
    )
    .fetch_one(&pool)
    .await
    .expect("load backfilled global scopes");
    assert_eq!(
        global_scopes,
        (
            "account-a".into(),
            "account-a".into(),
            "account-a".into(),
            "account-a".into(),
        )
    );

    let playbook_links: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT j.id, j.workspace_id, p.workspace_id \
         FROM journal_entries j JOIN playbooks p ON p.id=j.playbook_id \
         ORDER BY j.id",
    )
    .fetch_all(&pool)
    .await
    .expect("load migrated playbook links");
    assert_eq!(
        playbook_links,
        vec![
            ("trade-a".into(), "account-a".into(), "account-a".into()),
            ("trade-b".into(), "account-b".into(), "account-b".into()),
        ],
        "shared playbooks must be cloned and relinked inside each workspace"
    );

    let broken_links: i64 = sqlx::query_scalar(
        "SELECT \
            (SELECT COUNT(*) FROM notebook_note_trades nt \
             LEFT JOIN notebook_notes n ON n.id=nt.note_id \
             LEFT JOIN journal_entries j ON j.id=nt.trade_id \
             WHERE n.id IS NULL OR j.id IS NULL) + \
            (SELECT COUNT(*) FROM trade_tags tt \
             LEFT JOIN journal_entries j ON j.id=tt.journal_entry_id \
             LEFT JOIN tags t ON t.id=tt.tag_id \
             WHERE j.id IS NULL OR t.id IS NULL) + \
            (SELECT COUNT(*) FROM journal_brokerage_links l \
             LEFT JOIN journal_entries j ON j.id=l.journal_entry_id \
             LEFT JOIN brokerage_transactions b ON b.id=l.brokerage_transaction_id \
             WHERE j.id IS NULL OR b.id IS NULL) + \
            (SELECT COUNT(*) FROM ai_artifact_sources s \
             LEFT JOIN ai_artifacts a ON a.id=s.artifact_id \
             LEFT JOIN ai_source_documents d ON d.id=s.source_document_id \
             WHERE a.id IS NULL OR d.id IS NULL)",
    )
    .fetch_one(&pool)
    .await
    .expect("check migrated relationships");
    assert_eq!(broken_links, 0, "migration must preserve all seeded links");
}
