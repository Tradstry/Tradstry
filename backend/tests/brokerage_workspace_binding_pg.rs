mod pg_support;

use pg_support::{reset_schema, seed_user_workspace, test_pool};
use serde_json::json;
use tradstry_backend::service::brokerage::{
    client::SnapTradeAccount, workspaces::bind_workspace_brokerage_account,
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
        extra: json!({}),
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
