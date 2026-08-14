use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransactionReconciliation {
    pub status: String,
    pub broker_count: i32,
    pub mapped_count: i32,
    pub imported_count: i32,
    pub duplicate_count: i32,
    pub skipped_count: i32,
    pub pending_count: i32,
    pub failed_count: i32,
    pub local_count: i32,
    pub missing_count: i32,
    pub extra_count: i32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortfolioReconciliation {
    pub status: String,
    pub broker_holding_count: i32,
    pub mapped_holding_count: i32,
    pub local_holding_count: i32,
    pub broker_balance_count: i32,
    pub local_balance_count: i32,
    pub balance_discrepancy_count: i32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerageReconciliationState {
    pub diagnostic_id: String,
    pub transaction_status: String,
    pub transaction_checked_at: Option<String>,
    pub broker_transaction_count: i32,
    pub mapped_transaction_count: i32,
    pub imported_transaction_count: i32,
    pub duplicate_transaction_count: i32,
    pub skipped_transaction_count: i32,
    pub pending_transaction_count: i32,
    pub failed_transaction_count: i32,
    pub local_transaction_count: i32,
    pub missing_transaction_count: i32,
    pub extra_transaction_count: i32,
    pub portfolio_status: String,
    pub portfolio_checked_at: Option<String>,
    pub broker_holding_count: i32,
    pub mapped_holding_count: i32,
    pub local_holding_count: i32,
    pub broker_balance_count: i32,
    pub local_balance_count: i32,
    pub balance_discrepancy_count: i32,
    pub transaction_error: Option<String>,
    pub portfolio_error: Option<String>,
}

pub async fn record_transaction_reconciliation(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    snaptrade_account_id: &str,
    diagnostic_id: &str,
    report: &TransactionReconciliation,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO brokerage_reconciliation_state (
             user_id, workspace_id, snaptrade_account_id, diagnostic_id,
             transaction_status, transaction_checked_at, broker_transaction_count,
             mapped_transaction_count, imported_transaction_count,
             duplicate_transaction_count, skipped_transaction_count,
             pending_transaction_count, failed_transaction_count,
             local_transaction_count, missing_transaction_count,
             extra_transaction_count, transaction_error
         ) VALUES ($1,$2,$3,$4,$5,now(),$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
         ON CONFLICT (user_id, workspace_id, snaptrade_account_id) DO UPDATE SET
             transaction_status=EXCLUDED.transaction_status,
             transaction_checked_at=EXCLUDED.transaction_checked_at,
             broker_transaction_count=EXCLUDED.broker_transaction_count,
             mapped_transaction_count=EXCLUDED.mapped_transaction_count,
             imported_transaction_count=EXCLUDED.imported_transaction_count,
             duplicate_transaction_count=EXCLUDED.duplicate_transaction_count,
             skipped_transaction_count=EXCLUDED.skipped_transaction_count,
             pending_transaction_count=EXCLUDED.pending_transaction_count,
             failed_transaction_count=EXCLUDED.failed_transaction_count,
             local_transaction_count=EXCLUDED.local_transaction_count,
             missing_transaction_count=EXCLUDED.missing_transaction_count,
             extra_transaction_count=EXCLUDED.extra_transaction_count,
             transaction_error=EXCLUDED.transaction_error",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(snaptrade_account_id)
    .bind(diagnostic_id)
    .bind(&report.status)
    .bind(report.broker_count)
    .bind(report.mapped_count)
    .bind(report.imported_count)
    .bind(report.duplicate_count)
    .bind(report.skipped_count)
    .bind(report.pending_count)
    .bind(report.failed_count)
    .bind(report.local_count)
    .bind(report.missing_count)
    .bind(report.extra_count)
    .bind(report.error.as_deref())
    .execute(pool)
    .await
    .context("Failed to record transaction reconciliation")?;
    Ok(())
}

pub async fn record_portfolio_reconciliation(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    snaptrade_account_id: &str,
    diagnostic_id: &str,
    report: &PortfolioReconciliation,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO brokerage_reconciliation_state (
             user_id, workspace_id, snaptrade_account_id, diagnostic_id,
             portfolio_status, portfolio_checked_at, broker_holding_count,
             mapped_holding_count, local_holding_count, broker_balance_count,
             local_balance_count, balance_discrepancy_count, portfolio_error
         ) VALUES ($1,$2,$3,$4,$5,now(),$6,$7,$8,$9,$10,$11,$12)
         ON CONFLICT (user_id, workspace_id, snaptrade_account_id) DO UPDATE SET
             portfolio_status=EXCLUDED.portfolio_status,
             portfolio_checked_at=EXCLUDED.portfolio_checked_at,
             broker_holding_count=EXCLUDED.broker_holding_count,
             mapped_holding_count=EXCLUDED.mapped_holding_count,
             local_holding_count=EXCLUDED.local_holding_count,
             broker_balance_count=EXCLUDED.broker_balance_count,
             local_balance_count=EXCLUDED.local_balance_count,
             balance_discrepancy_count=EXCLUDED.balance_discrepancy_count,
             portfolio_error=EXCLUDED.portfolio_error",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(snaptrade_account_id)
    .bind(diagnostic_id)
    .bind(&report.status)
    .bind(report.broker_holding_count)
    .bind(report.mapped_holding_count)
    .bind(report.local_holding_count)
    .bind(report.broker_balance_count)
    .bind(report.local_balance_count)
    .bind(report.balance_discrepancy_count)
    .bind(report.error.as_deref())
    .execute(pool)
    .await
    .context("Failed to record portfolio reconciliation")?;
    Ok(())
}

pub async fn get_for_workspace(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    snaptrade_account_id: &str,
) -> Result<Option<BrokerageReconciliationState>> {
    let row = sqlx::query(
        "SELECT diagnostic_id, transaction_status, transaction_checked_at,
                broker_transaction_count, mapped_transaction_count,
                imported_transaction_count, duplicate_transaction_count,
                skipped_transaction_count, pending_transaction_count,
                failed_transaction_count, local_transaction_count,
                missing_transaction_count, extra_transaction_count,
                portfolio_status, portfolio_checked_at, broker_holding_count,
                mapped_holding_count, local_holding_count, broker_balance_count,
                local_balance_count, balance_discrepancy_count, transaction_error,
                portfolio_error
         FROM brokerage_reconciliation_state
         WHERE user_id=$1 AND workspace_id=$2 AND snaptrade_account_id=$3",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(snaptrade_account_id)
    .fetch_optional(pool)
    .await
    .context("Failed to read brokerage reconciliation")?;

    row.map(|row| {
        let transaction_checked_at: Option<DateTime<Utc>> = row.try_get(2)?;
        let portfolio_checked_at: Option<DateTime<Utc>> = row.try_get(14)?;
        Ok(BrokerageReconciliationState {
            diagnostic_id: row.try_get(0)?,
            transaction_status: row.try_get(1)?,
            transaction_checked_at: transaction_checked_at.map(|value| value.to_rfc3339()),
            broker_transaction_count: row.try_get(3)?,
            mapped_transaction_count: row.try_get(4)?,
            imported_transaction_count: row.try_get(5)?,
            duplicate_transaction_count: row.try_get(6)?,
            skipped_transaction_count: row.try_get(7)?,
            pending_transaction_count: row.try_get(8)?,
            failed_transaction_count: row.try_get(9)?,
            local_transaction_count: row.try_get(10)?,
            missing_transaction_count: row.try_get(11)?,
            extra_transaction_count: row.try_get(12)?,
            portfolio_status: row.try_get(13)?,
            portfolio_checked_at: portfolio_checked_at.map(|value| value.to_rfc3339()),
            broker_holding_count: row.try_get(15)?,
            mapped_holding_count: row.try_get(16)?,
            local_holding_count: row.try_get(17)?,
            broker_balance_count: row.try_get(18)?,
            local_balance_count: row.try_get(19)?,
            balance_discrepancy_count: row.try_get(20)?,
            transaction_error: row.try_get(21)?,
            portfolio_error: row.try_get(22)?,
        })
    })
    .transpose()
}
