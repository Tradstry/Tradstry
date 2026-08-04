mod pg_support;

use pg_support::{reset_schema, seed_user_account, test_pool};
use serde_json::json;
use tradstry_backend::service::brokerage::{
    accounts::materialize_connection_accounts, client::SnapTradeAccount,
};
use tradstry_backend::service::db::schema::tables::accounts_table;

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
async fn materializes_one_local_account_per_snaptrade_account() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, primary_id) = seed_user_account(&pool).await;

    accounts_table::update_snaptrade_credentials(
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

    let accounts = materialize_connection_accounts(&pool, &user_id, &primary_id, &upstream)
        .await
        .unwrap();

    assert_eq!(accounts.len(), 3);
    assert_eq!(accounts[0].id, primary_id);
    assert_eq!(accounts[0].snaptrade_account_id.as_deref(), Some("cash"));

    let stored = accounts_table::list_accounts(&pool, &user_id)
        .await
        .unwrap();
    assert_eq!(stored.len(), 3);
    assert_eq!(
        stored
            .iter()
            .filter_map(|account| account.snaptrade_account_id.as_deref())
            .collect::<Vec<_>>(),
        vec!["cash", "margin", "events"]
    );

    let repeated = materialize_connection_accounts(&pool, &user_id, &primary_id, &upstream)
        .await
        .unwrap();
    assert_eq!(repeated.len(), 3);
    assert_eq!(
        accounts_table::list_accounts(&pool, &user_id)
            .await
            .unwrap()
            .len(),
        3
    );
}
