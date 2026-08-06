mod pg_support;

use std::time::Duration;

use pg_support::{reset_schema, seed_user_workspace, test_pool};
use tokio::time::timeout;
use tradstry_backend::service::db::schema::tables::workspaces_table;
use tradstry_backend::service::db::schema::tables::workspaces_table::CreateWorkspaceInput;

async fn seed_two_workspaces() -> (sqlx::PgPool, String, String, String) {
    let pool = test_pool().await;
    let (user_id, first_workspace_id) = seed_user_workspace(&pool).await;
    let second = workspaces_table::create_workspace(
        &pool,
        &user_id,
        CreateWorkspaceInput {
            name: "Second Workspace".into(),
            icon: "chart-line-data-01".into(),
            currency: "USD".into(),
            asset_class: "mixed".into(),
            broker: None,
            risk_profile: "moderate".into(),
        },
    )
    .await
    .expect("create second workspace");

    (pool, user_id, first_workspace_id, second.id)
}

#[tokio::test]
async fn deletion_takes_the_per_user_lock() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .expect("migrate");
    let (pool, user_id, first_workspace_id, _) = seed_two_workspaces().await;

    let mut blocker = pool.begin().await.expect("begin blocker transaction");
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(&user_id)
        .execute(&mut *blocker)
        .await
        .expect("lock user");

    let delete_pool = pool.clone();
    let delete_user_id = user_id.clone();
    let mut deletion = tokio::spawn(async move {
        workspaces_table::delete_workspace(&delete_pool, &first_workspace_id, &delete_user_id).await
    });

    assert!(
        timeout(Duration::from_millis(100), &mut deletion)
            .await
            .is_err(),
        "deletion must wait while another transaction holds the user's lock"
    );

    blocker.rollback().await.expect("release user lock");
    assert!(deletion.await.expect("deletion task").expect("delete"));
}

#[tokio::test]
async fn concurrent_deletions_always_leave_one_workspace() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .expect("migrate");
    let (pool, user_id, first_workspace_id, second_workspace_id) = seed_two_workspaces().await;

    let first = workspaces_table::delete_workspace(&pool, &first_workspace_id, &user_id);
    let second = workspaces_table::delete_workspace(&pool, &second_workspace_id, &user_id);
    let (first_result, second_result) = tokio::join!(first, second);

    assert_eq!(
        usize::from(first_result.is_ok()) + usize::from(second_result.is_ok()),
        1,
        "exactly one deletion should succeed"
    );
    assert_eq!(
        workspaces_table::list_workspaces(&pool, &user_id)
            .await
            .expect("list remaining workspaces")
            .len(),
        1,
        "the final workspace must be preserved"
    );
}
