use anyhow::Result;

use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::notebook::images::{self, NotebookImage};

/// Find a single notebook media item (image or video) by id, scoped to the user.
pub async fn find_notebook_image(
    user_db: &UserDb,
    media_id: &str,
) -> Result<Option<NotebookImage>> {
    images::find_notebook_image(user_db.pool(), media_id, user_db.user_id()).await
}
