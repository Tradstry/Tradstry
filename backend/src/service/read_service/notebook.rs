use anyhow::Result;

use crate::service::turso::client::UserDb;
use crate::service::turso::schema::tables::notebook_folders_table::{
    self, CreateNotebookFolderInput, MoveNotebookNodeInput, NotebookFolder,
};
use crate::service::turso::schema::tables::notebook_images::{self, NotebookImage};
use crate::service::turso::schema::tables::notebook_table::{
    self, CreateNotebookNoteInput, NotebookNote, UpdateNotebookNoteInput,
};

/// Find a single notebook media item (image or video) by id, scoped to the user.
pub async fn find_notebook_image(
    user_db: &UserDb,
    media_id: &str,
) -> Result<Option<NotebookImage>> {
    notebook_images::find_notebook_image(user_db.conn(), media_id, user_db.user_id()).await
}

pub async fn list_notebook_notes(
    user_db: &UserDb,
    account_id: Option<&str>,
) -> Result<Vec<NotebookNote>> {
    notebook_table::list_notebook_notes(user_db.conn(), user_db.user_id(), account_id).await
}

pub async fn get_notebook_note(user_db: &UserDb, id: &str) -> Result<Option<NotebookNote>> {
    notebook_table::find_notebook_note(user_db.conn(), id, user_db.user_id()).await
}

pub async fn create_notebook_note(
    user_db: &UserDb,
    input: CreateNotebookNoteInput,
) -> Result<NotebookNote> {
    notebook_table::create_notebook_note(user_db.conn(), user_db.user_id(), input).await
}

pub async fn update_notebook_note(
    user_db: &UserDb,
    id: &str,
    input: UpdateNotebookNoteInput,
) -> Result<NotebookNote> {
    notebook_table::update_notebook_note(user_db.conn(), id, user_db.user_id(), input).await
}

pub async fn delete_notebook_note(user_db: &UserDb, id: &str) -> Result<bool> {
    notebook_table::delete_notebook_note(user_db.conn(), id, user_db.user_id()).await
}

pub async fn list_notebook_folders(
    user_db: &UserDb,
    account_id: &str,
) -> Result<Vec<NotebookFolder>> {
    notebook_folders_table::list_notebook_folders(user_db.conn(), account_id).await
}

pub async fn create_notebook_folder(
    user_db: &UserDb,
    input: CreateNotebookFolderInput,
) -> Result<NotebookFolder> {
    notebook_folders_table::create_notebook_folder(user_db.conn(), input).await
}

pub async fn rename_notebook_folder(user_db: &UserDb, id: &str, name: &str) -> Result<()> {
    notebook_folders_table::rename_notebook_folder(user_db.conn(), id, name).await
}

pub async fn move_notebook_node(user_db: &UserDb, input: MoveNotebookNodeInput) -> Result<()> {
    notebook_folders_table::move_notebook_node(user_db.conn(), input).await
}

/// Delete a notebook folder and its entire subtree (descendant folders + notes
/// + images + note-trade links via the cascading deletes in the table layer).
///
/// Cloudinary boundary: the `CloudinaryClient` lives in the route/resolver layer
/// (see `routes/notebook_images.rs::delete_notebook_image`), not in the
/// read-service, so this fn does NOT touch Cloudinary. It gathers the affected
/// image `cloudinary_public_id`s FIRST (aborting on error so nothing is deleted
/// if the gather fails), then removes the DB rows, and RETURNS the gathered
/// public_ids. The caller (GraphQL mutation, which holds the Cloudinary client)
/// is responsible for best-effort Cloudinary asset deletion of these ids.
pub async fn delete_notebook_folder(user_db: &UserDb, folder_id: &str) -> Result<Vec<String>> {
    // 1. Gather first — abort (propagate) before deleting anything if this fails.
    let public_ids =
        notebook_folders_table::gather_subtree_image_public_ids(user_db.conn(), folder_id).await?;

    // 2. Remove the DB rows for the whole subtree.
    notebook_folders_table::delete_notebook_folder_subtree(user_db.conn(), folder_id).await?;

    // 3. Hand the public_ids back to the caller for Cloudinary cleanup.
    Ok(public_ids)
}
