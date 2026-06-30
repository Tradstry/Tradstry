use anyhow::Result;

use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::accounts_table::{
    self, Account, CreateAccountInput, UpdateAccountInput,
};

pub async fn list_accounts(user_db: &UserDb) -> Result<Vec<Account>> {
    accounts_table::list_accounts(user_db.pool(), user_db.user_id()).await
}

pub async fn get_account(user_db: &UserDb, id: &str) -> Result<Option<Account>> {
    accounts_table::find_account(user_db.pool(), id, user_db.user_id()).await
}

pub async fn create_account(user_db: &UserDb, input: CreateAccountInput) -> Result<Account> {
    accounts_table::create_account(user_db.pool(), user_db.user_id(), input).await
}

pub async fn update_account(
    user_db: &UserDb,
    id: &str,
    input: UpdateAccountInput,
) -> Result<Account> {
    accounts_table::update_account(user_db.pool(), id, user_db.user_id(), input).await
}

pub async fn delete_account(user_db: &UserDb, id: &str) -> Result<bool> {
    accounts_table::delete_account(user_db.pool(), id, user_db.user_id()).await
}
