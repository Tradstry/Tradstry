use anyhow::{Context, Result};
use async_graphql::SimpleObject;
use libsql::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Helpers ─────────────────────────────────────────────────────────────────

fn opt_text(row: &libsql::Row, idx: i32) -> Option<String> {
    row.get::<libsql::Value>(idx).ok().and_then(|v| match v {
        libsql::Value::Text(s) if !s.is_empty() => Some(s),
        _ => None,
    })
}

fn opt_f64(row: &libsql::Row, idx: i32) -> Option<f64> {
    row.get::<libsql::Value>(idx).ok().and_then(|v| match v {
        libsql::Value::Real(f) => Some(f),
        libsql::Value::Integer(i) => Some(i as f64),
        _ => None,
    })
}

// ── BrokerageTransaction ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct BrokerageTransaction {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub snaptrade_id: String,
    pub symbol: Option<String>,
    pub symbol_description: Option<String>,
    pub raw_symbol: Option<String>,
    pub currency: String,
    pub transaction_type: String,
    pub option_type: Option<String>,
    pub price: f64,
    pub units: f64,
    pub amount: Option<f64>,
    pub fee: f64,
    pub fx_rate: Option<f64>,
    pub description: Option<String>,
    pub trade_date: Option<String>,
    pub settlement_date: String,
    pub institution: String,
    pub external_reference_id: Option<String>,
    pub raw_json: String,
    pub created_at: String,
    pub updated_at: String,
}

const TX_SELECT_COLS: &str = "id, user_id, account_id, snaptrade_id, symbol, symbol_description, \
    raw_symbol, currency, transaction_type, option_type, price, units, amount, fee, fx_rate, \
    description, trade_date, settlement_date, institution, external_reference_id, raw_json, \
    created_at, updated_at";

fn row_to_transaction(row: &libsql::Row) -> Result<BrokerageTransaction> {
    Ok(BrokerageTransaction {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        account_id: row.get::<String>(2)?,
        snaptrade_id: row.get::<String>(3)?,
        symbol: opt_text(row, 4),
        symbol_description: opt_text(row, 5),
        raw_symbol: opt_text(row, 6),
        currency: row.get::<String>(7)?,
        transaction_type: row.get::<String>(8)?,
        option_type: opt_text(row, 9),
        price: row.get::<f64>(10).unwrap_or(0.0),
        units: row.get::<f64>(11).unwrap_or(0.0),
        amount: opt_f64(row, 12),
        fee: row.get::<f64>(13).unwrap_or(0.0),
        fx_rate: opt_f64(row, 14),
        description: opt_text(row, 15),
        trade_date: opt_text(row, 16),
        settlement_date: row.get::<String>(17)?,
        institution: row.get::<String>(18)?,
        external_reference_id: opt_text(row, 19),
        raw_json: row.get::<String>(20)?,
        created_at: row.get::<String>(21)?,
        updated_at: row.get::<String>(22)?,
    })
}

pub struct TransactionFilters {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub transaction_type: Option<String>,
    pub sort_by: Option<String>,
    pub offset: i32,
    pub limit: i32,
}

impl Default for TransactionFilters {
    fn default() -> Self {
        Self {
            start_date: None,
            end_date: None,
            transaction_type: None,
            sort_by: None,
            offset: 0,
            limit: 1000,
        }
    }
}

pub struct TransactionPage {
    pub data: Vec<BrokerageTransaction>,
    pub total: i32,
    pub offset: i32,
    pub limit: i32,
}

pub async fn list_transactions(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
    filters: &TransactionFilters,
) -> Result<TransactionPage> {
    let mut where_clauses = vec!["user_id = ?1".to_string(), "account_id = ?2".to_string()];
    let mut params: Vec<libsql::Value> = vec![
        libsql::Value::Text(user_id.to_string()),
        libsql::Value::Text(account_id.to_string()),
    ];
    let mut idx = 3;

    if let Some(ref sd) = filters.start_date {
        where_clauses.push(format!("trade_date >= ?{idx}"));
        params.push(libsql::Value::Text(sd.clone()));
        idx += 1;
    }
    if let Some(ref ed) = filters.end_date {
        where_clauses.push(format!("trade_date <= ?{idx}"));
        params.push(libsql::Value::Text(ed.clone()));
        idx += 1;
    }
    if let Some(ref tt) = filters.transaction_type {
        where_clauses.push(format!("transaction_type = ?{idx}"));
        params.push(libsql::Value::Text(tt.clone()));
        idx += 1;
    }

    let where_sql = where_clauses.join(" AND ");

    // Count total
    let count_sql = format!("SELECT COUNT(*) FROM brokerage_transactions WHERE {where_sql}");
    let mut count_rows = conn
        .query(&count_sql, libsql::params_from_iter(params.clone()))
        .await
        .context("Failed to count transactions")?;
    let total = match count_rows.next().await? {
        Some(row) => row.get::<i32>(0).unwrap_or(0),
        None => 0,
    };

    // Fetch page
    params.push(libsql::Value::Integer(filters.limit as i64));
    params.push(libsql::Value::Integer(filters.offset as i64));
    let order_by = match filters.sort_by.as_deref() {
        Some("symbol") => "ORDER BY substr(trade_date, 1, 7) DESC, symbol ASC, trade_date DESC",
        _ => "ORDER BY trade_date DESC",
    };
    let data_sql = format!(
        "SELECT {TX_SELECT_COLS} FROM brokerage_transactions WHERE {where_sql} \
         {order_by} LIMIT ?{idx} OFFSET ?{}",
        idx + 1
    );

    let mut rows = conn
        .query(&data_sql, libsql::params_from_iter(params))
        .await
        .context("Failed to list transactions")?;

    let mut data = Vec::new();
    while let Some(row) = rows.next().await? {
        data.push(row_to_transaction(&row)?);
    }

    Ok(TransactionPage {
        data,
        total,
        offset: filters.offset,
        limit: filters.limit,
    })
}

/// Delete all stored transactions for a (user, account). Used during SnapTrade
/// re-registration recovery so the fresh sync — which produces new snaptrade_id
/// values for the same trades — doesn't duplicate rows alongside the historical
/// ones (the upsert is keyed on snaptrade_id).
pub async fn delete_transactions_for_account(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
) -> Result<u64> {
    let rows = conn
        .execute(
            "DELETE FROM brokerage_transactions WHERE user_id = ?1 AND account_id = ?2",
            libsql::params![user_id, account_id],
        )
        .await
        .context("Failed to delete brokerage transactions for account")?;
    Ok(rows)
}

/// Fetch all transactions for an account, ordered for lifecycle grouping
/// (symbol ASC, trade_date ASC, id ASC). Used by pending_trades — no
/// pagination because the algorithm needs the full per-symbol fill sequence.
pub async fn list_all_for_lifecycle(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<BrokerageTransaction>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {TX_SELECT_COLS} FROM brokerage_transactions \
                 WHERE user_id = ?1 AND account_id = ?2 \
                 ORDER BY symbol ASC, trade_date ASC, id ASC"
            ),
            libsql::params![user_id, account_id],
        )
        .await
        .context("Failed to list transactions for lifecycle")?;

    let mut data = Vec::new();
    while let Some(row) = rows.next().await? {
        data.push(row_to_transaction(&row)?);
    }
    Ok(data)
}

/// Fetch transactions by a list of IDs scoped to a user. Used by the
/// pending-trade prefill in the merge modal.
pub async fn get_transactions_by_ids(
    conn: &Connection,
    user_id: &str,
    ids: &[String],
) -> Result<Vec<BrokerageTransaction>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders: Vec<String> = (2..ids.len() + 2).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "SELECT {TX_SELECT_COLS} FROM brokerage_transactions \
         WHERE user_id = ?1 AND id IN ({}) \
         ORDER BY trade_date ASC",
        placeholders.join(", ")
    );

    let mut params: Vec<libsql::Value> = Vec::with_capacity(ids.len() + 1);
    params.push(libsql::Value::Text(user_id.to_string()));
    for id in ids {
        params.push(libsql::Value::Text(id.clone()));
    }

    let mut rows = conn
        .query(&sql, libsql::params_from_iter(params))
        .await
        .context("Failed to fetch transactions by ids")?;

    let mut data = Vec::new();
    while let Some(row) = rows.next().await? {
        data.push(row_to_transaction(&row)?);
    }
    Ok(data)
}

pub async fn get_transaction(
    conn: &Connection,
    id: &str,
    user_id: &str,
) -> Result<Option<BrokerageTransaction>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {TX_SELECT_COLS} FROM brokerage_transactions WHERE id = ?1 AND user_id = ?2"
            ),
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to get transaction")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_transaction(&row)?)),
        None => Ok(None),
    }
}

pub struct NewBrokerageTransaction {
    pub snaptrade_id: String,
    pub symbol: Option<String>,
    pub symbol_description: Option<String>,
    pub raw_symbol: Option<String>,
    pub currency: String,
    pub transaction_type: String,
    pub option_type: Option<String>,
    pub price: f64,
    pub units: f64,
    pub amount: Option<f64>,
    pub fee: f64,
    pub fx_rate: Option<f64>,
    pub description: Option<String>,
    pub trade_date: Option<String>,
    pub settlement_date: String,
    pub institution: String,
    pub external_reference_id: Option<String>,
    pub raw_json: String,
}

pub async fn upsert_transactions(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
    txs: &[NewBrokerageTransaction],
) -> Result<u64> {
    let mut count = 0u64;
    for tx in txs {
        let id = Uuid::new_v4().to_string();
        let rows = conn
            .execute(
                "INSERT INTO brokerage_transactions \
                 (id, user_id, account_id, snaptrade_id, symbol, symbol_description, raw_symbol, \
                  currency, transaction_type, option_type, price, units, amount, fee, fx_rate, \
                  description, trade_date, settlement_date, institution, external_reference_id, raw_json) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21) \
                 ON CONFLICT (user_id, account_id, snaptrade_id) DO UPDATE SET \
                  symbol=excluded.symbol, symbol_description=excluded.symbol_description, \
                  raw_symbol=excluded.raw_symbol, currency=excluded.currency, \
                  transaction_type=excluded.transaction_type, option_type=excluded.option_type, \
                  price=excluded.price, units=excluded.units, amount=excluded.amount, \
                  fee=excluded.fee, fx_rate=excluded.fx_rate, description=excluded.description, \
                  trade_date=excluded.trade_date, settlement_date=excluded.settlement_date, \
                  institution=excluded.institution, external_reference_id=excluded.external_reference_id, \
                  raw_json=excluded.raw_json",
                libsql::params![
                    id.as_str(),
                    user_id,
                    account_id,
                    tx.snaptrade_id.as_str(),
                    tx.symbol.as_deref().unwrap_or(""),
                    tx.symbol_description.as_deref().unwrap_or(""),
                    tx.raw_symbol.as_deref().unwrap_or(""),
                    tx.currency.as_str(),
                    tx.transaction_type.as_str(),
                    tx.option_type.as_deref().unwrap_or(""),
                    tx.price,
                    tx.units,
                    tx.amount.unwrap_or(0.0),
                    tx.fee,
                    tx.fx_rate.unwrap_or(0.0),
                    tx.description.as_deref().unwrap_or(""),
                    tx.trade_date.as_deref().unwrap_or(""),
                    tx.settlement_date.as_str(),
                    tx.institution.as_str(),
                    tx.external_reference_id.as_deref().unwrap_or(""),
                    tx.raw_json.as_str(),
                ],
            )
            .await
            .context("Failed to upsert transaction")?;
        count += rows;
    }
    Ok(count)
}

// ── BrokerageHolding ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct BrokerageHolding {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub snaptrade_symbol_id: Option<String>,
    pub symbol: String,
    pub symbol_description: Option<String>,
    pub raw_symbol: Option<String>,
    pub currency: String,
    pub units: f64,
    pub price: f64,
    pub market_value: Option<f64>,
    pub open_pnl: Option<f64>,
    pub average_purchase_price: Option<f64>,
    pub is_option: bool,
    pub option_type: Option<String>,
    pub strike_price: Option<f64>,
    pub expiration_date: Option<String>,
    pub raw_json: String,
    pub synced_at: String,
    pub created_at: String,
    pub updated_at: String,
}

const HOLDING_SELECT_COLS: &str = "id, user_id, account_id, snaptrade_symbol_id, symbol, \
    symbol_description, raw_symbol, currency, units, price, market_value, open_pnl, \
    average_purchase_price, is_option, option_type, strike_price, expiration_date, raw_json, \
    synced_at, created_at, updated_at";

fn row_to_holding(row: &libsql::Row) -> Result<BrokerageHolding> {
    Ok(BrokerageHolding {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        account_id: row.get::<String>(2)?,
        snaptrade_symbol_id: opt_text(row, 3),
        symbol: row.get::<String>(4)?,
        symbol_description: opt_text(row, 5),
        raw_symbol: opt_text(row, 6),
        currency: row.get::<String>(7)?,
        units: row.get::<f64>(8).unwrap_or(0.0),
        price: row.get::<f64>(9).unwrap_or(0.0),
        market_value: opt_f64(row, 10),
        open_pnl: opt_f64(row, 11),
        average_purchase_price: opt_f64(row, 12),
        is_option: row.get::<i32>(13).unwrap_or(0) != 0,
        option_type: opt_text(row, 14),
        strike_price: opt_f64(row, 15),
        expiration_date: opt_text(row, 16),
        raw_json: row.get::<String>(17)?,
        synced_at: row.get::<String>(18)?,
        created_at: row.get::<String>(19)?,
        updated_at: row.get::<String>(20)?,
    })
}

pub async fn list_holdings(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<BrokerageHolding>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {HOLDING_SELECT_COLS} FROM brokerage_holdings \
                 WHERE user_id = ?1 AND account_id = ?2 ORDER BY symbol"
            ),
            libsql::params![user_id, account_id],
        )
        .await
        .context("Failed to list holdings")?;

    let mut holdings = Vec::new();
    while let Some(row) = rows.next().await? {
        holdings.push(row_to_holding(&row)?);
    }
    Ok(holdings)
}

pub struct NewBrokerageHolding {
    pub snaptrade_symbol_id: Option<String>,
    pub symbol: String,
    pub symbol_description: Option<String>,
    pub raw_symbol: Option<String>,
    pub currency: String,
    pub units: f64,
    pub price: f64,
    pub market_value: Option<f64>,
    pub open_pnl: Option<f64>,
    pub average_purchase_price: Option<f64>,
    pub is_option: bool,
    pub option_type: Option<String>,
    pub strike_price: Option<f64>,
    pub expiration_date: Option<String>,
    pub raw_json: String,
}

pub async fn replace_holdings(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
    holdings: &[NewBrokerageHolding],
) -> Result<u64> {
    conn.execute(
        "DELETE FROM brokerage_holdings WHERE user_id = ?1 AND account_id = ?2",
        libsql::params![user_id, account_id],
    )
    .await
    .context("Failed to clear holdings")?;

    let mut count = 0u64;
    for h in holdings {
        let id = Uuid::new_v4().to_string();
        let rows = conn
            .execute(
                "INSERT INTO brokerage_holdings \
                 (id, user_id, account_id, snaptrade_symbol_id, symbol, symbol_description, \
                  raw_symbol, currency, units, price, market_value, open_pnl, \
                  average_purchase_price, is_option, option_type, strike_price, \
                  expiration_date, raw_json) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
                libsql::params![
                    id.as_str(),
                    user_id,
                    account_id,
                    h.snaptrade_symbol_id.as_deref().unwrap_or(""),
                    h.symbol.as_str(),
                    h.symbol_description.as_deref().unwrap_or(""),
                    h.raw_symbol.as_deref().unwrap_or(""),
                    h.currency.as_str(),
                    h.units,
                    h.price,
                    h.market_value.unwrap_or(0.0),
                    h.open_pnl.unwrap_or(0.0),
                    h.average_purchase_price.unwrap_or(0.0),
                    h.is_option as i32,
                    h.option_type.as_deref().unwrap_or(""),
                    h.strike_price.unwrap_or(0.0),
                    h.expiration_date.as_deref().unwrap_or(""),
                    h.raw_json.as_str(),
                ],
            )
            .await
            .context("Failed to insert holding")?;
        count += rows;
    }
    Ok(count)
}

// ── BrokerageBalance ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct BrokerageBalance {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub currency: String,
    pub cash: Option<f64>,
    pub buying_power: Option<f64>,
    pub synced_at: String,
    pub created_at: String,
    pub updated_at: String,
}

const BALANCE_SELECT_COLS: &str =
    "id, user_id, account_id, currency, cash, buying_power, synced_at, created_at, updated_at";

fn row_to_balance(row: &libsql::Row) -> Result<BrokerageBalance> {
    Ok(BrokerageBalance {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        account_id: row.get::<String>(2)?,
        currency: row.get::<String>(3)?,
        cash: opt_f64(row, 4),
        buying_power: opt_f64(row, 5),
        synced_at: row.get::<String>(6)?,
        created_at: row.get::<String>(7)?,
        updated_at: row.get::<String>(8)?,
    })
}

pub async fn list_balances(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<BrokerageBalance>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {BALANCE_SELECT_COLS} FROM brokerage_balances \
                 WHERE user_id = ?1 AND account_id = ?2 ORDER BY currency"
            ),
            libsql::params![user_id, account_id],
        )
        .await
        .context("Failed to list balances")?;

    let mut balances = Vec::new();
    while let Some(row) = rows.next().await? {
        balances.push(row_to_balance(&row)?);
    }
    Ok(balances)
}

pub struct NewBrokerageBalance {
    pub currency: String,
    pub cash: Option<f64>,
    pub buying_power: Option<f64>,
}

pub async fn replace_balances(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
    balances: &[NewBrokerageBalance],
) -> Result<u64> {
    conn.execute(
        "DELETE FROM brokerage_balances WHERE user_id = ?1 AND account_id = ?2",
        libsql::params![user_id, account_id],
    )
    .await
    .context("Failed to clear balances")?;

    let mut count = 0u64;
    for b in balances {
        let id = Uuid::new_v4().to_string();
        let rows = conn
            .execute(
                "INSERT INTO brokerage_balances (id, user_id, account_id, currency, cash, buying_power) \
                 VALUES (?1,?2,?3,?4,?5,?6)",
                libsql::params![
                    id.as_str(),
                    user_id,
                    account_id,
                    b.currency.as_str(),
                    b.cash.unwrap_or(0.0),
                    b.buying_power.unwrap_or(0.0),
                ],
            )
            .await
            .context("Failed to insert balance")?;
        count += rows;
    }
    Ok(count)
}
