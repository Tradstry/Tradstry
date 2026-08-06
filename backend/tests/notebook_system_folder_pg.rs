mod pg_support;
use pg_support::{reset_schema, seed_user_workspace, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::db::schema::tables::notebook::folders;

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

async fn system_folder(pool: &PgPool, workspace_id: &str) -> folders::NotebookFolder {
    folders::list_notebook_folders(pool, workspace_id)
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.is_system)
        .expect("account has a system folder")
}

#[tokio::test]
async fn every_account_is_provisioned_with_exactly_one_system_folder() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    // Backfill + create-time provisioning both run; neither may produce a duplicate.
    folders::ensure_system_folder(&pool, &user_id, &workspace_id)
        .await
        .unwrap();
    folders::ensure_system_folder(&pool, &user_id, &workspace_id)
        .await
        .unwrap();

    let all = folders::list_notebook_folders(&pool, &workspace_id)
        .await
        .unwrap();
    let system: Vec<_> = all.iter().filter(|f| f.is_system).collect();
    assert_eq!(system.len(), 1, "exactly one system folder per account");
    assert_eq!(system[0].name, folders::SYSTEM_FOLDER_NAME);
}

#[tokio::test]
async fn the_system_folder_cannot_be_renamed_or_deleted() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    folders::ensure_system_folder(&pool, &user_id, &workspace_id)
        .await
        .unwrap();

    let sys = system_folder(&pool, &workspace_id).await;

    // Enforced in the data layer, not the UI: the desktop sync path and any future MCP
    // write tool go through these same functions and must be refused too.
    assert!(
        folders::rename_notebook_folder(&pool, &sys.id, "Renamed")
            .await
            .is_err()
    );
    assert!(
        folders::delete_notebook_folder_subtree(&pool, &sys.id)
            .await
            .is_err()
    );

    let still_there = system_folder(&pool, &workspace_id).await;
    assert_eq!(still_there.name, folders::SYSTEM_FOLDER_NAME);
}

#[tokio::test]
async fn an_ordinary_folder_is_still_renamable_and_deletable() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let f = folders::create_notebook_folder(
        &pool,
        folders::CreateNotebookFolderInput {
            id: None,
            user_id: user_id.clone(),
            workspace_id: workspace_id.clone(),
            parent_folder_id: None,
            name: "Setups".into(),
        },
    )
    .await
    .unwrap();
    assert!(!f.is_system);

    folders::rename_notebook_folder(&pool, &f.id, "Setups 2026")
        .await
        .unwrap();
    folders::delete_notebook_folder_subtree(&pool, &f.id)
        .await
        .unwrap();

    let left = folders::list_notebook_folders(&pool, &workspace_id)
        .await
        .unwrap();
    assert!(left.iter().all(|x| x.id != f.id));
}
