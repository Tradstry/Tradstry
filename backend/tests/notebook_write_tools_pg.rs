//! End-to-end coverage of what the MCP notebook write tools actually do.
//!
//! The tools themselves are thin rmcp wrappers; everything that can go wrong lives in the
//! sequence below — the note-state branch, the Yjs delta, and keeping `document_json` in
//! step with the update chain. This drives that sequence against a real Postgres and a real
//! projector subprocess, exactly as `update_note` does.

mod pg_support;
use pg_support::{reset_schema, seed_user_workspace, test_pool};
use sqlx::PgPool;
use tradstry_backend::graphql::notebook::crdt as crdt_api;
use tradstry_backend::service::ai::projector::{self, EditMode};
use tradstry_backend::service::db::schema::tables::notebook::{
    crdt, folders,
    notes::{self, CreateNotebookNoteInput, UpdateNotebookNoteInput},
};

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

/// What `create_note` does: markdown in, note row out, filed in the System folder.
async fn create_note(pool: &PgPool, user_id: &str, workspace_id: &str, markdown: &str) -> String {
    let folder_id = folders::list_notebook_folders(pool, workspace_id)
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.is_system)
        .map(|f| f.id);

    let document_json = projector::markdown_to_json(markdown).await.unwrap();
    notes::create_notebook_note(
        pool,
        user_id,
        CreateNotebookNoteInput {
            id: None,
            workspace_id: workspace_id.to_string(),
            document_json,
            trade_ids: Vec::new(),
            folder_id,
        },
    )
    .await
    .unwrap()
    .id
}

/// What `update_note` does, including the branch on note state.
async fn update_note(
    pool: &PgPool,
    user_id: &str,
    note_id: &str,
    markdown: &str,
    mode: EditMode,
) -> Result<(), String> {
    let note = notes::find_notebook_note(pool, note_id, user_id)
        .await
        .unwrap()
        .expect("note");

    match crdt::note_state(pool, note_id).await.unwrap() {
        crdt::NoteState::Legacy => {
            let document_json = match mode {
                EditMode::Replace => projector::markdown_to_json(markdown).await.unwrap(),
                EditMode::Append => {
                    projector::append_markdown_to_json(&note.document_json, markdown)
                        .await
                        .unwrap()
                }
            };
            notes::update_notebook_note(
                pool,
                note_id,
                user_id,
                UpdateNotebookNoteInput {
                    workspace_id: None,
                    document_json: Some(document_json),
                    trade_ids: None,
                    folder_id: None,
                    expected_updated_at: None,
                },
            )
            .await
            .unwrap();
        }
        crdt::NoteState::Seeding => return Err("seeding".into()),
        crdt::NoteState::Crdt => {
            let history: Vec<Vec<u8>> = crdt_api::updates_since(pool, user_id, note_id, 0)
                .await
                .unwrap()
                .into_iter()
                .map(|(_, bytes)| bytes)
                .collect();

            let update = projector::apply_markdown(&history, markdown, mode)
                .await
                .unwrap();
            crdt_api::append_updates(pool, user_id, note_id, &[update])
                .await
                .unwrap();
            crdt::refresh_projection(pool, note_id).await.unwrap();
        }
    }
    Ok(())
}

async fn body(pool: &PgPool, user_id: &str, note_id: &str) -> String {
    notes::find_notebook_note(pool, note_id, user_id)
        .await
        .unwrap()
        .expect("note")
        .document_json
}

async fn title(pool: &PgPool, user_id: &str, note_id: &str) -> String {
    notes::find_notebook_note(pool, note_id, user_id)
        .await
        .unwrap()
        .expect("note")
        .title
}

#[tokio::test]
async fn create_note_files_markdown_into_the_system_folder_with_a_title_from_the_h1() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    folders::ensure_system_folder(&pool, &user_id, &workspace_id)
        .await
        .unwrap();

    let id = create_note(
        &pool,
        &user_id,
        &workspace_id,
        "# Weekly Report\n\nYou chased **three** entries.\n\n## Mistakes\n\n- AEHR\n- INOD\n",
    )
    .await;

    assert_eq!(title(&pool, &user_id, &id).await, "Weekly Report");

    let doc = body(&pool, &user_id, &id).await;
    assert!(doc.contains("Mistakes"), "markdown structure survived");
    assert!(doc.contains("AEHR"));

    // Filed into System, not left loose in the notebook.
    let note = notes::find_notebook_note(&pool, &id, &user_id)
        .await
        .unwrap()
        .unwrap();
    let system = folders::list_notebook_folders(&pool, &workspace_id)
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.is_system)
        .unwrap();
    assert_eq!(note.folder_id.as_deref(), Some(system.id.as_str()));
}

#[tokio::test]
async fn a_legacy_note_appends_without_losing_what_was_already_there() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let id = create_note(&pool, &user_id, &workspace_id, "# Log\n\nWeek 1.\n").await;
    update_note(
        &pool,
        &user_id,
        &id,
        "## Week 2\n\nWeek 2 body.\n",
        EditMode::Append,
    )
    .await
    .unwrap();

    let doc = body(&pool, &user_id, &id).await;
    assert!(doc.contains("Week 1."), "original content survived");
    assert!(doc.contains("Week 2 body."), "new content landed");
    assert_eq!(title(&pool, &user_id, &id).await, "Log");
}

#[tokio::test]
async fn a_legacy_note_replaces_its_whole_body() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let id = create_note(&pool, &user_id, &workspace_id, "# Old\n\nStale.\n").await;
    update_note(&pool, &user_id, &id, "# New\n\nFresh.\n", EditMode::Replace)
        .await
        .unwrap();

    let doc = body(&pool, &user_id, &id).await;
    assert!(doc.contains("Fresh."));
    assert!(!doc.contains("Stale."));
    assert_eq!(title(&pool, &user_id, &id).await, "New");
}

/// The real hazard. Once a client opens a note it is CRDT-backed: `document_json` is no
/// longer authoritative, and an edit must be a delta appended to the update chain. A doc
/// rebuilt from scratch would concatenate rather than conflict, doubling every paragraph.
#[tokio::test]
async fn a_crdt_note_is_edited_by_appending_a_delta_and_the_projection_keeps_up() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let id = create_note(
        &pool,
        &user_id,
        &workspace_id,
        "# Report\n\nOriginal line.\n",
    )
    .await;

    // This is what happens the first time the note is opened in the editor.
    crdt::seed_note(&pool, &id).await.unwrap();
    assert_eq!(
        crdt::note_state(&pool, &id).await.unwrap(),
        crdt::NoteState::Crdt
    );

    update_note(&pool, &user_id, &id, "Added line.\n", EditMode::Append)
        .await
        .unwrap();

    // `document_json` is refreshed from the chain, so the read tools and search see the edit.
    let doc = body(&pool, &user_id, &id).await;
    assert_eq!(
        doc.matches("Original line.").count(),
        1,
        "content duplicated: the edit rebuilt the doc instead of emitting a delta"
    );
    assert_eq!(
        doc.matches("Added line.").count(),
        1,
        "edit landed exactly once"
    );

    // And the chain itself really did grow — the edit is not a document_json-only write.
    let updates = crdt_api::updates_since(&pool, &user_id, &id, 0)
        .await
        .unwrap();
    assert!(
        updates.len() >= 2,
        "a delta was appended to the update chain"
    );
}

#[tokio::test]
async fn another_users_note_is_not_writable() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let id = create_note(&pool, &user_id, &workspace_id, "# Mine\n\nSecret.\n").await;

    // The tool resolves the note by (id, caller) before writing; a stranger simply
    // cannot see it, so there is nothing to write to.
    let seen = notes::find_notebook_note(&pool, &id, "some-other-user")
        .await
        .unwrap();
    assert!(seen.is_none());
}
