use anyhow::Result;

use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::notebook_images::{
    self, CreateNotebookImageInput, NotebookImage,
};

pub async fn create_notebook_image(
    user_db: &UserDb,
    input: CreateNotebookImageInput,
) -> Result<NotebookImage> {
    notebook_images::create_notebook_image(user_db.pool(), user_db.user_id(), input).await
}

pub async fn get_notebook_image(user_db: &UserDb, id: &str) -> Result<Option<NotebookImage>> {
    notebook_images::find_notebook_image(user_db.pool(), id, user_db.user_id()).await
}

pub async fn delete_notebook_image(user_db: &UserDb, id: &str) -> Result<()> {
    notebook_images::delete_notebook_image(user_db.pool(), id, user_db.user_id()).await
}
