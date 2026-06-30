use anyhow::{Context, Result};
use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PositionCalculatorHistoryEntry {
    pub id: String,
    pub user_id: String,
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

const SELECT_COLS: &str = "id, user_id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, shares, position_value, account_pct, stop_loss_pct, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at";

fn row_to_entry(row: &sqlx::postgres::PgRow) -> Result<PositionCalculatorHistoryEntry> {
    Ok(PositionCalculatorHistoryEntry {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        symbol: row.try_get::<String, _>(2)?,
        position_type: row.try_get::<String, _>(3)?,
        entry_price: row.try_get::<f64, _>(4)?,
        stop_loss: row.try_get::<f64, _>(5)?,
        account_balance: row.try_get::<f64, _>(6)?,
        account_risk: row.try_get::<f64, _>(7)?,
        shares: row.try_get::<f64, _>(8)?,
        position_value: row.try_get::<f64, _>(9)?,
        account_pct: row.try_get::<f64, _>(10)?,
        stop_loss_pct: row.try_get::<f64, _>(11)?,
        created_at: row.try_get::<String, _>(12)?,
    })
}

pub async fn list_history(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<PositionCalculatorHistoryEntry>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM position_calculator_history WHERE user_id = $1 ORDER BY created_at DESC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
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
        "INSERT INTO position_calculator_history (id, user_id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, shares, position_value, account_pct, stop_loss_pct) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(id.as_str())
    .bind(user_id)
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
