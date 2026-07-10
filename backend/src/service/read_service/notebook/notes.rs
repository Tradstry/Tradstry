use anyhow::Result;

use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::notebook::notes::{
    self, CreateNotebookNoteInput, NotebookNote, UpdateNotebookNoteInput,
};

pub async fn list_notebook_notes(
    user_db: &UserDb,
    account_id: Option<&str>,
) -> Result<Vec<NotebookNote>> {
    notes::list_notebook_notes(user_db.pool(), user_db.user_id(), account_id).await
}

pub async fn get_notebook_note(user_db: &UserDb, id: &str) -> Result<Option<NotebookNote>> {
    notes::find_notebook_note(user_db.pool(), id, user_db.user_id()).await
}

pub async fn create_notebook_note(
    user_db: &UserDb,
    input: CreateNotebookNoteInput,
) -> Result<NotebookNote> {
    notes::create_notebook_note(user_db.pool(), user_db.user_id(), input).await
}

pub async fn update_notebook_note(
    user_db: &UserDb,
    id: &str,
    input: UpdateNotebookNoteInput,
) -> Result<NotebookNote> {
    notes::update_notebook_note(user_db.pool(), id, user_db.user_id(), input).await
}

pub async fn delete_notebook_note(user_db: &UserDb, id: &str) -> Result<bool> {
    notes::delete_notebook_note(user_db.pool(), id, user_db.user_id()).await
}
