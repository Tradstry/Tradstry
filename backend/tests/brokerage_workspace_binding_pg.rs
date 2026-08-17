mod pg_support;

use pg_support::{reset_schema, seed_user_workspace, test_pool};
use std::collections::HashSet;
use tradstry_backend::service::brokerage::{
    client::SnapTradeAccount,
    workspaces::{bind_workspace_brokerage_account, create_workspaces_for_connection_accounts},
};
use tradstry_backend::service::db::schema::tables::workspaces_table;
use tradstry_backend::service::db::schema::tables::workspaces_table::CreateWorkspaceInput;

fn snaptrade_account(id: &str, name: &str, connection_id: &str) -> SnapTradeAccount {
    SnapTradeAccount {
        id: Some(id.to_string()),
        brokerage_authorization: Some(connection_id.to_string()),
        name: Some(name.to_string()),
        number: None,
        institution_name: Some("Webull".to_string()),
        sync_status: None,
    }
}

#[tokio::test]
async fn binds_only_one_snaptrade_account_to_the_selected_workspace() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, primary_id) = seed_user_workspace(&pool).await;

    workspaces_table::update_snaptrade_credentials(
        &pool,
        &primary_id,
        &user_id,
        "snaptrade-user",
        "encrypted-secret",
        Some("connection-1"),
    )
    .await
    .unwrap();

    let upstream = vec![
        snaptrade_account("cash", "Webull Individual Cash", "connection-1"),
        snaptrade_account("margin", "Webull Individual Margin", "connection-1"),
        snaptrade_account("events", "Webull Events Cash", "connection-1"),
        snaptrade_account("other-broker", "Other Brokerage", "connection-2"),
    ];

    let workspaces = bind_workspace_brokerage_account(&pool, &user_id, &primary_id, &upstream)
        .await
        .unwrap();

    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].id, primary_id);
    assert_eq!(workspaces[0].snaptrade_account_id.as_deref(), Some("cash"));
    assert_eq!(workspaces[0].broker.as_deref(), Some("Webull"));

    let stored = workspaces_table::list_workspaces(&pool, &user_id)
        .await
        .unwrap();
    assert_eq!(
        stored.len(),
        1,
        "brokerage discovery must not create workspaces"
    );
    assert_eq!(
        stored
            .iter()
            .filter_map(|workspace| workspace.snaptrade_account_id.as_deref())
            .collect::<Vec<_>>(),
        vec!["cash"]
    );

    let repeated = bind_workspace_brokerage_account(&pool, &user_id, &primary_id, &upstream)
        .await
        .unwrap();
    assert_eq!(repeated.len(), 1);
    assert_eq!(
        workspaces_table::list_workspaces(&pool, &user_id)
            .await
            .unwrap()
            .len(),
        1
    );

    workspaces_table::update_snaptrade_credentials(
        &pool,
        &primary_id,
        &user_id,
        "snaptrade-user",
        "encrypted-secret",
        Some("connection-2"),
    )
    .await
    .unwrap();
    let rebound = bind_workspace_brokerage_account(&pool, &user_id, &primary_id, &upstream)
        .await
        .unwrap();
    assert_eq!(
        rebound[0].snaptrade_account_id.as_deref(),
        Some("other-broker"),
        "changing the brokerage authorization must replace the old account binding"
    );
}

#[tokio::test]
async fn one_user_can_keep_different_brokerage_accounts_in_different_workspaces() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, options_workspace_id) = seed_user_workspace(&pool).await;
    let futures_workspace = workspaces_table::create_workspace(
        &pool,
        &user_id,
        CreateWorkspaceInput {
            name: "Futures".into(),
            icon: "chart-line-data-01".into(),
            currency: "USD".into(),
            asset_class: "futures".into(),
            broker: None,
            risk_profile: "moderate".into(),
        },
    )
    .await
    .unwrap();

    for workspace_id in [&options_workspace_id, &futures_workspace.id] {
        workspaces_table::update_snaptrade_credentials(
            &pool,
            workspace_id,
            &user_id,
            "snaptrade-user",
            "encrypted-secret",
            Some("connection-1"),
        )
        .await
        .unwrap();
    }

    bind_workspace_brokerage_account(
        &pool,
        &user_id,
        &options_workspace_id,
        &[snaptrade_account("options", "Options", "connection-1")],
    )
    .await
    .unwrap();
    bind_workspace_brokerage_account(
        &pool,
        &user_id,
        &futures_workspace.id,
        &[snaptrade_account("futures", "Futures", "connection-1")],
    )
    .await
    .unwrap();

    let stored = workspaces_table::list_workspaces(&pool, &user_id)
        .await
        .unwrap();
    assert_eq!(stored.len(), 2);
    assert_eq!(
        stored
            .iter()
            .find(|workspace| workspace.id == options_workspace_id)
            .and_then(|workspace| workspace.snaptrade_account_id.as_deref()),
        Some("options")
    );
    assert_eq!(
        stored
            .iter()
            .find(|workspace| workspace.id == futures_workspace.id)
            .and_then(|workspace| workspace.snaptrade_account_id.as_deref()),
        Some("futures")
    );
}

#[tokio::test]
async fn selected_brokerage_accounts_bind_matching_unlinked_workspaces() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, cash_workspace_id) = seed_user_workspace(&pool).await;

    workspaces_table::update_workspace(
        &pool,
        &cash_workspace_id,
        &user_id,
        workspaces_table::UpdateWorkspaceInput {
            name: Some("Cash Account".into()),
            icon: None,
            currency: None,
            asset_class: Some("stocks".into()),
            broker: Some("Webull".into()),
            risk_profile: None,
        },
    )
    .await
    .unwrap();
    workspaces_table::update_snaptrade_credentials(
        &pool,
        &cash_workspace_id,
        &user_id,
        "snaptrade-user",
        "encrypted-secret",
        Some("connection-1"),
    )
    .await
    .unwrap();
    workspaces_table::set_snaptrade_account_id(&pool, &cash_workspace_id, &user_id, "cash")
        .await
        .unwrap();

    let margin_workspace = workspaces_table::create_workspace(
        &pool,
        &user_id,
        CreateWorkspaceInput {
            name: "Webull Individual Margin".into(),
            icon: "chart-line-data-01".into(),
            currency: "USD".into(),
            asset_class: "mixed".into(),
            broker: None,
            risk_profile: "moderate".into(),
        },
    )
    .await
    .unwrap();
    let events_workspace = workspaces_table::create_workspace(
        &pool,
        &user_id,
        CreateWorkspaceInput {
            name: "Webull Events Cash".into(),
            icon: "chart-line-data-01".into(),
            currency: "USD".into(),
            asset_class: "mixed".into(),
            broker: None,
            risk_profile: "moderate".into(),
        },
    )
    .await
    .unwrap();

    let upstream = vec![
        snaptrade_account("cash", "Cash Account", "connection-1"),
        snaptrade_account("margin", "Webull Individual Margin", "connection-1"),
        snaptrade_account("events", "Webull Events Cash", "connection-1"),
    ];
    let requested = HashSet::from(["margin".to_string(), "events".to_string()]);

    let linked = create_workspaces_for_connection_accounts(
        &pool,
        &user_id,
        &cash_workspace_id,
        &upstream,
        &requested,
    )
    .await
    .unwrap();

    assert_eq!(linked.len(), 2);
    assert_eq!(
        workspaces_table::list_workspaces(&pool, &user_id)
            .await
            .unwrap()
            .len(),
        3,
        "matching unlinked workspaces should be attached, not duplicated"
    );

    let stored = workspaces_table::list_workspaces(&pool, &user_id)
        .await
        .unwrap();
    let margin = stored
        .iter()
        .find(|workspace| workspace.id == margin_workspace.id)
        .unwrap();
    assert_eq!(margin.snaptrade_user_id.as_deref(), Some("snaptrade-user"));
    assert_eq!(
        margin.snaptrade_connection_id.as_deref(),
        Some("connection-1")
    );
    assert_eq!(margin.snaptrade_account_id.as_deref(), Some("margin"));
    assert_eq!(margin.broker.as_deref(), Some("Webull"));

    let events = stored
        .iter()
        .find(|workspace| workspace.id == events_workspace.id)
        .unwrap();
    assert_eq!(events.snaptrade_account_id.as_deref(), Some("events"));
    assert_eq!(
        events.snaptrade_connection_id.as_deref(),
        Some("connection-1")
    );
}

#[tokio::test]
async fn sync_outcomes_keep_the_last_success_and_reject_stale_attempts() {
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
        Some("connection-1"),
    )
    .await
    .unwrap();

    workspaces_table::mark_brokerage_sync_started(&pool, &workspace_id, &user_id, "attempt-1")
        .await
        .unwrap();
    assert!(
        workspaces_table::mark_brokerage_sync_completed(
            &pool,
            &workspace_id,
            &user_id,
            "attempt-1",
            12,
            3,
            1,
        )
        .await
        .unwrap()
    );

    let first = workspaces_table::brokerage_sync_outcome(&pool, &workspace_id, &user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.status, "completed");
    assert_eq!(first.diagnostic_id.as_deref(), Some("attempt-1"));
    assert_eq!(first.transactions_synced, 12);
    assert_eq!(first.holdings_synced, 3);
    assert_eq!(first.balances_synced, 1);
    let first_success = first.succeeded_at.clone();
    assert!(first_success.is_some());

    workspaces_table::mark_brokerage_sync_started(&pool, &workspace_id, &user_id, "attempt-2")
        .await
        .unwrap();
    assert!(
        !workspaces_table::mark_brokerage_sync_completed(
            &pool,
            &workspace_id,
            &user_id,
            "attempt-1",
            99,
            99,
            99,
        )
        .await
        .unwrap(),
        "an older background task must not replace a newer attempt"
    );
    assert!(
        workspaces_table::mark_brokerage_sync_failed(
            &pool,
            &workspace_id,
            &user_id,
            "attempt-2",
            "provider refresh timed out",
        )
        .await
        .unwrap()
    );

    let failed = workspaces_table::brokerage_sync_outcome(&pool, &workspace_id, &user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.diagnostic_id.as_deref(), Some("attempt-2"));
    assert_eq!(failed.error.as_deref(), Some("provider refresh timed out"));
    assert_eq!(failed.succeeded_at, first_success);
    assert_eq!(failed.transactions_synced, 0);
    assert_eq!(failed.holdings_synced, 0);
    assert_eq!(failed.balances_synced, 0);
}
