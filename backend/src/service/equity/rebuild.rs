use std::collections::HashSet;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};

use crate::service::db::schema::tables::equity_table;
use crate::service::equity::prices;
use crate::service::equity::replay::{
    self, CashSignConvention, EquityPoint, PriceLookup, ReplayHealth, Txn, TxnKind,
};

#[derive(Debug, Clone)]
pub struct RebuildReport {
    pub points: usize,
    pub prices_fetched: usize,
    pub reconstructed_equity: Option<f64>,
    pub reported_equity: Option<f64>,
    pub health: ReplayHealth,
}

async fn load_txns(pool: &PgPool, user_id: &str, account_id: &str) -> Result<(Vec<Txn>, String)> {
    let currency: String =
        sqlx::query_scalar("SELECT currency FROM accounts WHERE id = $1 AND user_id = $2")
            .bind(account_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .context("Failed to read account currency")?
            .unwrap_or_else(|| "USD".to_string());

    let rows = sqlx::query(
        "SELECT symbol, transaction_type, option_type, units, amount, price, fee, currency, settlement_date \
         FROM brokerage_transactions \
         WHERE user_id = $1 AND account_id = $2 \
         ORDER BY settlement_date ASC",
    )
    .bind(user_id)
    .bind(account_id)
    .fetch_all(pool)
    .await
    .context("Failed to read brokerage transactions")?;

    let mut txns = Vec::with_capacity(rows.len());
    for row in &rows {
        let settlement: DateTime<Utc> = row.try_get("settlement_date")?;
        let amount: Option<f64> = row.try_get("amount")?;
        let units: f64 = row.try_get("units")?;
        let price: f64 = row.try_get("price")?;
        let fee: f64 = row.try_get("fee")?;
        let transaction_type: String = row.try_get("transaction_type")?;
        let option_type: Option<String> = row.try_get("option_type")?;
        let txn_currency: String = row.try_get("currency")?;

        // A missing `amount` is the one case where we must synthesize the cash impact.
        let amount = amount.unwrap_or_else(|| match replay::classify(&transaction_type) {
            TxnKind::Buy => -(units.abs() * price + fee),
            TxnKind::Sell => units.abs() * price - fee,
            _ => 0.0,
        });

        txns.push(Txn {
            settlement_date: settlement.date_naive(),
            transaction_type,
            symbol: row.try_get("symbol")?,
            units,
            amount,
            is_option: is_option_type(option_type.as_deref()),
            is_foreign_currency: txn_currency != currency,
        });
    }
    Ok((txns, currency))
}

/// SnapTrade sends `""` — not NULL — for equity trades, so a plain null check would
/// misclassify every stock trade as an option and zero out all share positions.
fn is_option_type(option_type: Option<&str>) -> bool {
    option_type.is_some_and(|s| !s.trim().is_empty())
}

pub async fn rebuild_account_equity(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
) -> Result<RebuildReport> {
    let (txns, _currency) = load_txns(pool, user_id, account_id).await?;
    if txns.is_empty() {
        equity_table::replace_equity_history(pool, user_id, account_id, &[]).await?;
        return Ok(RebuildReport {
            points: 0,
            prices_fetched: 0,
            reconstructed_equity: None,
            reported_equity: None,
            health: ReplayHealth::default(),
        });
    }

    let symbols: Vec<String> = txns
        .iter()
        .filter(|t| {
            !t.is_option
                && matches!(
                    replay::classify(&t.transaction_type),
                    TxnKind::Buy | TxnKind::Sell
                )
        })
        .filter_map(|t| t.symbol.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let from = txns
        .iter()
        .map(|t| t.settlement_date)
        .min()
        .unwrap_or_else(|| Utc::now().date_naive());
    let to = Utc::now().date_naive();

    let (closes, prices_fetched) = prices::load_closes(pool, &symbols, from, to).await?;
    let splits = prices::load_splits(&symbols).await?;

    let (points, health) = replay::replay(
        &txns,
        &splits,
        &PriceLookup(&closes),
        CashSignConvention::default(),
    );

    equity_table::replace_equity_history(pool, user_id, account_id, &points).await?;

    let reconstructed = points.last().map(|p: &EquityPoint| p.equity);
    let reported: Option<f64> =
        sqlx::query_scalar("SELECT total_value FROM accounts WHERE id = $1 AND user_id = $2")
            .bind(account_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .context("Failed to read reported account value")?
            .flatten();

    equity_table::save_rebuild_health(
        pool,
        user_id,
        account_id,
        reconstructed,
        reported,
        &health,
        crate::service::equity::REPLAY_VERSION,
    )
    .await?;

    Ok(RebuildReport {
        points: points.len(),
        prices_fetched,
        reconstructed_equity: reconstructed,
        reported_equity: reported,
        health,
    })
}

/// Rebuild only if the stored curve was produced by an older replay, or was never built.
/// This is what makes a math fix self-heal instead of waiting for a manual rebuild.
pub async fn rebuild_if_stale(pool: &PgPool, user_id: &str, account_id: &str) -> Result<()> {
    let stored = equity_table::rebuild_health(pool, user_id, account_id).await?;
    let fresh = stored
        .as_ref()
        .is_some_and(|h| h.replay_version == crate::service::equity::REPLAY_VERSION);
    if fresh {
        return Ok(());
    }
    rebuild_account_equity(pool, user_id, account_id).await?;
    Ok(())
}

/// Every account that has any broker transactions, for the scheduled sweep.
pub async fn rebuild_every_account(pool: &PgPool) -> Result<usize> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT DISTINCT user_id, account_id FROM brokerage_transactions")
            .fetch_all(pool)
            .await
            .context("Failed to list accounts for the equity sweep")?;

    let mut rebuilt = 0usize;
    for (user_id, account_id) in rows {
        match rebuild_account_equity(pool, &user_id, &account_id).await {
            Ok(_) => rebuilt += 1,
            Err(e) => log::warn!("equity: sweep failed for account {account_id}: {e}"),
        }
    }
    Ok(rebuilt)
}

pub async fn rebuild_all_accounts(pool: &PgPool, user_id: &str) -> Result<()> {
    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM accounts WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(pool)
        .await
        .context("Failed to list accounts for equity rebuild")?;

    for account_id in ids {
        if let Err(e) = rebuild_account_equity(pool, user_id, &account_id).await {
            log::warn!("equity: rebuild failed for account {account_id}: {e}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_option_type;

    #[test]
    fn empty_option_type_is_an_equity_trade_not_an_option() {
        assert!(!is_option_type(None));
        assert!(!is_option_type(Some("")));
        assert!(!is_option_type(Some("  ")));
        assert!(is_option_type(Some("CALL")));
        assert!(is_option_type(Some("PUT")));
    }
}
