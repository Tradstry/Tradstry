mod pg_support;

use pg_support::{reset_schema, seed_user_workspace, test_pool};
use tradstry_backend::service::db::schema::tables::{
    brokerage_reconciliation_table::{self, PortfolioReconciliation, TransactionReconciliation},
    workspaces_table::{self, CreateWorkspaceInput},
};

async fn webull_workspace(
    pool: &sqlx::PgPool,
    user_id: &str,
    name: &str,
    account_id: &str,
) -> String {
    let workspace = workspaces_table::create_workspace(
        pool,
        user_id,
        CreateWorkspaceInput {
            name: name.to_string(),
            icon: "bank".to_string(),
            currency: "USD".to_string(),
            asset_class: "stocks".to_string(),
            broker: Some("Webull".to_string()),
            risk_profile: "moderate".to_string(),
        },
    )
    .await
    .unwrap();
    workspaces_table::set_snaptrade_account_id(pool, &workspace.id, user_id, account_id)
        .await
        .unwrap();
    workspace.id
}

#[tokio::test]
async fn webull_subaccounts_keep_independent_reconciliation_state() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, primary_workspace_id) = seed_user_workspace(&pool).await;
    workspaces_table::update_snaptrade_credentials(
        &pool,
        &primary_workspace_id,
        &user_id,
        "snaptrade-user",
        "encrypted-secret",
        Some("webull-connection"),
    )
    .await
    .unwrap();
    workspaces_table::set_snaptrade_account_id(
        &pool,
        &primary_workspace_id,
        &user_id,
        "webull-cash",
    )
    .await
    .unwrap();
    let margin_workspace_id =
        webull_workspace(&pool, &user_id, "Webull Margin", "webull-margin").await;
    let events_workspace_id =
        webull_workspace(&pool, &user_id, "Webull Events", "webull-events").await;

    for (workspace_id, account_id, broker_count, local_count) in [
        (&primary_workspace_id, "webull-cash", 18, 18),
        (&margin_workspace_id, "webull-margin", 41, 40),
        (&events_workspace_id, "webull-events", 3, 3),
    ] {
        brokerage_reconciliation_table::record_transaction_reconciliation(
            &pool,
            &user_id,
            workspace_id,
            account_id,
            &format!("diag-{account_id}"),
            &TransactionReconciliation {
                status: if broker_count == local_count {
                    "matched".to_string()
                } else {
                    "discrepancy".to_string()
                },
                broker_count,
                mapped_count: broker_count,
                imported_count: local_count,
                local_count,
                missing_count: broker_count - local_count,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }

    let cash = brokerage_reconciliation_table::get_for_workspace(
        &pool,
        &user_id,
        &primary_workspace_id,
        "webull-cash",
    )
    .await
    .unwrap()
    .unwrap();
    let margin = brokerage_reconciliation_table::get_for_workspace(
        &pool,
        &user_id,
        &margin_workspace_id,
        "webull-margin",
    )
    .await
    .unwrap()
    .unwrap();
    let events = brokerage_reconciliation_table::get_for_workspace(
        &pool,
        &user_id,
        &events_workspace_id,
        "webull-events",
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(cash.transaction_status, "matched");
    assert_eq!(cash.broker_transaction_count, 18);
    assert_eq!(margin.transaction_status, "discrepancy");
    assert_eq!(margin.missing_transaction_count, 1);
    assert_eq!(events.transaction_status, "matched");
    assert_eq!(events.broker_transaction_count, 3);
    assert!(
        brokerage_reconciliation_table::get_for_workspace(
            &pool,
            &user_id,
            &margin_workspace_id,
            "webull-cash",
        )
        .await
        .unwrap()
        .is_none(),
        "one Webull account must never leak into another workspace"
    );
}

#[tokio::test]
async fn transaction_and_portfolio_errors_are_preserved_independently() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    workspaces_table::update_snaptrade_credentials(
        &pool,
        &workspace_id,
        &user_id,
        "snaptrade-user",
        "encrypted-secret",
        Some("webull-connection"),
    )
    .await
    .unwrap();
    workspaces_table::set_snaptrade_account_id(&pool, &workspace_id, &user_id, "webull-cash")
        .await
        .unwrap();

    brokerage_reconciliation_table::record_transaction_reconciliation(
        &pool,
        &user_id,
        &workspace_id,
        "webull-cash",
        "diag-transactions",
        &TransactionReconciliation {
            status: "failed".to_string(),
            failed_count: 1,
            error: Some("transaction history timed out".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    brokerage_reconciliation_table::record_portfolio_reconciliation(
        &pool,
        &user_id,
        &workspace_id,
        "webull-cash",
        "diag-portfolio",
        &PortfolioReconciliation {
            status: "matched".to_string(),
            broker_holding_count: 2,
            mapped_holding_count: 2,
            local_holding_count: 2,
            broker_balance_count: 1,
            local_balance_count: 1,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let state = brokerage_reconciliation_table::get_for_workspace(
        &pool,
        &user_id,
        &workspace_id,
        "webull-cash",
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(state.transaction_status, "failed");
    assert_eq!(state.portfolio_status, "matched");
    assert_eq!(state.diagnostic_id, "diag-transactions");
    assert_eq!(
        state.transaction_error.as_deref(),
        Some("transaction history timed out")
    );
    assert_eq!(state.portfolio_error, None);
}
