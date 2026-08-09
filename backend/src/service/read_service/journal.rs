use anyhow::Result;

use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::journal_table::{
    self, CreateJournalEntryInput, JournalEntry, UpdateJournalEntryInput,
};

/// Trade outcome, matching the `status` column's CHECK constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeStatus {
    Profit,
    Loss,
}

impl TradeStatus {
    /// The stored column value (`'profit'` / `'loss'`).
    pub fn as_str(self) -> &'static str {
        match self {
            TradeStatus::Profit => "profit",
            TradeStatus::Loss => "loss",
        }
    }
}

/// Filters for [`list_journal_entries_filtered`], composed into a parameterized
/// SQL `WHERE` clause by the table layer. Every field is optional; `None` means
/// no constraint on that column.
#[derive(Debug, Default, Clone)]
pub struct JournalFilter {
    /// Restrict to a single trading account (exact match).
    pub workspace_id: Option<String>,
    /// Restrict to a ticker symbol (case-insensitive).
    pub symbol: Option<String>,
    /// Playbook filter, tri-state: `None` = don't filter, `Some(None)` = only
    /// trades with no playbook assigned, `Some(Some(id))` = a specific playbook.
    pub playbook_id: Option<Option<String>>,
    /// Restrict to winning (`Profit`) or losing (`Loss`) trades.
    pub status: Option<TradeStatus>,
    /// Inclusive lower bound on `total_pl` (percent P/L).
    pub min_pl_pct: Option<f64>,
    /// Inclusive upper bound on `total_pl` (percent P/L).
    pub max_pl_pct: Option<f64>,
    /// `Some(true)` = only trades with a stop set (`stop_loss <> 0`);
    /// `Some(false)` = only trades with no stop (`stop_loss = 0`).
    pub has_stop_loss: Option<bool>,
    /// Case-insensitive substring match against the `mistakes` field.
    pub mistake_contains: Option<String>,
    /// Inclusive lower bound on the UTC calendar date of `close_date` (YYYY-MM-DD).
    pub date_from: Option<String>,
    /// Inclusive upper bound on the UTC calendar date of `close_date` (YYYY-MM-DD).
    pub date_to: Option<String>,
    /// Keyset cursor `(created_at, id)`: return only rows strictly after it in
    /// `(created_at DESC, id DESC)` order. `created_at` is the microsecond RFC3339.
    pub after: Option<(String, String)>,
    /// Maximum rows to return. Defaults to 50, hard-capped at 500.
    pub limit: Option<u32>,
}

pub async fn list_journal_entries(user_db: &UserDb) -> Result<Vec<JournalEntry>> {
    journal_table::list_journal_entries(user_db.pool(), user_db.user_id()).await
}

pub async fn list_journal_entries_for_workspace(
    user_db: &UserDb,
    workspace_id: &str,
) -> Result<Vec<JournalEntry>> {
    journal_table::list_journal_entries_for_workspace(
        user_db.pool(),
        user_db.user_id(),
        workspace_id,
    )
    .await
}

/// List a user's journal entries with `filter` applied in SQL. See
/// [`journal_table::list_journal_entries_filtered`].
pub async fn list_journal_entries_filtered(
    user_db: &UserDb,
    filter: &JournalFilter,
) -> Result<Vec<JournalEntry>> {
    journal_table::list_journal_entries_filtered(user_db.pool(), user_db.user_id(), filter).await
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
