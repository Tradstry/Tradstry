use anyhow::{Context, Result};
use libsql::Connection;

use super::client::{BrokerageClient, SnapTradeActivity, SnapTradeOptionPosition, SnapTradePosition};
use crate::service::turso::schema::tables::brokerage_table::{
    self, NewBrokerageBalance, NewBrokerageHolding, NewBrokerageTransaction,
};

/// Get the latest trade_date for a given user+account to use as start_date for incremental sync.
async fn latest_trade_date(conn: &Connection, user_id: &str, account_id: &str) -> Option<String> {
    let mut rows = conn
        .query(
            "SELECT MAX(trade_date) FROM brokerage_transactions WHERE user_id = ?1 AND account_id = ?2",
            libsql::params![user_id, account_id],
        )
        .await
        .ok()?;
    let row = rows.next().await.ok()??;
    let date: Option<String> = row.get(0).ok();
    // Return the date part only (YYYY-MM-DD) if present
    date.and_then(|d| {
        if d.is_empty() { None } else { Some(d.split('T').next().unwrap_or(&d).to_string()) }
    })
}

/// Syncs transactions from SnapTrade.
/// `snaptrade_account_id` is the SnapTrade-side account ID (for API calls).
/// `internal_account_id` is the Tradstry-side account ID (for DB storage).
pub async fn sync_transactions(
    client: &BrokerageClient,
    conn: &Connection,
    user_id: &str,
    user_secret: &str,
    snaptrade_account_id: &str,
    internal_account_id: &str,
) -> Result<u64> {
    let mut total_synced = 0u64;
    let mut offset = 0i32;
    let limit = 1000i32;

    // Incremental sync: only fetch transactions newer than the latest we have
    let start_date = latest_trade_date(conn, user_id, internal_account_id).await;
    if let Some(ref d) = start_date {
        log::info!("Incremental sync from start_date={} for account={}", d, internal_account_id);
    }

    loop {
        let response = client
            .fetch_transactions(
                user_id,
                user_secret,
                snaptrade_account_id,
                start_date.as_deref(),
                None,
                None,
                Some(offset),
                Some(limit),
            )
            .await
            .context("Failed to fetch transactions page")?;

        let activities = &response.data;
        if activities.is_empty() {
            break;
        }

        let new_txs: Vec<NewBrokerageTransaction> = activities
            .iter()
            .filter_map(|a| map_activity_to_transaction(a))
            .collect();

        let upserted = brokerage_table::upsert_transactions(conn, user_id, internal_account_id, &new_txs)
            .await
            .context("Failed to upsert transactions")?;
        total_synced += upserted;

        let page_total = response
            .pagination
            .as_ref()
            .and_then(|p| p.total)
            .unwrap_or(activities.len() as i32);

        offset += limit;
        if offset >= page_total {
            break;
        }
    }

    Ok(total_synced)
}

/// Syncs holdings from SnapTrade.
/// `snaptrade_account_id` is the SnapTrade-side account ID (for API calls).
/// `internal_account_id` is the Tradstry-side account ID (for DB storage).
pub async fn sync_holdings(
    client: &BrokerageClient,
    conn: &Connection,
    user_id: &str,
    user_secret: &str,
    snaptrade_account_id: &str,
    internal_account_id: &str,
) -> Result<(u64, u64)> {
    let response = client
        .fetch_holdings(user_id, user_secret, snaptrade_account_id)
        .await
        .context("Failed to fetch holdings")?;

    let mut holdings: Vec<NewBrokerageHolding> = Vec::new();

    if let Some(positions) = &response.positions {
        for p in positions {
            if let Some(h) = map_position_to_holding(p) {
                holdings.push(h);
            }
        }
    }

    if let Some(option_positions) = &response.option_positions {
        for op in option_positions {
            if let Some(h) = map_option_position_to_holding(op) {
                holdings.push(h);
            }
        }
    }

    let holdings_count =
        brokerage_table::replace_holdings(conn, user_id, internal_account_id, &holdings).await?;

    let balances: Vec<NewBrokerageBalance> = response
        .balances
        .as_ref()
        .map(|bals| {
            bals.iter()
                .map(|b| NewBrokerageBalance {
                    currency: b
                        .currency
                        .as_ref()
                        .and_then(|c| c.code.clone())
                        .unwrap_or_else(|| "USD".to_string()),
                    cash: b.cash,
                    buying_power: b.buying_power,
                })
                .collect()
        })
        .unwrap_or_default();

    let balances_count =
        brokerage_table::replace_balances(conn, user_id, internal_account_id, &balances).await?;

    Ok((holdings_count, balances_count))
}

fn map_activity_to_transaction(a: &SnapTradeActivity) -> Option<NewBrokerageTransaction> {
    let snaptrade_id = a.id.clone()?;
    let settlement_date = a.settlement_date.clone().unwrap_or_default();
    let institution = a.institution.clone().unwrap_or_default();
    let transaction_type = a.activity_type.clone().unwrap_or_default();
    let raw_json = serde_json::to_string(a).unwrap_or_default();

    Some(NewBrokerageTransaction {
        snaptrade_id,
        symbol: a.symbol.as_ref().and_then(|s| s.symbol.clone()),
        symbol_description: a.symbol.as_ref().and_then(|s| s.description.clone()),
        raw_symbol: a.symbol.as_ref().and_then(|s| s.raw_symbol.clone()),
        currency: a
            .currency
            .as_ref()
            .and_then(|c| c.code.clone())
            .unwrap_or_else(|| "USD".to_string()),
        transaction_type,
        option_type: a.option_type.clone(),
        price: a.price.unwrap_or(0.0),
        units: a.units.unwrap_or(0.0),
        amount: a.amount,
        fee: a.fee.unwrap_or(0.0),
        fx_rate: a.fx_rate,
        description: a.description.clone(),
        trade_date: a.trade_date.clone(),
        settlement_date,
        institution,
        external_reference_id: a.external_reference_id.clone(),
        raw_json,
    })
}

fn map_position_to_holding(p: &SnapTradePosition) -> Option<NewBrokerageHolding> {
    let symbol = p.symbol.as_ref().and_then(|s| s.symbol.clone())?;
    let raw_json = serde_json::to_string(p).unwrap_or_default();

    Some(NewBrokerageHolding {
        snaptrade_symbol_id: p.symbol.as_ref().and_then(|s| s.id.clone()),
        symbol,
        symbol_description: p.symbol.as_ref().and_then(|s| s.description.clone()),
        raw_symbol: p.symbol.as_ref().and_then(|s| s.raw_symbol.clone()),
        currency: p
            .currency
            .as_ref()
            .and_then(|c| c.code.clone())
            .unwrap_or_else(|| "USD".to_string()),
        units: p.units.unwrap_or(0.0),
        price: p.price.unwrap_or(0.0),
        market_value: None,
        open_pnl: p.open_pnl,
        average_purchase_price: p.average_purchase_price,
        is_option: false,
        option_type: None,
        strike_price: None,
        expiration_date: None,
        raw_json,
    })
}

fn map_option_position_to_holding(op: &SnapTradeOptionPosition) -> Option<NewBrokerageHolding> {
    let opt_sym = op.option_symbol.as_ref()?;
    let symbol = opt_sym.ticker.clone().unwrap_or_default();
    if symbol.is_empty() {
        return None;
    }
    let raw_json = serde_json::to_string(op).unwrap_or_default();

    Some(NewBrokerageHolding {
        snaptrade_symbol_id: opt_sym.id.clone(),
        symbol,
        symbol_description: opt_sym
            .underlying_symbol
            .as_ref()
            .and_then(|s| s.description.clone()),
        raw_symbol: opt_sym
            .underlying_symbol
            .as_ref()
            .and_then(|s| s.raw_symbol.clone()),
        currency: "USD".to_string(),
        units: op.units.unwrap_or(0.0),
        price: op.price.unwrap_or(0.0),
        market_value: None,
        open_pnl: None,
        average_purchase_price: op.average_purchase_price,
        is_option: true,
        option_type: opt_sym.option_type.clone(),
        strike_price: opt_sym.strike_price,
        expiration_date: opt_sym.expiration_date.clone(),
        raw_json,
    })
}
