use tradstry_backend::service::db::schema::tables::{notebook::folders, notebook::notes};

mod pg_support;
use pg_support::{seed_user_account, test_pool};

const EMPTY_DOC: &str = r#"{"root":{"children":[]}}"#;

async fn make_note(
    pool: &sqlx::PgPool,
    user_id: &str,
    account_id: &str,
    folder_id: Option<String>,
) -> notes::NotebookNote {
    notes::create_notebook_note(
        pool,
        user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            account_id: account_id.to_string(),
            document_json: EMPTY_DOC.into(),
            trade_ids: vec![],
            folder_id,
        },
    )
    .await
    .expect("create note")
}

async fn make_folder(
    pool: &sqlx::PgPool,
    user_id: &str,
    account_id: &str,
    parent_folder_id: Option<String>,
) -> folders::NotebookFolder {
    folders::create_notebook_folder(
        pool,
        folders::CreateNotebookFolderInput {
            id: None,
            user_id: user_id.to_string(),
            account_id: account_id.to_string(),
            parent_folder_id,
            name: "Setups".into(),
        },
    )
    .await
    .expect("create folder")
}

#[tokio::test]
async fn deleted_note_is_hidden_from_every_read_path() {
    let pool = test_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    let note = make_note(&pool, &user_id, &account_id, None).await;

    assert!(
        notes::delete_notebook_note(&pool, &note.id, &user_id)
            .await
            .unwrap()
    );

    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM notebook_notes WHERE id = $1")
        .bind(&note.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "soft delete must retain the row as a tombstone");

    let listed = notes::list_notebook_notes(&pool, &user_id, Some(&account_id))
        .await
        .unwrap();
    assert!(
        listed.iter().all(|n| n.id != note.id),
        "list returned a deleted note"
    );

    let found = notes::find_notebook_note(&pool, &note.id, &user_id)
        .await
        .unwrap();
    assert!(found.is_none(), "find returned a deleted note");
}

#[tokio::test]
async fn double_delete_note_is_idempotent() {
    let pool = test_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    let note = make_note(&pool, &user_id, &account_id, None).await;

    assert!(
        notes::delete_notebook_note(&pool, &note.id, &user_id)
            .await
            .unwrap(),
        "first delete tombstones the row"
    );
    assert!(
        !notes::delete_notebook_note(&pool, &note.id, &user_id)
            .await
            .unwrap(),
        "second delete finds nothing live and returns false"
    );
}

#[tokio::test]
async fn deleted_folder_subtree_is_hidden() {
    let pool = test_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    let folder = make_folder(&pool, &user_id, &account_id, None).await;

    folders::delete_notebook_folder_subtree(&pool, &folder.id)
        .await
        .unwrap();

    let listed = folders::list_notebook_folders(&pool, &account_id)
        .await
        .unwrap();
    assert!(
        listed.iter().all(|f| f.id != folder.id),
        "list returned a deleted folder"
    );
    assert!(
        folders::find_notebook_folder(&pool, &folder.id)
            .await
            .unwrap()
            .is_none(),
        "find returned a deleted folder"
    );
}

#[tokio::test]
async fn deleting_folder_tombstones_notes_in_nested_subfolders() {
    let pool = test_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    let root = make_folder(&pool, &user_id, &account_id, None).await;
    let child = make_folder(&pool, &user_id, &account_id, Some(root.id.clone())).await;

    let direct_note = make_note(&pool, &user_id, &account_id, Some(root.id.clone())).await;
    let nested_note = make_note(&pool, &user_id, &account_id, Some(child.id.clone())).await;

    folders::delete_notebook_folder_subtree(&pool, &root.id)
        .await
        .unwrap();

    for note in [&direct_note, &nested_note] {
        let found = notes::find_notebook_note(&pool, &note.id, &user_id)
            .await
            .unwrap();
        assert!(
            found.is_none(),
            "note {} in deleted subtree must be hidden",
            note.id
        );

        let (count,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM notebook_notes WHERE id = $1 AND deleted_at IS NOT NULL",
        )
        .bind(&note.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "note {} must survive as a tombstone", note.id);
    }

    let folders = folders::list_notebook_folders(&pool, &account_id)
        .await
        .unwrap();
    assert!(
        folders.iter().all(|f| f.id != root.id && f.id != child.id),
        "nested folders must be hidden after subtree delete"
    );
}
