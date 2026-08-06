use anyhow::{Context, Result};
use async_graphql::SimpleObject;
use chrono::{DateTime, Utc};
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
    pub workspace_id: String,
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
    pub contract_multiplier: f64,
    pub underlying_symbol: Option<String>,
    pub option_kind: Option<String>,
    pub strike_price: Option<f64>,
    pub option_expiration: Option<String>,
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
const TX_SELECT_COLS: &str = "id, user_id, workspace_id, snaptrade_id, NULLIF(symbol, '') AS symbol, \
    NULLIF(symbol_description, '') AS symbol_description, NULLIF(raw_symbol, '') AS raw_symbol, \
    currency, transaction_type, NULLIF(option_type, '') AS option_type, price, units, amount, \
    fee, fx_rate, NULLIF(description, '') AS description, \
    to_char(trade_date AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS trade_date, \
    to_char(settlement_date AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS settlement_date, \
    institution, NULLIF(external_reference_id, '') AS external_reference_id, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at, \
    contract_multiplier, NULLIF(underlying_symbol, '') AS underlying_symbol, \
    NULLIF(option_kind, '') AS option_kind, strike_price, \
    NULLIF(option_expiration, '') AS option_expiration";

fn row_to_transaction(row: &sqlx::postgres::PgRow) -> Result<BrokerageTransaction> {
    Ok(BrokerageTransaction {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        workspace_id: row.try_get::<String, _>(2)?,
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
        contract_multiplier: row.try_get::<f64, _>(22).unwrap_or(1.0),
        underlying_symbol: row.try_get::<Option<String>, _>(23)?,
        option_kind: row.try_get::<Option<String>, _>(24)?,
        strike_price: row.try_get::<Option<f64>, _>(25)?,
        option_expiration: row.try_get::<Option<String>, _>(26)?,
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
    workspace_id: &str,
    filters: &TransactionFilters,
) -> Result<TransactionPage> {
    let mut where_clauses = vec!["user_id = $1".to_string(), "workspace_id = $2".to_string()];
    let mut params: Vec<TxParam> = vec![
        TxParam::Text(user_id.to_string()),
        TxParam::Text(workspace_id.to_string()),
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
        // Substring, not exact: the search box is free text, and an option's
        // `symbol` is the full OCC contract (e.g. "IONQ  260508P00037000"), so an
        // exact `symbol = 'IONQ'` would hide every option under its underlying.
        // Match the underlying and the human description too.
        where_clauses.push(format!(
            "(symbol ILIKE ${idx} OR underlying_symbol ILIKE ${idx} \
             OR symbol_description ILIKE ${idx} OR raw_symbol ILIKE ${idx})"
        ));
        params.push(TxParam::Text(format!("%{}%", sym.trim())));
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
    workspace_id: &str,
) -> Result<u64> {
    let rows =
        sqlx::query("DELETE FROM brokerage_transactions WHERE user_id = $1 AND workspace_id = $2")
            .bind(user_id)
            .bind(workspace_id)
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
    workspace_id: &str,
) -> Result<Vec<BrokerageTransaction>> {
    let sql = format!(
        "SELECT {TX_SELECT_COLS} FROM brokerage_transactions \
         WHERE user_id = $1 AND workspace_id = $2 \
         ORDER BY symbol ASC, trade_date ASC, id ASC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(workspace_id)
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
    pub contract_multiplier: f64,
    pub underlying_symbol: Option<String>,
    pub option_kind: Option<String>,
    pub strike_price: Option<f64>,
    pub option_expiration: Option<String>,
}

/// Hash of the fill's attributes plus the brokerage's own reference for it.
/// Mirrors the expression in migration 0027 exactly, including the fixed
/// 8-decimal formatting, so both sides derive the same signature for the same
/// fill. `brokerage_dedup_pg` asserts that equivalence against this code path.
///
/// Excludes `snaptrade_id`, which SnapTrade regenerates per fetch and on
/// re-registration. Includes `external_reference_id`, which is what keeps the
/// key a function of the row rather than of its position in a fetched batch.
fn signature(tx: &NewBrokerageTransaction, trade_date: Option<&DateTime<Utc>>) -> String {
    let composite = format!(
        "{}|{}|{:.8}|{:.8}|{}|{}",
        tx.symbol.as_deref().unwrap_or("").to_lowercase(),
        trade_date.map_or(String::new(), |d| d.format("%Y-%m-%dT%H:%M:%S").to_string()),
        tx.units,
        tx.price,
        tx.transaction_type.to_lowercase(),
        tx.external_reference_id.as_deref().unwrap_or(""),
    );
    format!("{:x}", md5::compute(composite.as_bytes()))
}

/// Tracks how many times each signature has been seen so far in a sync run, so
/// fills that carry no reference id still get stable `:0`, `:1`, `:2` suffixes.
///
/// Lives across pagination: a sync fetches in pages of 1000, and restarting the
/// count per page would give two such fills the same key. Groups share a
/// `trade_date`, so an incremental window never splits one.
pub type SignatureCounts = std::collections::HashMap<String, u32>;

/// Rows per statement. Each row binds 27 parameters and Postgres caps a
/// statement at 65535, so 1000 leaves generous headroom.
const UPSERT_CHUNK: usize = 1000;

pub async fn upsert_transactions(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    txs: &[NewBrokerageTransaction],
    seen: &mut SignatureCounts,
) -> Result<u64> {
    // Ordinals are assigned in input order before chunking so a chunk boundary
    // can't restart the count for an order's identical partial fills.
    let mut prepared = Vec::with_capacity(txs.len());
    for tx in txs {
        // Temporal columns are TIMESTAMPTZ; parse the incoming SnapTrade strings.
        // trade_date is nullable, so an empty/absent string binds NULL.
        let trade_date = match tx.trade_date.as_deref() {
            Some(s) if !s.is_empty() => Some(parse_flexible_datetime(s)?),
            _ => None,
        };
        let settlement_date = parse_flexible_datetime(&tx.settlement_date)?;
        let sig = signature(tx, trade_date.as_ref());
        let ordinal = seen.entry(sig.clone()).or_insert(0);
        let dedup_key = format!("{sig}:{ordinal}");
        *ordinal += 1;
        prepared.push((tx, trade_date, settlement_date, dedup_key));
    }

    let mut count = 0u64;
    for chunk in prepared.chunks(UPSERT_CHUNK) {
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT INTO brokerage_transactions \
                 (id, user_id, workspace_id, snaptrade_id, symbol, symbol_description, raw_symbol, \
                  currency, transaction_type, option_type, price, units, amount, fee, fx_rate, \
                  description, trade_date, settlement_date, institution, external_reference_id, raw_json, \
                  contract_multiplier, underlying_symbol, option_kind, strike_price, option_expiration, \
                  dedup_key) ",
        );
        qb.push_values(
            chunk,
            |mut b, (tx, trade_date, settlement_date, dedup_key)| {
                b.push_bind(Uuid::new_v4().to_string())
                    .push_bind(user_id)
                    .push_bind(workspace_id)
                    .push_bind(tx.snaptrade_id.as_str())
                    .push_bind(tx.symbol.as_deref().unwrap_or(""))
                    .push_bind(tx.symbol_description.as_deref().unwrap_or(""))
                    .push_bind(tx.raw_symbol.as_deref().unwrap_or(""))
                    .push_bind(tx.currency.as_str())
                    .push_bind(tx.transaction_type.as_str())
                    .push_bind(tx.option_type.as_deref().unwrap_or(""))
                    .push_bind(tx.price)
                    .push_bind(tx.units)
                    .push_bind(tx.amount.unwrap_or(0.0))
                    .push_bind(tx.fee)
                    .push_bind(tx.fx_rate.unwrap_or(0.0))
                    .push_bind(tx.description.as_deref().unwrap_or(""))
                    .push_bind(*trade_date)
                    .push_bind(*settlement_date)
                    .push_bind(tx.institution.as_str())
                    .push_bind(tx.external_reference_id.as_deref().unwrap_or(""))
                    .push_bind(tx.raw_json.as_str())
                    .push_bind(tx.contract_multiplier)
                    .push_bind(tx.underlying_symbol.as_deref().unwrap_or(""))
                    .push_bind(tx.option_kind.as_deref().unwrap_or(""))
                    .push_bind(tx.strike_price)
                    .push_bind(tx.option_expiration.as_deref().unwrap_or(""))
                    .push_bind(dedup_key.as_str());
            },
        );
        qb.push(
            " ON CONFLICT (user_id, workspace_id, dedup_key) DO UPDATE SET \
                  snaptrade_id=EXCLUDED.snaptrade_id, \
                  symbol=EXCLUDED.symbol, symbol_description=EXCLUDED.symbol_description, \
                  raw_symbol=EXCLUDED.raw_symbol, currency=EXCLUDED.currency, \
                  transaction_type=EXCLUDED.transaction_type, option_type=EXCLUDED.option_type, \
                  price=EXCLUDED.price, units=EXCLUDED.units, amount=EXCLUDED.amount, \
                  fee=EXCLUDED.fee, fx_rate=EXCLUDED.fx_rate, description=EXCLUDED.description, \
                  trade_date=EXCLUDED.trade_date, settlement_date=EXCLUDED.settlement_date, \
                  institution=EXCLUDED.institution, external_reference_id=EXCLUDED.external_reference_id, \
                  raw_json=EXCLUDED.raw_json, contract_multiplier=EXCLUDED.contract_multiplier, \
                  underlying_symbol=EXCLUDED.underlying_symbol, option_kind=EXCLUDED.option_kind, \
                  strike_price=EXCLUDED.strike_price, option_expiration=EXCLUDED.option_expiration",
        );

        count += qb
            .build()
            .execute(pool)
            .await
            .context("Failed to upsert transactions")?
            .rows_affected();
    }
    Ok(count)
}

// ── BrokerageHolding ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct BrokerageHolding {
    pub id: String,
    pub user_id: String,
    pub workspace_id: String,
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
const HOLDING_SELECT_COLS: &str = "id, user_id, workspace_id, NULLIF(snaptrade_symbol_id, '') AS snaptrade_symbol_id, \
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
        workspace_id: row.try_get::<String, _>(2)?,
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
    workspace_id: &str,
) -> Result<Vec<BrokerageHolding>> {
    let sql = format!(
        "SELECT {HOLDING_SELECT_COLS} FROM brokerage_holdings \
         WHERE user_id = $1 AND workspace_id = $2 ORDER BY symbol"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(workspace_id)
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
    workspace_id: &str,
    holdings: &[NewBrokerageHolding],
) -> Result<u64> {
    // Atomic swap: clear then re-insert in a single transaction so a reader never
    // sees the account with zero holdings mid-sync.
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM brokerage_holdings WHERE user_id = $1 AND workspace_id = $2")
        .bind(user_id)
        .bind(workspace_id)
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
                 (id, user_id, workspace_id, snaptrade_symbol_id, symbol, symbol_description, \
                  raw_symbol, currency, units, price, market_value, open_pnl, \
                  average_purchase_price, is_option, option_type, strike_price, \
                  expiration_date, raw_json) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)",
        )
        .bind(id.as_str())
        .bind(user_id)
        .bind(workspace_id)
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
    pub workspace_id: String,
    pub currency: String,
    pub cash: Option<f64>,
    pub buying_power: Option<f64>,
    pub synced_at: String,
    pub created_at: String,
    pub updated_at: String,
}

const BALANCE_SELECT_COLS: &str = "id, user_id, workspace_id, currency, cash, buying_power, \
    to_char(synced_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS synced_at, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

fn row_to_balance(row: &sqlx::postgres::PgRow) -> Result<BrokerageBalance> {
    Ok(BrokerageBalance {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        workspace_id: row.try_get::<String, _>(2)?,
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
    workspace_id: &str,
) -> Result<Vec<BrokerageBalance>> {
    let sql = format!(
        "SELECT {BALANCE_SELECT_COLS} FROM brokerage_balances \
         WHERE user_id = $1 AND workspace_id = $2 ORDER BY currency"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(workspace_id)
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
    workspace_id: &str,
    balances: &[NewBrokerageBalance],
) -> Result<u64> {
    // Atomic swap: clear then re-insert in a single transaction.
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM brokerage_balances WHERE user_id = $1 AND workspace_id = $2")
        .bind(user_id)
        .bind(workspace_id)
        .execute(&mut *tx)
        .await
        .context("Failed to clear balances")?;

    let mut count = 0u64;
    for b in balances {
        let id = Uuid::new_v4().to_string();
        let rows = sqlx::query(
            "INSERT INTO brokerage_balances (id, user_id, workspace_id, currency, cash, buying_power) \
                 VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(id.as_str())
        .bind(user_id)
        .bind(workspace_id)
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

// ── Sync state ──────────────────────────────────────────────────────────────

pub async fn count_transactions(pool: &PgPool, user_id: &str, workspace_id: &str) -> Result<i64> {
    sqlx::query_scalar(
        "SELECT count(*) FROM brokerage_transactions WHERE user_id = $1 AND workspace_id = $2",
    )
    .bind(user_id)
    .bind(workspace_id)
    .fetch_one(pool)
    .await
    .context("Failed to count brokerage transactions")
}

/// How far SnapTrade had synced this brokerage account's transactions the last
/// time we fetched them. `None` means we have never synced it, so the caller
/// should treat it as stale and do a full fetch.
pub async fn transactions_synced_through(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    snaptrade_account_id: &str,
) -> Result<Option<String>> {
    let row = sqlx::query(
        "SELECT transactions_last_successful_sync FROM brokerage_sync_state \
         WHERE user_id = $1 AND workspace_id = $2 AND snaptrade_account_id = $3",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(snaptrade_account_id)
    .fetch_optional(pool)
    .await
    .context("Failed to read brokerage sync state")?;

    Ok(row.and_then(|r| r.try_get::<Option<String>, _>(0).ok().flatten()))
}

pub async fn record_transactions_synced_through(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    snaptrade_account_id: &str,
    synced_through: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO brokerage_sync_state \
             (user_id, workspace_id, snaptrade_account_id, transactions_last_successful_sync) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id, workspace_id, snaptrade_account_id) DO UPDATE SET \
             transactions_last_successful_sync = EXCLUDED.transactions_last_successful_sync, \
             updated_at = now()",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(snaptrade_account_id)
    .bind(synced_through)
    .execute(pool)
    .await
    .context("Failed to record brokerage sync state")?;
    Ok(())
}
