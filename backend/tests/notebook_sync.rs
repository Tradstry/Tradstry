use tradstry_backend::service::db::schema::tables::{
    notebook::folders, notebook::notes, notebook::sync,
};
use uuid::Uuid;

mod pg_support;
use pg_support::{seed_user_workspace, test_pool};

const EMPTY_DOC: &str = r#"{"root":{"children":[]}}"#;

#[tokio::test]
async fn client_supplied_id_is_used_verbatim() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    let wanted = Uuid::new_v4().to_string();

    let note = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: Some(wanted.clone()),
            workspace_id,
            document_json: EMPTY_DOC.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .expect("create");

    assert_eq!(note.id, wanted);
}

#[tokio::test]
async fn absent_id_is_minted_by_server() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let note = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            workspace_id,
            document_json: EMPTY_DOC.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .expect("create");

    assert_eq!(note.id.len(), 36, "expected a UUID");
}

#[tokio::test]
async fn invalid_client_supplied_note_id_is_rejected() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let result = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: Some("not-a-uuid".to_string()),
            workspace_id,
            document_json: EMPTY_DOC.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await;

    assert!(result.is_err(), "non-UUID client id must be rejected");
}

#[tokio::test]
async fn client_supplied_folder_id_is_used_verbatim() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    let wanted = Uuid::new_v4().to_string();

    let folder = folders::create_notebook_folder(
        &pool,
        folders::CreateNotebookFolderInput {
            id: Some(wanted.clone()),
            user_id,
            workspace_id,
            parent_folder_id: None,
            name: "Setups".into(),
        },
    )
    .await
    .expect("create");

    assert_eq!(folder.id, wanted);
}

#[tokio::test]
async fn invalid_client_supplied_folder_id_is_rejected() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let result = folders::create_notebook_folder(
        &pool,
        folders::CreateNotebookFolderInput {
            id: Some("not-a-uuid".to_string()),
            user_id,
            workspace_id,
            parent_folder_id: None,
            name: "Setups".into(),
        },
    )
    .await;

    assert!(result.is_err(), "non-UUID client id must be rejected");
}

#[tokio::test]
async fn pull_includes_tombstones() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let note = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            workspace_id: workspace_id.clone(),
            document_json: EMPTY_DOC.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap();

    notes::delete_notebook_note(&pool, &note.id, &user_id)
        .await
        .unwrap();

    let deltas = sync::notes_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();

    let found = deltas
        .iter()
        .find(|d| d.id == note.id)
        .expect("a deleted note must still appear in the delta feed");
    assert!(
        found.deleted_at.is_some(),
        "tombstone must carry deleted_at"
    );
}

#[tokio::test]
async fn cursor_excludes_unchanged_rows() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            workspace_id: workspace_id.clone(),
            document_json: EMPTY_DOC.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap();

    let first = sync::notes_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(first.len(), 1);
    let cookie = first.iter().map(|d| d.updated_at.clone()).max().unwrap();

    // The cursor is `>=`, so the boundary row is re-delivered rather than risking a
    // permanent skip of a row committed at the same microsecond. Re-delivery is safe:
    // the client's merge is idempotent. What must NOT appear is anything newer.
    let second = sync::notes_since(&pool, &user_id, &workspace_id, Some(&cookie))
        .await
        .unwrap();
    assert!(
        second.iter().all(|d| d.updated_at == cookie),
        "cursor returned a row older than the cookie",
    );
    assert!(
        second.len() <= first.len(),
        "cursor must not grow the result set when nothing changed",
    );
}

#[tokio::test]
async fn mutation_id_advances_and_is_idempotent() {
    let pool = test_pool().await;
    let (user_id, _account_id) = seed_user_workspace(&pool).await;
    let client_id = Uuid::new_v4().to_string();
    let mut tx = pool.begin().await.unwrap();

    assert_eq!(
        sync::last_mutation_id(&mut tx, &client_id, &user_id)
            .await
            .unwrap(),
        0
    );
    sync::advance_mutation_id(&mut tx, &client_id, &user_id, 5)
        .await
        .unwrap();
    assert_eq!(
        sync::last_mutation_id(&mut tx, &client_id, &user_id)
            .await
            .unwrap(),
        5
    );

    // Advancing backwards must be a no-op, not a regression.
    sync::advance_mutation_id(&mut tx, &client_id, &user_id, 3)
        .await
        .unwrap();
    assert_eq!(
        sync::last_mutation_id(&mut tx, &client_id, &user_id)
            .await
            .unwrap(),
        5
    );

    tx.commit().await.unwrap();
}

use serde_json::json;
use tradstry_backend::graphql::notebook::sync::{NotebookMutation, apply_mutation};

#[tokio::test]
async fn replayed_mutation_is_applied_once() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    let note_id = Uuid::new_v4().to_string();

    let m = NotebookMutation {
        id: 1,
        name: "createNote".into(),
        args: json!({
            "id": note_id,
            "workspaceId": workspace_id,
            "documentJson": EMPTY_DOC,
            "tradeIds": [],
            "folderId": null,
        })
        .to_string(),
        hlc: "000000000000001:00000:client-a".into(),
    };

    let client = Uuid::new_v4().to_string();
    // Apply the same batch twice, as an at-least-once channel would.
    let first = apply_mutation(&pool, &user_id, &client, &m).await.unwrap();
    let second = apply_mutation(&pool, &user_id, &client, &m).await.unwrap();
    assert_eq!(first, 1);
    assert_eq!(second, 1, "replay must not advance past the mutation id");

    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM notebook_notes WHERE id = $1")
        .bind(&note_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "replayed mutation created a duplicate note");
}

#[tokio::test]
async fn invalid_mutation_still_advances_cursor() {
    let pool = test_pool().await;
    let (user_id, _account_id) = seed_user_workspace(&pool).await;

    let bad = NotebookMutation {
        id: 1,
        name: "createNote".into(),
        args: "{ this is not json".into(),
        hlc: "000000000000001:00000:client-b".into(),
    };

    let client = Uuid::new_v4().to_string();
    let last = apply_mutation(&pool, &user_id, &client, &bad)
        .await
        .expect("an invalid mutation must not fail the batch");
    assert_eq!(
        last, 1,
        "a permanently invalid mutation must be acknowledged, or the client deadlocks"
    );
}

fn mutation(id: i64, name: &str, args: serde_json::Value, client: &str) -> NotebookMutation {
    NotebookMutation {
        id,
        name: name.into(),
        args: args.to_string(),
        hlc: format!("{id:015}:00000:{client}"),
    }
}

fn create_note_args(
    note_id: &str,
    workspace_id: &str,
    folder_id: Option<&str>,
) -> serde_json::Value {
    json!({
        "id": note_id,
        "workspaceId": workspace_id,
        "documentJson": EMPTY_DOC,
        "tradeIds": [],
        "folderId": folder_id,
    })
}

#[tokio::test]
async fn delete_note_mutation_tombstones() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    let client = Uuid::new_v4().to_string();
    let note_id = Uuid::new_v4().to_string();

    apply_mutation(
        &pool,
        &user_id,
        &client,
        &mutation(
            1,
            "createNote",
            create_note_args(&note_id, &workspace_id, None),
            &client,
        ),
    )
    .await
    .unwrap();
    apply_mutation(
        &pool,
        &user_id,
        &client,
        &mutation(2, "deleteNote", json!({ "id": note_id }), &client),
    )
    .await
    .unwrap();

    let (count,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notebook_notes WHERE id = $1 AND deleted_at IS NOT NULL",
    )
    .bind(&note_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "deleteNote must soft-delete the row");
}

#[tokio::test]
async fn create_folder_mutation_lands() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    let client = Uuid::new_v4().to_string();
    let folder_id = Uuid::new_v4().to_string();

    apply_mutation(
        &pool,
        &user_id,
        &client,
        &mutation(
            1,
            "createFolder",
            json!({ "id": folder_id, "workspaceId": workspace_id, "name": "Setups", "parentFolderId": null, "sortOrder": 3 }),
            &client,
        ),
    )
    .await
    .unwrap();

    let (name, sort_order): (String, i64) =
        sqlx::query_as("SELECT name, sort_order FROM notebook_folders WHERE id = $1")
            .bind(&folder_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(name, "Setups");
    assert_eq!(
        sort_order, 3,
        "createFolder must honor the client-supplied sortOrder"
    );
}

#[tokio::test]
async fn rename_folder_mutation_lands() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    let client = Uuid::new_v4().to_string();
    let folder_id = Uuid::new_v4().to_string();

    apply_mutation(
        &pool,
        &user_id,
        &client,
        &mutation(
            1,
            "createFolder",
            json!({ "id": folder_id, "workspaceId": workspace_id, "name": "Old", "parentFolderId": null, "sortOrder": 0 }),
            &client,
        ),
    )
    .await
    .unwrap();
    apply_mutation(
        &pool,
        &user_id,
        &client,
        &mutation(
            2,
            "renameFolder",
            json!({ "id": folder_id, "name": "New" }),
            &client,
        ),
    )
    .await
    .unwrap();

    let (name,): (String,) = sqlx::query_as("SELECT name FROM notebook_folders WHERE id = $1")
        .bind(&folder_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "New");
}

#[tokio::test]
async fn delete_folder_mutation_tombstones_subtree() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    let client = Uuid::new_v4().to_string();
    let folder_id = Uuid::new_v4().to_string();
    let note_id = Uuid::new_v4().to_string();

    apply_mutation(
        &pool,
        &user_id,
        &client,
        &mutation(
            1,
            "createFolder",
            json!({ "id": folder_id, "workspaceId": workspace_id, "name": "Doomed", "parentFolderId": null, "sortOrder": 0 }),
            &client,
        ),
    )
    .await
    .unwrap();
    apply_mutation(
        &pool,
        &user_id,
        &client,
        &mutation(
            2,
            "createNote",
            create_note_args(&note_id, &workspace_id, Some(&folder_id)),
            &client,
        ),
    )
    .await
    .unwrap();
    apply_mutation(
        &pool,
        &user_id,
        &client,
        &mutation(3, "deleteFolder", json!({ "id": folder_id }), &client),
    )
    .await
    .unwrap();

    let (folder_dead,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notebook_folders WHERE id = $1 AND deleted_at IS NOT NULL",
    )
    .bind(&folder_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (note_dead,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM notebook_notes WHERE id = $1 AND deleted_at IS NOT NULL",
    )
    .bind(&note_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(folder_dead, 1, "deleteFolder must tombstone the folder");
    assert_eq!(
        note_dead, 1,
        "deleteFolder must tombstone notes inside the folder"
    );
}

#[tokio::test]
async fn stale_update_is_rejected() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let note = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            workspace_id,
            document_json: r#"{"root":{"children":[]}}"#.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap();

    let stale = note.updated_at.clone();

    // Someone else writes.
    notes::update_notebook_note(
        &pool,
        &note.id,
        &user_id,
        notes::UpdateNotebookNoteInput {
            expected_updated_at: None,
            document_json: Some(r#"{"root":{"children":[1]}}"#.into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Our write, based on the old state, must not win.
    let err = notes::update_notebook_note(
        &pool,
        &note.id,
        &user_id,
        notes::UpdateNotebookNoteInput {
            expected_updated_at: Some(stale),
            document_json: Some(r#"{"root":{"children":[2]}}"#.into()),
            ..Default::default()
        },
    )
    .await;

    assert!(
        err.is_err(),
        "a stale write must be rejected, not silently applied"
    );
    assert!(
        err.unwrap_err().to_string().starts_with("CONFLICT:"),
        "conflict message must be machine-detectable by the web client"
    );

    // The other writer's content must survive the rejected stale write.
    let (doc,): (String,) =
        sqlx::query_as("SELECT document_json FROM notebook_notes WHERE id = $1")
            .bind(&note.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        doc.contains("[1]"),
        "stale write clobbered the winning write"
    );
}

#[tokio::test]
async fn update_without_expected_updated_at_still_succeeds() {
    // The desktop push path always passes `None`; it must never start failing.
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let note = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            workspace_id,
            document_json: r#"{"root":{"children":[]}}"#.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap();

    let updated = notes::update_notebook_note(
        &pool,
        &note.id,
        &user_id,
        notes::UpdateNotebookNoteInput {
            expected_updated_at: None,
            document_json: Some(r#"{"root":{"children":[9]}}"#.into()),
            ..Default::default()
        },
    )
    .await
    .expect("an update with no expected_updated_at must succeed");

    assert!(updated.document_json.contains("[9]"));
}

#[tokio::test]
async fn fresh_update_with_matching_expected_succeeds() {
    // Proves the updated_at round-trips at full precision: a write whose
    // expected_updated_at equals the stored value must be accepted. If the API
    // serialized updated_at to whole seconds, this would 409 every time.
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let note = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            workspace_id,
            document_json: r#"{"root":{"children":[]}}"#.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap();

    let updated = notes::update_notebook_note(
        &pool,
        &note.id,
        &user_id,
        notes::UpdateNotebookNoteInput {
            expected_updated_at: Some(note.updated_at.clone()),
            document_json: Some(r#"{"root":{"children":[7]}}"#.into()),
            ..Default::default()
        },
    )
    .await
    .expect("a write matching the current updated_at must be accepted");

    assert!(updated.document_json.contains("[7]"));
}

#[tokio::test]
async fn move_node_mutation_reparents() {
    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    let client = Uuid::new_v4().to_string();
    let folder_id = Uuid::new_v4().to_string();
    let note_id = Uuid::new_v4().to_string();

    apply_mutation(
        &pool,
        &user_id,
        &client,
        &mutation(
            1,
            "createFolder",
            json!({ "id": folder_id, "workspaceId": workspace_id, "name": "Target", "parentFolderId": null, "sortOrder": 0 }),
            &client,
        ),
    )
    .await
    .unwrap();
    apply_mutation(
        &pool,
        &user_id,
        &client,
        &mutation(
            2,
            "createNote",
            create_note_args(&note_id, &workspace_id, None),
            &client,
        ),
    )
    .await
    .unwrap();
    apply_mutation(
        &pool,
        &user_id,
        &client,
        &mutation(
            3,
            "moveNode",
            json!({ "workspaceId": workspace_id, "nodeId": note_id, "nodeType": "note", "newParentFolderId": folder_id, "newSortOrder": 0 }),
            &client,
        ),
    )
    .await
    .unwrap();

    let (folder_of_note,): (Option<String>,) =
        sqlx::query_as("SELECT folder_id FROM notebook_notes WHERE id = $1")
            .bind(&note_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(folder_of_note.as_deref(), Some(folder_id.as_str()));
}

/// The desktop enqueues CRDT body edits as `appendNoteUpdate`. Without a server arm
/// the push path treats it as an unknown mutation, logs, and STILL advances the
/// watermark — so the update is dropped and acked, and the client deletes it from
/// its outbox. The first synced keystroke on a crdt note would vanish silently.
#[tokio::test]
async fn append_note_update_mutation_is_applied_not_swallowed() {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    use tradstry_backend::service::db::schema::tables::notebook::crdt;

    let pool = test_pool().await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .unwrap();
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let note = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            workspace_id,
            document_json: EMPTY_DOC.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap();
    crdt::mark_crdt(&pool, &note.id, &[1, 2, 3]).await.unwrap();

    let blob: Vec<u8> = vec![0x00, 0xff, 0x41];
    let m = NotebookMutation {
        id: 1,
        name: "appendNoteUpdate".into(),
        args: json!({ "noteId": note.id, "update": B64.encode(&blob) }).to_string(),
        hlc: String::new(),
    };

    let client_id = Uuid::new_v4().to_string();
    apply_mutation(&pool, &user_id, &client_id, &m)
        .await
        .unwrap();

    let rows: Vec<(Vec<u8>,)> =
        sqlx::query_as("SELECT update FROM notebook_note_updates WHERE note_id = $1 ORDER BY seq")
            .bind(&note.id)
            .fetch_all(&pool)
            .await
            .unwrap();

    assert_eq!(rows.len(), 1, "appendNoteUpdate was swallowed, not applied");
    assert_eq!(rows[0].0, blob, "update bytes were mutated in transit");
}

#[tokio::test]
async fn a_note_created_through_sync_is_born_crdt() {
    use tradstry_backend::service::db::schema::tables::notebook::crdt::{self, NoteState};

    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    let note_id = Uuid::new_v4().to_string();

    let document_json = r#"{"root":{"type":"root","version":1,"direction":"ltr","format":"","indent":0,"children":[{"type":"paragraph","version":1,"direction":"ltr","format":"","indent":0,"children":[{"type":"text","text":"hello","format":0,"detail":0,"mode":"normal","style":"","version":1}]}]}}"#;

    let m = NotebookMutation {
        id: 1,
        name: "createNote".into(),
        args: json!({
            "id": note_id,
            "workspaceId": workspace_id,
            "documentJson": document_json,
            "tradeIds": [],
            "folderId": null,
        })
        .to_string(),
        hlc: "000000000000001:00000:client-seed".into(),
    };

    let client = Uuid::new_v4().to_string();
    apply_mutation(&pool, &user_id, &client, &m).await.unwrap();

    assert_eq!(
        crdt::note_state(&pool, &note_id).await.unwrap(),
        NoteState::Crdt,
        "a note must be born crdt; nothing else ever seeds it"
    );

    let (updates,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM notebook_note_updates WHERE note_id = $1")
            .bind(&note_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(updates, 1, "seeding must append exactly one update");
}

/// The desktop mints its own seed offline. The server must install that exact blob
/// and must not seed a second Y.Doc: two seeds concatenate, silently duplicating
/// every paragraph.
#[tokio::test]
async fn a_client_supplied_seed_is_installed_and_not_reseeded() {
    use tradstry_backend::service::db::schema::tables::notebook::crdt::{self, NoteState};

    let pool = test_pool().await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    let note_id = Uuid::new_v4().to_string();

    let seed = b"pretend-yjs-update";
    let state_vector = b"pretend-state-vector";
    let b64 = |b: &[u8]| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b);

    let m = NotebookMutation {
        id: 1,
        name: "createNote".into(),
        args: json!({
            "id": note_id,
            "workspaceId": workspace_id,
            "documentJson": EMPTY_DOC,
            "tradeIds": [],
            "folderId": null,
            "seedUpdate": b64(seed),
            "seedStateVector": b64(state_vector),
        })
        .to_string(),
        hlc: "000000000000001:00000:client-seeder".into(),
    };

    // Same client twice: deduped by mutation id, so this never re-enters the effect.
    let client = Uuid::new_v4().to_string();
    apply_mutation(&pool, &user_id, &client, &m).await.unwrap();
    apply_mutation(&pool, &user_id, &client, &m).await.unwrap();

    // A *different* client with the same createNote does re-enter the effect — a
    // rebase or a second device replaying the note's creation. This is the path that
    // can seed a note twice, and the one the ON CONFLICT guard exists for.
    let other = Uuid::new_v4().to_string();
    apply_mutation(&pool, &user_id, &other, &m).await.unwrap();

    assert_eq!(
        crdt::note_state(&pool, &note_id).await.unwrap(),
        NoteState::Crdt,
    );

    let rows: Vec<(Vec<u8>,)> =
        sqlx::query_as("SELECT update FROM notebook_note_updates WHERE note_id = $1 ORDER BY seq")
            .bind(&note_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 1, "a note must be seeded exactly once");
    assert_eq!(
        rows[0].0, seed,
        "the creator's seed must be stored verbatim"
    );
}

/// A pull's ack must be scoped to the asking device. With a user-wide `MAX`, device
/// A (at mutation 1) would be told device B's 9 was applied; a client that trusted
/// it would truncate its outbox and discard mutations the server never saw.
#[tokio::test]
async fn pull_acks_only_the_asking_clients_mutations() {
    let pool = test_pool().await;
    let (user_id, _account_id) = seed_user_workspace(&pool).await;

    let device_a = Uuid::new_v4().to_string();
    let device_b = Uuid::new_v4().to_string();

    let mut tx = pool.begin().await.unwrap();
    sync::advance_mutation_id(&mut tx, &device_a, &user_id, 1)
        .await
        .unwrap();
    sync::advance_mutation_id(&mut tx, &device_b, &user_id, 9)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let a = sync::last_mutation_id_for_client(&pool, &device_a, &user_id)
        .await
        .unwrap();
    let b = sync::last_mutation_id_for_client(&pool, &device_b, &user_id)
        .await
        .unwrap();

    assert_eq!(a, 1, "device A must not be acked device B's mutations");
    assert_eq!(b, 9);

    // A device the server has never seen is at zero, not at the user's maximum.
    let fresh = sync::last_mutation_id_for_client(&pool, &Uuid::new_v4().to_string(), &user_id)
        .await
        .unwrap();
    assert_eq!(fresh, 0, "an unknown device must start from zero");
}
