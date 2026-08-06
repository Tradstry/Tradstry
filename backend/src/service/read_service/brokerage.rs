use anyhow::Result;

use crate::service::brokerage::pending_trades::{self, PendingTrade};
use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::brokerage_table::{
    self, BrokerageBalance, BrokerageHolding, BrokerageTransaction, TransactionFilters,
    TransactionPage,
};

pub async fn list_transactions(
    user_db: &UserDb,
    workspace_id: &str,
    filters: &TransactionFilters,
) -> Result<TransactionPage> {
    brokerage_table::list_transactions(user_db.pool(), user_db.user_id(), workspace_id, filters)
        .await
}

pub async fn get_transaction(user_db: &UserDb, id: &str) -> Result<Option<BrokerageTransaction>> {
    brokerage_table::get_transaction(user_db.pool(), id, user_db.user_id()).await
}

pub async fn get_transactions_by_ids(
    user_db: &UserDb,
    ids: &[String],
) -> Result<Vec<BrokerageTransaction>> {
    brokerage_table::get_transactions_by_ids(user_db.pool(), user_db.user_id(), ids).await
}

pub async fn list_pending_trades(
    user_db: &UserDb,
    workspace_id: &str,
) -> Result<Vec<PendingTrade>> {
    pending_trades::compute_pending_trades(user_db.pool(), user_db.user_id(), workspace_id).await
}

pub async fn list_holdings(user_db: &UserDb, workspace_id: &str) -> Result<Vec<BrokerageHolding>> {
    brokerage_table::list_holdings(user_db.pool(), user_db.user_id(), workspace_id).await
}

pub async fn list_balances(user_db: &UserDb, workspace_id: &str) -> Result<Vec<BrokerageBalance>> {
    brokerage_table::list_balances(user_db.pool(), user_db.user_id(), workspace_id).await
}
