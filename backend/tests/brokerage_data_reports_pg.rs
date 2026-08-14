mod pg_support;

use pg_support::{reset_schema, seed_user_workspace, test_pool};
use serde_json::json;
use tradstry_backend::service::db::schema::tables::{
    brokerage_data_report_table::{self, CreateBrokerageDataReport},
    workspaces_table,
};
use uuid::Uuid;

async fn create_report(
    pool: &sqlx::PgPool,
    user_id: &str,
    workspace_id: &str,
    snapshot: &serde_json::Value,
) -> anyhow::Result<brokerage_data_report_table::BrokerageDataReport> {
    let id = Uuid::new_v4().to_string();
    brokerage_data_report_table::create(
        pool,
        CreateBrokerageDataReport {
            id: &id,
            user_id,
            workspace_id,
            snaptrade_account_id: "webull-cash",
            diagnostic_id: "diag-cash",
            category: "balances",
            note: Some("Cash does not match Webull"),
            diagnostic_snapshot: snapshot,
        },
    )
    .await
}

async fn connect_workspace(pool: &sqlx::PgPool, user_id: &str, workspace_id: &str) {
    workspaces_table::update_snaptrade_credentials(
        pool,
        workspace_id,
        user_id,
        "snaptrade-user",
        "encrypted-secret",
        Some("webull-connection"),
    )
    .await
    .unwrap();
    workspaces_table::set_snaptrade_account_id(pool, workspace_id, user_id, "webull-cash")
        .await
        .unwrap();
}

#[tokio::test]
async fn report_is_scoped_to_the_connected_workspace_and_rate_limited() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    let (other_user_id, _) = seed_user_workspace(&pool).await;
    connect_workspace(&pool, &user_id, &workspace_id).await;

    let snapshot = json!({"version": 1, "sync": {"status": "completed"}});
    let report = create_report(&pool, &user_id, &workspace_id, &snapshot)
        .await
        .unwrap();
    assert_eq!(report.diagnostic_id, "diag-cash");

    assert!(
        create_report(&pool, &other_user_id, &workspace_id, &snapshot)
            .await
            .is_err(),
        "another user must not attach a report to this workspace",
    );

    for _ in 1..5 {
        create_report(&pool, &user_id, &workspace_id, &snapshot)
            .await
            .unwrap();
    }
    let limited = create_report(&pool, &user_id, &workspace_id, &snapshot)
        .await
        .unwrap_err();
    assert!(limited.to_string().contains("Too many reports"));
}
