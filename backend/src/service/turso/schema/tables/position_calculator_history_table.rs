use anyhow::{Context, Result};
use async_graphql::{InputObject, SimpleObject};
use libsql::Connection;
use serde::{Deserialize, Serialize};
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

const SELECT_COLS: &str = "id, user_id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, shares, position_value, account_pct, stop_loss_pct, created_at";

fn row_to_entry(row: &libsql::Row) -> Result<PositionCalculatorHistoryEntry> {
    Ok(PositionCalculatorHistoryEntry {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        symbol: row.get::<String>(2)?,
        position_type: row.get::<String>(3)?,
        entry_price: row.get::<f64>(4)?,
        stop_loss: row.get::<f64>(5)?,
        account_balance: row.get::<f64>(6)?,
        account_risk: row.get::<f64>(7)?,
        shares: row.get::<f64>(8)?,
        position_value: row.get::<f64>(9)?,
        account_pct: row.get::<f64>(10)?,
        stop_loss_pct: row.get::<f64>(11)?,
        created_at: row.get::<String>(12)?,
    })
}

pub async fn list_history(
    conn: &Connection,
    user_id: &str,
) -> Result<Vec<PositionCalculatorHistoryEntry>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {SELECT_COLS} FROM position_calculator_history WHERE user_id = ?1 ORDER BY created_at DESC"
            ),
            libsql::params![user_id],
        )
        .await
        .context("Failed to list position calculator history")?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await? {
        entries.push(row_to_entry(&row)?);
    }

    Ok(entries)
}

pub async fn create_history_entry(
    conn: &Connection,
    user_id: &str,
    input: CreatePositionCalculatorHistoryInput,
) -> Result<PositionCalculatorHistoryEntry> {
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO position_calculator_history (id, user_id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, shares, position_value, account_pct, stop_loss_pct) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        libsql::params![
            id.as_str(),
            user_id,
            input.symbol.trim(),
            input.position_type.as_str(),
            input.entry_price,
            input.stop_loss,
            input.account_balance,
            input.account_risk,
            input.shares,
            input.position_value,
            input.account_pct,
            input.stop_loss_pct,
        ],
    )
    .await
    .context("Failed to insert position calculator history entry")?;

    let mut rows = conn
        .query(
            &format!("SELECT {SELECT_COLS} FROM position_calculator_history WHERE id = ?1 AND user_id = ?2"),
            libsql::params![id.as_str(), user_id],
        )
        .await
        .context("Failed to fetch history entry after insert")?;

    match rows.next().await? {
        Some(row) => Ok(row_to_entry(&row)?),
        None => anyhow::bail!("History entry not found after insert"),
    }
}

pub async fn delete_history_entry(conn: &Connection, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM position_calculator_history WHERE id = ?1 AND user_id = ?2",
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to delete position calculator history entry")?;

    Ok(rows_affected > 0)
}
