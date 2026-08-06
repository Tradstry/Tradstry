use anyhow::{Context, Result};
use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PositionCalculatorHistoryEntry {
    pub id: String,
    pub user_id: String,
    pub workspace_id: String,
    pub symbol: String,
    pub position_type: String,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub account_balance: f64,
    pub account_risk: f64,
    pub shares: f64,
    pub position_value: f64,
    pub account_pct: f64,
    pub stop_loss_pct: f64,
    pub created_at: String,
}

#[derive(Debug, InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct CreatePositionCalculatorHistoryInput {
    pub workspace_id: String,
    pub symbol: String,
    pub position_type: String,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub account_balance: f64,
    pub account_risk: f64,
    pub shares: f64,
    pub position_value: f64,
    pub account_pct: f64,
    pub stop_loss_pct: f64,
}

const SELECT_COLS: &str = "id, user_id, workspace_id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, shares, position_value, account_pct, stop_loss_pct, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at";

fn row_to_entry(row: &sqlx::postgres::PgRow) -> Result<PositionCalculatorHistoryEntry> {
    Ok(PositionCalculatorHistoryEntry {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        workspace_id: row.try_get::<String, _>(2)?,
        symbol: row.try_get::<String, _>(3)?,
        position_type: row.try_get::<String, _>(4)?,
        entry_price: row.try_get::<f64, _>(5)?,
        stop_loss: row.try_get::<f64, _>(6)?,
        account_balance: row.try_get::<f64, _>(7)?,
        account_risk: row.try_get::<f64, _>(8)?,
        shares: row.try_get::<f64, _>(9)?,
        position_value: row.try_get::<f64, _>(10)?,
        account_pct: row.try_get::<f64, _>(11)?,
        stop_loss_pct: row.try_get::<f64, _>(12)?,
        created_at: row.try_get::<String, _>(13)?,
    })
}

pub async fn list_history(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
) -> Result<Vec<PositionCalculatorHistoryEntry>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM position_calculator_history WHERE user_id = $1 AND workspace_id = $2 ORDER BY created_at DESC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(workspace_id)
        .fetch_all(pool)
        .await
        .context("Failed to list position calculator history")?;

    let mut entries = Vec::new();
    for row in &rows {
        entries.push(row_to_entry(row)?);
    }

    Ok(entries)
}

pub async fn create_history_entry(
    pool: &PgPool,
    user_id: &str,
    input: CreatePositionCalculatorHistoryInput,
) -> Result<PositionCalculatorHistoryEntry> {
    let id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO position_calculator_history (id, user_id, workspace_id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, shares, position_value, account_pct, stop_loss_pct) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(&input.workspace_id)
    .bind(input.symbol.trim())
    .bind(input.position_type.as_str())
    .bind(input.entry_price)
    .bind(input.stop_loss)
    .bind(input.account_balance)
    .bind(input.account_risk)
    .bind(input.shares)
    .bind(input.position_value)
    .bind(input.account_pct)
    .bind(input.stop_loss_pct)
    .execute(pool)
    .await
    .context("Failed to insert position calculator history entry")?;

    let sql = format!(
        "SELECT {SELECT_COLS} FROM position_calculator_history WHERE id = $1 AND user_id = $2"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id.as_str())
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("Failed to fetch history entry after insert")?;

    match row {
        Some(row) => Ok(row_to_entry(&row)?),
        None => anyhow::bail!("History entry not found after insert"),
    }
}

pub async fn delete_history_entry(pool: &PgPool, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected =
        sqlx::query("DELETE FROM position_calculator_history WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await
            .context("Failed to delete position calculator history entry")?
            .rows_affected();

    Ok(rows_affected > 0)
}

// ---- Offline-first sync (append + soft-delete only, never updated) -------

/// The whole-row payload a `createPositionCalculatorHistory` mutation
/// carries. History is insert + soft-delete only — no update path exists.
pub struct HistoryWriteArgs {
    pub id: String,
    pub workspace_id: String,
    pub symbol: String,
    pub position_type: String,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub account_balance: f64,
    pub account_risk: f64,
    pub shares: f64,
    pub position_value: f64,
    pub account_pct: f64,
    pub stop_loss_pct: f64,
}

#[derive(Debug, Clone)]
pub struct HistoryDelta {
    pub id: String,
    pub workspace_id: String,
    pub symbol: String,
    pub position_type: String,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub account_balance: f64,
    pub account_risk: f64,
    pub shares: f64,
    pub position_value: f64,
    pub account_pct: f64,
    pub stop_loss_pct: f64,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

const DELTA_COLS: &str = "id, workspace_id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, \
    shares, position_value, account_pct, stop_loss_pct, hlc, \
    to_char(deleted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS deleted_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS updated_at";

pub async fn create_history_tx(
    conn: &mut PgConnection,
    user_id: &str,
    args: &HistoryWriteArgs,
    hlc: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO position_calculator_history \
         (id, user_id, workspace_id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, \
          shares, position_value, account_pct, stop_loss_pct, hlc) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(&args.id)
    .bind(user_id)
    .bind(&args.workspace_id)
    .bind(&args.symbol)
    .bind(&args.position_type)
    .bind(args.entry_price)
    .bind(args.stop_loss)
    .bind(args.account_balance)
    .bind(args.account_risk)
    .bind(args.shares)
    .bind(args.position_value)
    .bind(args.account_pct)
    .bind(args.stop_loss_pct)
    .bind(hlc)
    .execute(&mut *conn)
    .await
    .context("create_history_tx")?;
    Ok(())
}

pub async fn soft_delete_history_tx(
    conn: &mut PgConnection,
    user_id: &str,
    id: &str,
    hlc: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE position_calculator_history SET deleted_at = now(), hlc = $1 \
         WHERE id = $2 AND user_id = $3 AND deleted_at IS NULL",
    )
    .bind(hlc)
    .bind(id)
    .bind(user_id)
    .execute(&mut *conn)
    .await
    .context("soft_delete_history_tx")?;
    Ok(())
}

/// User-scoped pull deltas. Deliberately does NOT filter `deleted_at IS
/// NULL` — see `playbook_table::playbooks_since`.
pub async fn history_since(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    cookie: Option<&str>,
) -> Result<Vec<HistoryDelta>> {
    // A first pull that saw no rows returns `""` as the cursor, and
    // `''::timestamptz` throws. Treat an empty cookie as "no cursor".
    let cookie = cookie.filter(|c| !c.is_empty());
    let sql = format!(
        "SELECT {DELTA_COLS} FROM position_calculator_history \
         WHERE user_id = $1 AND workspace_id = $2 AND ($3::text IS NULL OR updated_at >= $3::timestamptz) \
         ORDER BY updated_at ASC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(workspace_id)
        .bind(cookie)
        .fetch_all(pool)
        .await
        .context("Failed to read position calculator history deltas")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(HistoryDelta {
            id: row.try_get("id")?,
            workspace_id: row.try_get("workspace_id")?,
            symbol: row.try_get("symbol")?,
            position_type: row.try_get("position_type")?,
            entry_price: row.try_get("entry_price")?,
            stop_loss: row.try_get("stop_loss")?,
            account_balance: row.try_get("account_balance")?,
            account_risk: row.try_get("account_risk")?,
            shares: row.try_get("shares")?,
            position_value: row.try_get("position_value")?,
            account_pct: row.try_get("account_pct")?,
            stop_loss_pct: row.try_get("stop_loss_pct")?,
            hlc: row.try_get("hlc")?,
            deleted_at: row.try_get("deleted_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(out)
}
