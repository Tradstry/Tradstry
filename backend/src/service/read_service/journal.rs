use anyhow::Result;

use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::journal_table::{
    self, CreateJournalEntryInput, JournalEntry, UpdateJournalEntryInput,
};

pub async fn list_journal_entries(user_db: &UserDb) -> Result<Vec<JournalEntry>> {
    journal_table::list_journal_entries(user_db.pool(), user_db.user_id()).await
}

pub async fn get_journal_entry(user_db: &UserDb, id: &str) -> Result<Option<JournalEntry>> {
    journal_table::find_journal_entry(user_db.pool(), id, user_db.user_id()).await
}

pub async fn create_journal_entry(
    user_db: &UserDb,
    input: CreateJournalEntryInput,
) -> Result<JournalEntry> {
    journal_table::create_journal_entry(user_db.pool(), user_db.user_id(), input).await
}

pub async fn update_journal_entry(
    user_db: &UserDb,
    id: &str,
    input: UpdateJournalEntryInput,
) -> Result<JournalEntry> {
    journal_table::update_journal_entry(user_db.pool(), id, user_db.user_id(), input).await
}

pub async fn delete_journal_entry(user_db: &UserDb, id: &str) -> Result<bool> {
    journal_table::delete_journal_entry(user_db.pool(), id, user_db.user_id()).await
}
