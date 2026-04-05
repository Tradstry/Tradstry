use anyhow::Result;

use crate::service::turso::client::UserDb;
use crate::service::turso::schema::tables::brokerage_table::{
    self, BrokerageBalance, BrokerageHolding, BrokerageTransaction, TransactionFilters,
    TransactionPage,
};

pub async fn list_transactions(
    user_db: &UserDb,
    account_id: &str,
    filters: &TransactionFilters,
) -> Result<TransactionPage> {
    brokerage_table::list_transactions(user_db.conn(), user_db.user_id(), account_id, filters).await
}

pub async fn get_transaction(user_db: &UserDb, id: &str) -> Result<Option<BrokerageTransaction>> {
    brokerage_table::get_transaction(user_db.conn(), id, user_db.user_id()).await
}

pub async fn list_holdings(user_db: &UserDb, account_id: &str) -> Result<Vec<BrokerageHolding>> {
    brokerage_table::list_holdings(user_db.conn(), user_db.user_id(), account_id).await
}

pub async fn list_balances(user_db: &UserDb, account_id: &str) -> Result<Vec<BrokerageBalance>> {
    brokerage_table::list_balances(user_db.conn(), user_db.user_id(), account_id).await
}
