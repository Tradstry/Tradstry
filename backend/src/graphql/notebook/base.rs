use async_graphql::{Context, Enum, InputObject, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::db::schema::tables::notebook::folders::{
    self, MoveNotebookNodeInput as TableMoveNotebookNodeInput, NotebookFolder, NotebookNodeType,
};
use crate::service::db::schema::tables::notebook::notes::{
    CreateNotebookNoteInput, NotebookNote, UpdateNotebookNoteInput,
};
use crate::service::r2::R2Client;
use crate::service::read_service::notebook as notebook_service;
use crate::service::read_service::users::ensure_user;
use crate::service::{ai::jobs as ai_jobs, db::Db};

/// GraphQL-facing mirror of the table-layer `NotebookNodeType` enum.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum NotebookNodeTypeGql {
    Folder,
    Note,
}

impl From<NotebookNodeTypeGql> for NotebookNodeType {
    fn from(value: NotebookNodeTypeGql) -> Self {
        match value {
            NotebookNodeTypeGql::Folder => NotebookNodeType::Folder,
            NotebookNodeTypeGql::Note => NotebookNodeType::Note,
        }
    }
}

#[derive(InputObject)]
pub struct CreateNotebookFolderInput {
    pub id: Option<String>,
    pub workspace_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
}

#[derive(InputObject)]
pub struct MoveNotebookNodeInput {
    pub workspace_id: String,
    pub node_id: String,
    pub node_type: NotebookNodeTypeGql,
    pub new_parent_folder_id: Option<String>,
    pub new_sort_order: i64,
}

pub(super) async fn get_user_db(ctx: &Context<'_>) -> Result<crate::service::db::client::UserDb> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let db = ctx.data::<Arc<Db>>()?;
    let pool = db.pool();

    let full_name = jwt
        .other
        .get("full_name")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let email = jwt
        .other
        .get("email")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    let user = ensure_user(pool, &jwt.sub, full_name, email).await?;

    Ok(db.get_user_db(&user.id))
}

// Presigned R2 GET URLs expire; 7 days is the SigV4 max and is re-signed on
// every note fetch, so a normal session never sees expiry.
const PRESIGN_TTL: std::time::Duration = std::time::Duration::from_secs(604_800);

/// Overwrite each note image's (empty) `secure_url` with a freshly presigned
/// R2 GET URL derived from its object key (stored in `cloudinary_public_id`).
async fn presign_note_images(ctx: &Context<'_>, notes: &mut [NotebookNote]) -> Result<()> {
    let r2 = ctx.data::<Arc<R2Client>>()?;

    // Collect the indices + object keys of images that still need a URL. Only
    // R2-backed rows have an empty secure_url; rows still on Cloudinary keep
    // their existing URL until migrated.
    let mut targets: Vec<(usize, usize, String)> = Vec::new();
    for (note_idx, note) in notes.iter().enumerate() {
        for (img_idx, image) in note.images.iter().enumerate() {
            if image.secure_url.is_empty() {
                targets.push((note_idx, img_idx, image.cloudinary_public_id.clone()));
            }
        }
    }

    // Presign concurrently — each call is an independent R2 network round-trip.
    let urls = futures_util::future::join_all(
        targets
            .iter()
            .map(|(_, _, key)| r2.presigned_get_url(key, PRESIGN_TTL)),
    )
    .await;

    // Write successful URLs back by index; a presign error leaves the URL empty.
    for ((note_idx, img_idx, _), url) in targets.iter().zip(urls) {
        if let Ok(url) = url {
            notes[*note_idx].images[*img_idx].secure_url = url;
        }
    }

    Ok(())
}

#[derive(Default)]
pub struct NotebookQuery;

#[Object]
impl NotebookQuery {
    async fn notebook_notes(
        &self,
        ctx: &Context<'_>,
        workspace_id: Option<String>,
    ) -> Result<Vec<NotebookNote>> {
        let user_db = get_user_db(ctx).await?;
        let mut notes =
            notebook_service::list_notebook_notes(&user_db, workspace_id.as_deref()).await?;
        presign_note_images(ctx, &mut notes).await?;
        Ok(notes)
    }

    async fn notebook_note(&self, ctx: &Context<'_>, id: String) -> Result<Option<NotebookNote>> {
        let user_db = get_user_db(ctx).await?;
        let mut note = notebook_service::get_notebook_note(&user_db, &id).await?;
        if let Some(note) = note.as_mut() {
            presign_note_images(ctx, std::slice::from_mut(note)).await?;
        }
        Ok(note)
    }

    async fn notebook_folders(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<Vec<NotebookFolder>> {
        let user_db = get_user_db(ctx).await?;
        Ok(notebook_service::list_notebook_folders(&user_db, &workspace_id).await?)
    }
}

#[derive(Default)]
pub struct NotebookMutation;

#[Object]
impl NotebookMutation {
    async fn create_notebook_note(
        &self,
        ctx: &Context<'_>,
        input: CreateNotebookNoteInput,
    ) -> Result<NotebookNote> {
        let user_db = get_user_db(ctx).await?;
        let note = notebook_service::create_notebook_note(&user_db, input).await?;
        super::sync::seed_new_note(user_db.pool(), &note.id).await;
        let db = ctx.data::<Arc<Db>>()?;
        ai_jobs::enqueue_account_reindex(db.as_ref(), user_db.user_id(), &note.workspace_id)
            .await?;
        Ok(note)
    }

    async fn update_notebook_note(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdateNotebookNoteInput,
    ) -> Result<NotebookNote> {
        let user_db = get_user_db(ctx).await?;
        let note = notebook_service::update_notebook_note(&user_db, &id, input).await?;
        let db = ctx.data::<Arc<Db>>()?;
        ai_jobs::enqueue_account_reindex(db.as_ref(), user_db.user_id(), &note.workspace_id)
            .await?;
        Ok(note)
    }

    /// Toggle a note's star and/or pin flags. Metadata only — leave a flag null to keep it.
    /// No reindex: star/pin don't change the note's searchable content.
    async fn set_notebook_note_flags(
        &self,
        ctx: &Context<'_>,
        id: String,
        is_starred: Option<bool>,
        is_pinned: Option<bool>,
    ) -> Result<NotebookNote> {
        let user_db = get_user_db(ctx).await?;
        let note =
            notebook_service::set_notebook_note_flags(&user_db, &id, is_starred, is_pinned).await?;
        Ok(note)
    }

    async fn delete_notebook_note(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        // Fetch first so we have the note's media object keys before the DB
        // cascade removes the notebook::images rows.
        let existing = notebook_service::get_notebook_note(&user_db, &id).await?;
        let deleted = notebook_service::delete_notebook_note(&user_db, &id).await?;
        if deleted && let Some(note) = existing {
            // Best-effort R2 cleanup of the note's media, concurrently. The DB rows
            // are already gone via cascade; `cloudinary_public_id` holds the R2 key.
            let r2 = ctx.data::<Arc<R2Client>>()?;
            let results = futures_util::future::join_all(
                note.images
                    .iter()
                    .map(|image| r2.delete_object(&image.cloudinary_public_id)),
            )
            .await;
            for (image, result) in note.images.iter().zip(results) {
                if let Err(error) = result {
                    log::warn!(
                        "Failed to delete notebook media '{}' from R2 during note delete: {error}",
                        image.cloudinary_public_id
                    );
                }
            }

            let db = ctx.data::<Arc<Db>>()?;
            ai_jobs::enqueue_account_reindex(db.as_ref(), user_db.user_id(), &note.workspace_id)
                .await?;
        }
        Ok(deleted)
    }

    async fn create_notebook_folder(
        &self,
        ctx: &Context<'_>,
        input: CreateNotebookFolderInput,
    ) -> Result<NotebookFolder> {
        let user_db = get_user_db(ctx).await?;
        let table_input = folders::CreateNotebookFolderInput {
            id: input.id,
            user_id: user_db.user_id().to_string(),
            workspace_id: input.workspace_id,
            parent_folder_id: input.parent_folder_id,
            name: input.name,
        };
        Ok(notebook_service::create_notebook_folder(&user_db, table_input).await?)
    }

    async fn rename_notebook_folder(
        &self,
        ctx: &Context<'_>,
        id: String,
        name: String,
    ) -> Result<NotebookFolder> {
        let user_db = get_user_db(ctx).await?;
        notebook_service::rename_notebook_folder(&user_db, &id, &name).await?;
        // Return the freshly renamed folder via a direct lookup.
        folders::find_notebook_folder(user_db.pool(), &id)
            .await?
            .ok_or_else(|| async_graphql::Error::new("Notebook folder not found after rename"))
    }

    async fn delete_notebook_folder(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;

        // Look up the folder first so we can enqueue a reindex for its account.
        let folder = folders::find_notebook_folder(user_db.pool(), &id).await?;

        // Read-service cascades the DB delete and hands back R2 object keys.
        let object_keys = notebook_service::delete_notebook_folder(&user_db, &id).await?;

        // Best-effort R2 cleanup — log and continue on failure.
        let r2 = ctx.data::<Arc<R2Client>>()?;
        for object_key in object_keys {
            if let Err(error) = r2.delete_object(&object_key).await {
                log::warn!(
                    "Failed to delete notebook media '{object_key}' from R2 during folder delete: {error}"
                );
            }
        }

        // Enqueue a single account reindex if we know which account this was.
        if let Some(folder) = folder {
            let db = ctx.data::<Arc<Db>>()?;
            ai_jobs::enqueue_account_reindex(db.as_ref(), user_db.user_id(), &folder.workspace_id)
                .await?;
        }

        Ok(true)
    }

    async fn move_notebook_node(
        &self,
        ctx: &Context<'_>,
        input: MoveNotebookNodeInput,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        let table_input = TableMoveNotebookNodeInput {
            workspace_id: input.workspace_id,
            node_id: input.node_id,
            node_type: input.node_type.into(),
            new_parent_folder_id: input.new_parent_folder_id,
            new_sort_order: input.new_sort_order,
        };

        // Map the cycle-guard (and any other) anyhow error into a clean
        // async_graphql error so the client sees a readable message.
        notebook_service::move_notebook_node(&user_db, table_input)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(true)
    }
}
