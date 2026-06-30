use anyhow::{Context, Result};
use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::service::db::util::parse_flexible_datetime;

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

// raw_json (the full SnapTrade blob) is deliberately excluded from reads: it's
// ~10x the size of every other column combined and is never read back after
// being stored, so selecting it just bloated list payloads. The write/sync path
// still persists it.
//
// Timestamp columns are TIMESTAMPTZ; we surface them as full ISO-8601 UTC
// strings via to_char so the GraphQL String fields stay unchanged. Empty-string
// sentinels stored for nullable text columns are normalized back to NULL on read
// by NULLIF so they continue to surface as None.
const TX_SELECT_COLS: &str = "id, user_id, account_id, snaptrade_id, NULLIF(symbol, '') AS symbol, \
    NULLIF(symbol_description, '') AS symbol_description, NULLIF(raw_symbol, '') AS raw_symbol, \
    currency, transaction_type, NULLIF(option_type, '') AS option_type, price, units, amount, \
    fee, fx_rate, NULLIF(description, '') AS description, \
    to_char(trade_date AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS trade_date, \
    to_char(settlement_date AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS settlement_date, \
    institution, NULLIF(external_reference_id, '') AS external_reference_id, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

fn row_to_transaction(row: &sqlx::postgres::PgRow) -> Result<BrokerageTransaction> {
    Ok(BrokerageTransaction {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        account_id: row.try_get::<String, _>(2)?,
        snaptrade_id: row.try_get::<String, _>(3)?,
        symbol: row.try_get::<Option<String>, _>(4)?,
        symbol_description: row.try_get::<Option<String>, _>(5)?,
        raw_symbol: row.try_get::<Option<String>, _>(6)?,
        currency: row.try_get::<String, _>(7)?,
        transaction_type: row.try_get::<String, _>(8)?,
        option_type: row.try_get::<Option<String>, _>(9)?,
        price: row.try_get::<f64, _>(10).unwrap_or(0.0),
        units: row.try_get::<f64, _>(11).unwrap_or(0.0),
        amount: row.try_get::<Option<f64>, _>(12)?,
        fee: row.try_get::<f64, _>(13).unwrap_or(0.0),
        fx_rate: row.try_get::<Option<f64>, _>(14)?,
        description: row.try_get::<Option<String>, _>(15)?,
        trade_date: row.try_get::<Option<String>, _>(16)?,
        settlement_date: row.try_get::<String, _>(17)?,
        institution: row.try_get::<String, _>(18)?,
        external_reference_id: row.try_get::<Option<String>, _>(19)?,
        // Not selected (see TX_SELECT_COLS); reads return it empty.
        raw_json: String::new(),
        created_at: row.try_get::<String, _>(20)?,
        updated_at: row.try_get::<String, _>(21)?,
    })
}

pub struct TransactionFilters {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub transaction_type: Option<String>,
    pub symbol: Option<String>,
    pub sort_by: Option<String>,
    /// None = no filter; Some(true) = only journalled (linked) transactions;
    /// Some(false) = only not-yet-journalled (unlinked) transactions.
    pub is_journalled: Option<bool>,
    pub offset: i32,
    pub limit: i32,
}

impl Default for TransactionFilters {
    fn default() -> Self {
        Self {
            start_date: None,
            end_date: None,
            transaction_type: None,
            symbol: None,
            sort_by: None,
            is_journalled: None,
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

/// A bound parameter for the dynamically-built transaction queries. The columns
/// are heterogeneous (text, day-string date filters, and the int limit/offset),
/// so we carry an enum and bind by variant when assembling the query.
enum TxParam {
    Text(String),
    Int(i64),
}

pub async fn list_transactions(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    filters: &TransactionFilters,
) -> Result<TransactionPage> {
    let mut where_clauses = vec!["user_id = $1".to_string(), "account_id = $2".to_string()];
    let mut params: Vec<TxParam> = vec![
        TxParam::Text(user_id.to_string()),
        TxParam::Text(account_id.to_string()),
    ];
    let mut idx = 3;

    if let Some(ref sd) = filters.start_date {
        where_clauses.push(format!(
            "(trade_date AT TIME ZONE 'UTC')::date >= ${idx}::date"
        ));
        params.push(TxParam::Text(sd.clone()));
        idx += 1;
    }
    if let Some(ref ed) = filters.end_date {
        where_clauses.push(format!(
            "(trade_date AT TIME ZONE 'UTC')::date <= ${idx}::date"
        ));
        params.push(TxParam::Text(ed.clone()));
        idx += 1;
    }
    if let Some(ref tt) = filters.transaction_type {
        where_clauses.push(format!("transaction_type = ${idx}"));
        params.push(TxParam::Text(tt.clone()));
        idx += 1;
    }
    if let Some(ref sym) = filters.symbol {
        where_clauses.push(format!("symbol = ${idx}"));
        params.push(TxParam::Text(sym.clone()));
        idx += 1;
    }
    // Journalled filter: linked transactions live in journal_brokerage_links.
    // Reuses the already-bound user_id ($1), so it adds no new placeholder.
    if let Some(journalled) = filters.is_journalled {
        let op = if journalled { "IN" } else { "NOT IN" };
        where_clauses.push(format!(
            "id {op} (SELECT brokerage_transaction_id FROM journal_brokerage_links WHERE user_id = $1)"
        ));
    }

    let where_sql = where_clauses.join(" AND ");

    // Count total
    let count_sql = format!("SELECT COUNT(*) FROM brokerage_transactions WHERE {where_sql}");
    let mut count_query = sqlx::query(sqlx::AssertSqlSafe(count_sql));
    for p in &params {
        count_query = match p {
            TxParam::Text(s) => count_query.bind(s),
            TxParam::Int(i) => count_query.bind(*i),
        };
    }
    let count_row = count_query
        .fetch_one(pool)
        .await
        .context("Failed to count transactions")?;
    let total = count_row.try_get::<i64, _>(0).unwrap_or(0) as i32;

    // Fetch page
    let order_by = match filters.sort_by.as_deref() {
        Some("symbol") => {
            "ORDER BY to_char(trade_date AT TIME ZONE 'UTC', 'YYYY-MM') DESC, symbol ASC, trade_date DESC"
        }
        _ => "ORDER BY trade_date DESC",
    };
    let data_sql = format!(
        "SELECT {TX_SELECT_COLS} FROM brokerage_transactions WHERE {where_sql} \
         {order_by} LIMIT ${idx} OFFSET ${}",
        idx + 1
    );
    params.push(TxParam::Int(filters.limit as i64));
    params.push(TxParam::Int(filters.offset as i64));

    let mut data_query = sqlx::query(sqlx::AssertSqlSafe(data_sql));
    for p in &params {
        data_query = match p {
            TxParam::Text(s) => data_query.bind(s),
            TxParam::Int(i) => data_query.bind(*i),
        };
    }
    let rows = data_query
        .fetch_all(pool)
        .await
        .context("Failed to list transactions")?;

    let mut data = Vec::new();
    for row in &rows {
        data.push(row_to_transaction(row)?);
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
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
) -> Result<u64> {
    let rows =
        sqlx::query("DELETE FROM brokerage_transactions WHERE user_id = $1 AND account_id = $2")
            .bind(user_id)
            .bind(account_id)
            .execute(pool)
            .await
            .context("Failed to delete brokerage transactions for account")?
            .rows_affected();
    Ok(rows)
}

/// Fetch all transactions for an account, ordered for lifecycle grouping
/// (symbol ASC, trade_date ASC, id ASC). Used by pending_trades — no
/// pagination because the algorithm needs the full per-symbol fill sequence.
pub async fn list_all_for_lifecycle(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<BrokerageTransaction>> {
    let sql = format!(
        "SELECT {TX_SELECT_COLS} FROM brokerage_transactions \
         WHERE user_id = $1 AND account_id = $2 \
         ORDER BY symbol ASC, trade_date ASC, id ASC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .fetch_all(pool)
        .await
        .context("Failed to list transactions for lifecycle")?;

    let mut data = Vec::new();
    for row in &rows {
        data.push(row_to_transaction(row)?);
    }
    Ok(data)
}

/// Fetch transactions by a list of IDs scoped to a user. Used by the
/// pending-trade prefill in the merge modal.
pub async fn get_transactions_by_ids(
    pool: &PgPool,
    user_id: &str,
    ids: &[String],
) -> Result<Vec<BrokerageTransaction>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders: Vec<String> = (2..ids.len() + 2).map(|i| format!("${i}")).collect();
    let sql = format!(
        "SELECT {TX_SELECT_COLS} FROM brokerage_transactions \
         WHERE user_id = $1 AND id IN ({}) \
         ORDER BY trade_date ASC",
        placeholders.join(", ")
    );

    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(user_id);
    for id in ids {
        query = query.bind(id);
    }

    let rows = query
        .fetch_all(pool)
        .await
        .context("Failed to fetch transactions by ids")?;

    let mut data = Vec::new();
    for row in &rows {
        data.push(row_to_transaction(row)?);
    }
    Ok(data)
}

pub async fn get_transaction(
    pool: &PgPool,
    id: &str,
    user_id: &str,
) -> Result<Option<BrokerageTransaction>> {
    let sql = format!(
        "SELECT {TX_SELECT_COLS} FROM brokerage_transactions WHERE id = $1 AND user_id = $2"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("Failed to get transaction")?;

    match row {
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
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    txs: &[NewBrokerageTransaction],
) -> Result<u64> {
    let mut count = 0u64;
    for tx in txs {
        let id = Uuid::new_v4().to_string();
        // Temporal columns are TIMESTAMPTZ; parse the incoming SnapTrade strings.
        // trade_date is nullable, so an empty/absent string binds NULL.
        let trade_date = match tx.trade_date.as_deref() {
            Some(s) if !s.is_empty() => Some(parse_flexible_datetime(s)?),
            _ => None,
        };
        let settlement_date = parse_flexible_datetime(&tx.settlement_date)?;
        let rows = sqlx::query(
            "INSERT INTO brokerage_transactions \
                 (id, user_id, account_id, snaptrade_id, symbol, symbol_description, raw_symbol, \
                  currency, transaction_type, option_type, price, units, amount, fee, fx_rate, \
                  description, trade_date, settlement_date, institution, external_reference_id, raw_json) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21) \
                 ON CONFLICT (user_id, account_id, snaptrade_id) DO UPDATE SET \
                  symbol=EXCLUDED.symbol, symbol_description=EXCLUDED.symbol_description, \
                  raw_symbol=EXCLUDED.raw_symbol, currency=EXCLUDED.currency, \
                  transaction_type=EXCLUDED.transaction_type, option_type=EXCLUDED.option_type, \
                  price=EXCLUDED.price, units=EXCLUDED.units, amount=EXCLUDED.amount, \
                  fee=EXCLUDED.fee, fx_rate=EXCLUDED.fx_rate, description=EXCLUDED.description, \
                  trade_date=EXCLUDED.trade_date, settlement_date=EXCLUDED.settlement_date, \
                  institution=EXCLUDED.institution, external_reference_id=EXCLUDED.external_reference_id, \
                  raw_json=EXCLUDED.raw_json",
        )
        .bind(id.as_str())
        .bind(user_id)
        .bind(account_id)
        .bind(tx.snaptrade_id.as_str())
        .bind(tx.symbol.as_deref().unwrap_or(""))
        .bind(tx.symbol_description.as_deref().unwrap_or(""))
        .bind(tx.raw_symbol.as_deref().unwrap_or(""))
        .bind(tx.currency.as_str())
        .bind(tx.transaction_type.as_str())
        .bind(tx.option_type.as_deref().unwrap_or(""))
        .bind(tx.price)
        .bind(tx.units)
        .bind(tx.amount.unwrap_or(0.0))
        .bind(tx.fee)
        .bind(tx.fx_rate.unwrap_or(0.0))
        .bind(tx.description.as_deref().unwrap_or(""))
        .bind(trade_date)
        .bind(settlement_date)
        .bind(tx.institution.as_str())
        .bind(tx.external_reference_id.as_deref().unwrap_or(""))
        .bind(tx.raw_json.as_str())
        .execute(pool)
        .await
        .context("Failed to upsert transaction")?
        .rows_affected();
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

// raw_json excluded from reads for the same reason as transactions (see
// TX_SELECT_COLS): it's a large blob that's never read back. Timestamp columns
// (expiration_date, synced_at, created_at, updated_at) are TIMESTAMPTZ and
// surfaced as ISO-8601 UTC strings; empty-text sentinels normalized to NULL.
const HOLDING_SELECT_COLS: &str = "id, user_id, account_id, NULLIF(snaptrade_symbol_id, '') AS snaptrade_symbol_id, \
    symbol, NULLIF(symbol_description, '') AS symbol_description, NULLIF(raw_symbol, '') AS raw_symbol, \
    currency, units, price, market_value, open_pnl, average_purchase_price, is_option, \
    NULLIF(option_type, '') AS option_type, strike_price, \
    to_char(expiration_date AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS expiration_date, \
    to_char(synced_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS synced_at, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

fn row_to_holding(row: &sqlx::postgres::PgRow) -> Result<BrokerageHolding> {
    Ok(BrokerageHolding {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        account_id: row.try_get::<String, _>(2)?,
        snaptrade_symbol_id: row.try_get::<Option<String>, _>(3)?,
        symbol: row.try_get::<String, _>(4)?,
        symbol_description: row.try_get::<Option<String>, _>(5)?,
        raw_symbol: row.try_get::<Option<String>, _>(6)?,
        currency: row.try_get::<String, _>(7)?,
        units: row.try_get::<f64, _>(8).unwrap_or(0.0),
        price: row.try_get::<f64, _>(9).unwrap_or(0.0),
        market_value: row.try_get::<Option<f64>, _>(10)?,
        open_pnl: row.try_get::<Option<f64>, _>(11)?,
        average_purchase_price: row.try_get::<Option<f64>, _>(12)?,
        is_option: row.try_get::<bool, _>(13).unwrap_or(false),
        option_type: row.try_get::<Option<String>, _>(14)?,
        strike_price: row.try_get::<Option<f64>, _>(15)?,
        expiration_date: row.try_get::<Option<String>, _>(16)?,
        // Not selected (see HOLDING_SELECT_COLS); reads return it empty.
        raw_json: String::new(),
        synced_at: row.try_get::<String, _>(17)?,
        created_at: row.try_get::<String, _>(18)?,
        updated_at: row.try_get::<String, _>(19)?,
    })
}

pub async fn list_holdings(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<BrokerageHolding>> {
    let sql = format!(
        "SELECT {HOLDING_SELECT_COLS} FROM brokerage_holdings \
         WHERE user_id = $1 AND account_id = $2 ORDER BY symbol"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .fetch_all(pool)
        .await
        .context("Failed to list holdings")?;

    let mut holdings = Vec::new();
    for row in &rows {
        holdings.push(row_to_holding(row)?);
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
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    holdings: &[NewBrokerageHolding],
) -> Result<u64> {
    // Atomic swap: clear then re-insert in a single transaction so a reader never
    // sees the account with zero holdings mid-sync.
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM brokerage_holdings WHERE user_id = $1 AND account_id = $2")
        .bind(user_id)
        .bind(account_id)
        .execute(&mut *tx)
        .await
        .context("Failed to clear holdings")?;

    let mut count = 0u64;
    for h in holdings {
        let id = Uuid::new_v4().to_string();
        // expiration_date is TIMESTAMPTZ and nullable; parse when present.
        let expiration_date = match h.expiration_date.as_deref() {
            Some(s) if !s.is_empty() => Some(parse_flexible_datetime(s)?),
            _ => None,
        };
        let rows = sqlx::query(
            "INSERT INTO brokerage_holdings \
                 (id, user_id, account_id, snaptrade_symbol_id, symbol, symbol_description, \
                  raw_symbol, currency, units, price, market_value, open_pnl, \
                  average_purchase_price, is_option, option_type, strike_price, \
                  expiration_date, raw_json) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)",
        )
        .bind(id.as_str())
        .bind(user_id)
        .bind(account_id)
        .bind(h.snaptrade_symbol_id.as_deref().unwrap_or(""))
        .bind(h.symbol.as_str())
        .bind(h.symbol_description.as_deref().unwrap_or(""))
        .bind(h.raw_symbol.as_deref().unwrap_or(""))
        .bind(h.currency.as_str())
        .bind(h.units)
        .bind(h.price)
        .bind(h.market_value.unwrap_or(0.0))
        .bind(h.open_pnl.unwrap_or(0.0))
        .bind(h.average_purchase_price.unwrap_or(0.0))
        .bind(h.is_option)
        .bind(h.option_type.as_deref().unwrap_or(""))
        .bind(h.strike_price.unwrap_or(0.0))
        .bind(expiration_date)
        .bind(h.raw_json.as_str())
        .execute(&mut *tx)
        .await
        .context("Failed to insert holding")?
        .rows_affected();
        count += rows;
    }

    tx.commit().await?;
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

const BALANCE_SELECT_COLS: &str = "id, user_id, account_id, currency, cash, buying_power, \
    to_char(synced_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS synced_at, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

fn row_to_balance(row: &sqlx::postgres::PgRow) -> Result<BrokerageBalance> {
    Ok(BrokerageBalance {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        account_id: row.try_get::<String, _>(2)?,
        currency: row.try_get::<String, _>(3)?,
        cash: row.try_get::<Option<f64>, _>(4)?,
        buying_power: row.try_get::<Option<f64>, _>(5)?,
        synced_at: row.try_get::<String, _>(6)?,
        created_at: row.try_get::<String, _>(7)?,
        updated_at: row.try_get::<String, _>(8)?,
    })
}

pub async fn list_balances(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<BrokerageBalance>> {
    let sql = format!(
        "SELECT {BALANCE_SELECT_COLS} FROM brokerage_balances \
         WHERE user_id = $1 AND account_id = $2 ORDER BY currency"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .fetch_all(pool)
        .await
        .context("Failed to list balances")?;

    let mut balances = Vec::new();
    for row in &rows {
        balances.push(row_to_balance(row)?);
    }
    Ok(balances)
}

pub struct NewBrokerageBalance {
    pub currency: String,
    pub cash: Option<f64>,
    pub buying_power: Option<f64>,
}

pub async fn replace_balances(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    balances: &[NewBrokerageBalance],
) -> Result<u64> {
    // Atomic swap: clear then re-insert in a single transaction.
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM brokerage_balances WHERE user_id = $1 AND account_id = $2")
        .bind(user_id)
        .bind(account_id)
        .execute(&mut *tx)
        .await
        .context("Failed to clear balances")?;

    let mut count = 0u64;
    for b in balances {
        let id = Uuid::new_v4().to_string();
        let rows = sqlx::query(
            "INSERT INTO brokerage_balances (id, user_id, account_id, currency, cash, buying_power) \
                 VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(id.as_str())
        .bind(user_id)
        .bind(account_id)
        .bind(b.currency.as_str())
        .bind(b.cash.unwrap_or(0.0))
        .bind(b.buying_power.unwrap_or(0.0))
        .execute(&mut *tx)
        .await
        .context("Failed to insert balance")?
        .rows_affected();
        count += rows;
    }

    tx.commit().await?;
    Ok(count)
}
