//! Replicache-style push/pull for the notebook.
//!
//! Push is idempotent by per-client sequential mutation id: the server ignores
//! any mutation whose id <= last_mutation_id. That converts an at-least-once
//! delivery channel into exactly-once *application*. Exactly-once *delivery* is
//! impossible over an unreliable channel (Two Generals).

use anyhow::{Result, bail};
use async_graphql::{Context, InputObject, Object, SimpleObject};
use serde::Deserialize;
use sqlx::{PgConnection, PgPool};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

use crate::service::db::schema::tables::notebook::crdt;
use crate::service::db::schema::tables::notebook::folders::{
    self, CreateNotebookFolderInput, MoveNotebookNodeInput, NotebookNodeType,
};
use crate::service::db::schema::tables::notebook::notes::{self, CreateNotebookNoteInput};
use crate::service::db::schema::tables::notebook::sync::{
    self, NotebookFolderDelta, NotebookNoteDelta,
};

/// `graphql(name)` is required: `notebook::NotebookMutation` is the notebook's
/// mutation root, and async-graphql panics at schema build on a duplicate type name.
#[derive(InputObject, Clone)]
#[graphql(name = "NotebookMutationInput")]
pub struct NotebookMutation {
    pub id: i64,
    pub name: String,
    /// JSON payload. Every generated id and timestamp is inside here, because
    /// mutations are replayed during rebase and must be deterministic.
    pub args: String,
    pub hlc: String,
}

#[derive(InputObject)]
pub struct NotebookPushInput {
    pub client_id: String,
    pub account_id: String,
    pub mutations: Vec<NotebookMutation>,
}

#[derive(SimpleObject)]
pub struct NotebookPushResult {
    pub last_mutation_id: i64,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotebookNoteDeltaGql {
    pub id: String,
    pub folder_id: Option<String>,
    pub title: String,
    pub document_json: String,
    pub sort_order: i64,
    pub trade_ids: Vec<String>,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

impl From<NotebookNoteDelta> for NotebookNoteDeltaGql {
    fn from(d: NotebookNoteDelta) -> Self {
        Self {
            id: d.id,
            folder_id: d.folder_id,
            title: d.title,
            document_json: d.document_json,
            sort_order: d.sort_order,
            trade_ids: d.trade_ids,
            hlc: d.hlc,
            deleted_at: d.deleted_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotebookFolderDeltaGql {
    pub id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

impl From<NotebookFolderDelta> for NotebookFolderDeltaGql {
    fn from(d: NotebookFolderDelta) -> Self {
        Self {
            id: d.id,
            parent_folder_id: d.parent_folder_id,
            name: d.name,
            sort_order: d.sort_order,
            hlc: d.hlc,
            deleted_at: d.deleted_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotebookPullResult {
    pub cookie: String,
    pub last_mutation_id: i64,
    pub notes: Vec<NotebookNoteDeltaGql>,
    pub folders: Vec<NotebookFolderDeltaGql>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateNoteArgs {
    id: String,
    account_id: String,
    document_json: String,
    #[serde(default)]
    trade_ids: Vec<String>,
    folder_id: Option<String>,
    /// base64 of the Y.Doc update the creating device minted for this note. The
    /// creator seeds; a note carrying one must not be seeded again by the server.
    seed_update: Option<String>,
    seed_state_vector: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppendNoteUpdateArgs {
    note_id: String,
    /// Standard padded base64. The desktop encodes with the same engine.
    update: String,
}

#[derive(Deserialize)]
struct IdArgs {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFolderArgs {
    id: String,
    account_id: String,
    name: String,
    parent_folder_id: Option<String>,
    sort_order: i64,
}

#[derive(Deserialize)]
struct RenameFolderArgs {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveNodeArgs {
    account_id: String,
    node_id: String,
    node_type: String,
    new_parent_folder_id: Option<String>,
    new_sort_order: i64,
}

/// Applies one mutation and returns the client's new `last_mutation_id`.
///
/// Both the effect and the cursor advance inside ONE transaction. Committing a
/// dedupe key separately from the operation leaves a TOCTOU window in which a
/// concurrent retry double-applies.
pub async fn apply_mutation(
    pool: &PgPool,
    user_id: &str,
    client_id: &str,
    m: &NotebookMutation,
) -> Result<i64> {
    let mut tx = pool.begin().await?;

    let last = sync::last_mutation_id(&mut tx, client_id, user_id).await?;
    if m.id <= last {
        tx.commit().await?;
        return Ok(last); // Already applied. Idempotent replay of an at-least-once delivery.
    }

    // A mutation that can never succeed must still be acknowledged, or the client
    // retries it forever and deadlocks its queue behind it. A SAVEPOINT isolates a
    // failed effect so the watermark can still advance in this same transaction.
    sqlx::query("SAVEPOINT apply_effect")
        .execute(&mut *tx)
        .await?;
    let effect_ok = match apply_effect(&mut tx, user_id, m).await {
        Ok(()) => {
            sqlx::query("RELEASE SAVEPOINT apply_effect")
                .execute(&mut *tx)
                .await?;
            true
        }
        Err(err) => {
            log::warn!(
                "dropping permanently invalid notebook mutation (client={client_id}, id={}, name={}): {err}",
                m.id,
                m.name
            );
            sqlx::query("ROLLBACK TO SAVEPOINT apply_effect")
                .execute(&mut *tx)
                .await?;
            false
        }
    };

    sync::advance_mutation_id(&mut tx, client_id, user_id, m.id).await?;
    tx.commit().await?;

    // Only seed here when the creator did not: the web resolver's notes arrive with
    // no seed, the desktop's carry their own. Seeding a note twice concatenates the
    // two Y.Docs, silently duplicating every paragraph.
    if effect_ok && m.name == "createNote" {
        let a: CreateNoteArgs = serde_json::from_str(&m.args)?;
        if a.seed_update.is_none() {
            seed_new_note(pool, &a.id).await;
        }
    }
    Ok(m.id)
}

/// Every note is born `crdt`. Seeding runs after the note's transaction commits,
/// because the projector is a subprocess and would pin a connection for the whole
/// ~130ms it takes.
///
/// A failure here is logged, not returned: the note row already exists, so failing
/// the caller would strand it. `seed_note` claims `seeding` before it can fail, and
/// `sweep_stale_seeding` re-drives anything left in that state.
pub async fn seed_new_note(pool: &PgPool, note_id: &str) {
    if let Err(error) = crdt::seed_note(pool, note_id).await {
        log::error!("note {note_id}: seeding failed, sweeper will retry: {error:#}");
    }
}

async fn apply_effect(conn: &mut PgConnection, user_id: &str, m: &NotebookMutation) -> Result<()> {
    match m.name.as_str() {
        "createNote" => {
            let a: CreateNoteArgs = serde_json::from_str(&m.args)?;
            let note_id = a.id.clone();
            notes::create_notebook_note_tx(
                conn,
                user_id,
                CreateNotebookNoteInput {
                    id: Some(a.id),
                    account_id: a.account_id,
                    document_json: a.document_json,
                    trade_ids: a.trade_ids,
                    folder_id: a.folder_id,
                },
                &m.hlc,
            )
            .await?;

            // The creator seeds. Installing it in this same transaction means the
            // note is never briefly `crdt` with an empty update log, which another
            // client would read as a legacy note.
            if let Some(seed) = a.seed_update {
                let update = BASE64.decode(seed.trim())?;
                let state_vector = match a.seed_state_vector {
                    Some(sv) => BASE64.decode(sv.trim())?,
                    None => bail!("createNote carried a seed update with no state vector"),
                };
                crdt::install_seed_tx(conn, &note_id, &update, &state_vector).await?;
            }
        }
        "appendNoteUpdate" => {
            let a: AppendNoteUpdateArgs = serde_json::from_str(&m.args)?;
            use anyhow::Context as _;
            use base64::Engine as _;
            let update = base64::engine::general_purpose::STANDARD
                .decode(&a.update)
                .context("appendNoteUpdate.update must be standard base64")?;
            super::crdt::append_updates_tx(
                conn,
                user_id,
                &a.note_id,
                std::slice::from_ref(&update),
            )
            .await?;
        }
        "deleteNote" => {
            let a: IdArgs = serde_json::from_str(&m.args)?;
            notes::delete_notebook_note_tx(conn, &a.id, user_id, &m.hlc).await?;
        }
        "createFolder" => {
            let a: CreateFolderArgs = serde_json::from_str(&m.args)?;
            folders::create_notebook_folder_tx(
                conn,
                CreateNotebookFolderInput {
                    id: Some(a.id),
                    user_id: user_id.to_string(),
                    account_id: a.account_id,
                    parent_folder_id: a.parent_folder_id,
                    name: a.name,
                },
                a.sort_order,
                &m.hlc,
            )
            .await?;
        }
        "renameFolder" => {
            let a: RenameFolderArgs = serde_json::from_str(&m.args)?;
            folders::rename_notebook_folder_tx(conn, &a.id, &a.name, &m.hlc).await?;
        }
        "deleteFolder" => {
            let a: IdArgs = serde_json::from_str(&m.args)?;
            folders::delete_notebook_folder_subtree_tx(conn, &a.id, &m.hlc).await?;
        }
        "moveNode" => {
            let a: MoveNodeArgs = serde_json::from_str(&m.args)?;
            let node_type = match a.node_type.as_str() {
                "folder" | "Folder" => NotebookNodeType::Folder,
                "note" | "Note" => NotebookNodeType::Note,
                other => anyhow::bail!("unknown node type '{other}'"),
            };
            folders::move_notebook_node_tx(
                conn,
                MoveNotebookNodeInput {
                    account_id: a.account_id,
                    node_id: a.node_id,
                    node_type,
                    new_parent_folder_id: a.new_parent_folder_id,
                    new_sort_order: a.new_sort_order,
                },
                &m.hlc,
            )
            .await?;
        }
        other => anyhow::bail!("unknown mutation '{other}'"),
    }
    Ok(())
}

#[derive(Default)]
pub struct NotebookSyncMutation;

#[Object]
impl NotebookSyncMutation {
    async fn push_notebook(
        &self,
        ctx: &Context<'_>,
        input: NotebookPushInput,
    ) -> async_graphql::Result<NotebookPushResult> {
        let user_db = super::base::get_user_db(ctx).await?;
        let pool = user_db.pool();
        let user_id = user_db.user_id();

        let mut sorted = input.mutations.clone();
        sorted.sort_by_key(|m| m.id);

        let mut conn = pool.acquire().await?;
        let mut last = sync::last_mutation_id(&mut conn, &input.client_id, user_id).await?;
        drop(conn);

        for m in &sorted {
            if m.id <= last {
                continue; // Already applied; apply_mutation would no-op anyway.
            }
            // Never skip a gap: applying id N when N-1 was never seen would advance
            // the watermark past a lost mutation, silently dropping it forever. Stop
            // and let the client resend the contiguous tail.
            if m.id > last + 1 {
                break;
            }
            last = apply_mutation(pool, user_id, &input.client_id, m).await?;
        }

        Ok(NotebookPushResult {
            last_mutation_id: last,
        })
    }
}

#[derive(Default)]
pub struct NotebookSyncQuery;

#[Object]
impl NotebookSyncQuery {
    async fn pull_notebook(
        &self,
        ctx: &Context<'_>,
        cookie: Option<String>,
        account_id: String,
        client_id: String,
    ) -> async_graphql::Result<NotebookPullResult> {
        let user_db = super::base::get_user_db(ctx).await?;
        let pool = user_db.pool();
        let user_id = user_db.user_id();

        let notes = sync::notes_since(pool, user_id, &account_id, cookie.as_deref()).await?;
        let folders = sync::folders_since(pool, user_id, &account_id, cookie.as_deref()).await?;

        // Opaque cursor: the max updated_at across returned rows, or the incoming
        // cookie echoed back when nothing changed. Fixed-width ISO microsecond
        // strings sort chronologically, so lexicographic max is correct.
        let mut next = cookie.unwrap_or_default();
        for updated_at in notes
            .iter()
            .map(|n| &n.updated_at)
            .chain(folders.iter().map(|f| &f.updated_at))
        {
            if *updated_at > next {
                next = updated_at.clone();
            }
        }

        let last_mutation_id = sync::last_mutation_id_for_client(pool, &client_id, user_id).await?;

        Ok(NotebookPullResult {
            cookie: next,
            last_mutation_id,
            notes: notes.into_iter().map(Into::into).collect(),
            folders: folders.into_iter().map(Into::into).collect(),
        })
    }
}
