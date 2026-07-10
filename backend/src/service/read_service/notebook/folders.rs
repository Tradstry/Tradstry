use anyhow::Result;

use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::notebook::folders::{
    self, CreateNotebookFolderInput, MoveNotebookNodeInput, NotebookFolder,
};

pub async fn list_notebook_folders(
    user_db: &UserDb,
    account_id: &str,
) -> Result<Vec<NotebookFolder>> {
    folders::list_notebook_folders(user_db.pool(), account_id).await
}

pub async fn create_notebook_folder(
    user_db: &UserDb,
    input: CreateNotebookFolderInput,
) -> Result<NotebookFolder> {
    folders::create_notebook_folder(user_db.pool(), input).await
}

pub async fn rename_notebook_folder(user_db: &UserDb, id: &str, name: &str) -> Result<()> {
    folders::rename_notebook_folder(user_db.pool(), id, name).await
}

pub async fn move_notebook_node(user_db: &UserDb, input: MoveNotebookNodeInput) -> Result<()> {
    folders::move_notebook_node(user_db.pool(), input).await
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
    let public_ids = folders::gather_subtree_image_public_ids(user_db.pool(), folder_id).await?;

    // 2. Remove the DB rows for the whole subtree.
    folders::delete_notebook_folder_subtree(user_db.pool(), folder_id).await?;

    // 3. Hand the public_ids back to the caller for Cloudinary cleanup.
    Ok(public_ids)
}
