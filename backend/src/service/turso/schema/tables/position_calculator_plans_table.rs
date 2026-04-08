use anyhow::{Context, Result};
use async_graphql::{InputObject, SimpleObject};
use libsql::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct Tranche {
    pub id: String,
    pub percent: f64,
    pub shares: f64,
    pub target_price: f64,
    pub status: String,
    pub filled_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PositionCalculatorPlan {
    pub id: String,
    pub user_id: String,
    pub symbol: String,
    pub position_type: String,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub account_balance: f64,
    pub account_risk: f64,
    pub total_shares: f64,
    pub position_value: f64,
    pub status: String,
    pub tranches: Vec<Tranche>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct CreateTrancheInput {
    pub percent: f64,
    pub shares: f64,
    pub target_price: f64,
}

#[derive(Debug, InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct CreatePositionCalculatorPlanInput {
    pub symbol: String,
    pub position_type: String,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub account_balance: f64,
    pub account_risk: f64,
    pub total_shares: f64,
    pub position_value: f64,
    pub tranches: Vec<CreateTrancheInput>,
    pub notes: Option<String>,
}

#[derive(Debug, InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct UpdateTrancheInput {
    pub id: String,
    pub percent: Option<f64>,
    pub shares: Option<f64>,
    pub target_price: Option<f64>,
    pub status: Option<String>,
}

#[derive(Debug, InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct UpdatePositionCalculatorPlanInput {
    pub status: Option<String>,
    pub tranches: Option<Vec<UpdateTrancheInput>>,
    pub notes: Option<String>,
    #[graphql(default)]
    pub clear_notes: bool,
}

const SELECT_COLS: &str = "id, user_id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, total_shares, position_value, status, tranches_json, notes, created_at, updated_at";

fn nullable_text(row: &libsql::Row, index: i32) -> Option<String> {
    row.get::<libsql::Value>(index)
        .ok()
        .and_then(|value| match value {
            libsql::Value::Text(text) if !text.is_empty() => Some(text),
            _ => None,
        })
}

fn row_to_plan(row: &libsql::Row) -> Result<PositionCalculatorPlan> {
    let tranches_json = row.get::<String>(11)?;
    let tranches: Vec<Tranche> =
        serde_json::from_str(&tranches_json).unwrap_or_default();

    Ok(PositionCalculatorPlan {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        symbol: row.get::<String>(2)?,
        position_type: row.get::<String>(3)?,
        entry_price: row.get::<f64>(4)?,
        stop_loss: row.get::<f64>(5)?,
        account_balance: row.get::<f64>(6)?,
        account_risk: row.get::<f64>(7)?,
        total_shares: row.get::<f64>(8)?,
        position_value: row.get::<f64>(9)?,
        status: row.get::<String>(10)?,
        tranches,
        notes: nullable_text(row, 12),
        created_at: row.get::<String>(13)?,
        updated_at: row.get::<String>(14)?,
    })
}

pub async fn list_plans(
    conn: &Connection,
    user_id: &str,
) -> Result<Vec<PositionCalculatorPlan>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {SELECT_COLS} FROM position_calculator_plans WHERE user_id = ?1 ORDER BY created_at DESC"
            ),
            libsql::params![user_id],
        )
        .await
        .context("Failed to list position calculator plans")?;

    let mut plans = Vec::new();
    while let Some(row) = rows.next().await? {
        plans.push(row_to_plan(&row)?);
    }

    Ok(plans)
}

pub async fn find_plan(
    conn: &Connection,
    id: &str,
    user_id: &str,
) -> Result<Option<PositionCalculatorPlan>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {SELECT_COLS} FROM position_calculator_plans WHERE id = ?1 AND user_id = ?2"
            ),
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to find position calculator plan")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_plan(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_plan(
    conn: &Connection,
    user_id: &str,
    input: CreatePositionCalculatorPlanInput,
) -> Result<PositionCalculatorPlan> {
    let id = Uuid::new_v4().to_string();

    let tranches: Vec<Tranche> = input
        .tranches
        .into_iter()
        .map(|t| Tranche {
            id: Uuid::new_v4().to_string(),
            percent: t.percent,
            shares: t.shares,
            target_price: t.target_price,
            status: "planned".to_string(),
            filled_at: None,
        })
        .collect();

    let tranches_json = serde_json::to_string(&tranches)?;

    conn.execute(
        "INSERT INTO position_calculator_plans (id, user_id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, total_shares, position_value, tranches_json, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        libsql::params![
            id.as_str(),
            user_id,
            input.symbol.trim(),
            input.position_type.as_str(),
            input.entry_price,
            input.stop_loss,
            input.account_balance,
            input.account_risk,
            input.total_shares,
            input.position_value,
            tranches_json.as_str(),
            input.notes.as_deref(),
        ],
    )
    .await
    .context("Failed to insert position calculator plan")?;

    find_plan(conn, &id, user_id)
        .await?
        .context("Plan not found after insert")
}

pub async fn update_plan(
    conn: &Connection,
    id: &str,
    user_id: &str,
    input: UpdatePositionCalculatorPlanInput,
) -> Result<PositionCalculatorPlan> {
    let current = find_plan(conn, id, user_id)
        .await?
        .context("Plan not found")?;

    let status = input.status.unwrap_or(current.status);

    let notes = if input.clear_notes {
        None
    } else {
        input.notes.or(current.notes)
    };

    let tranches = if let Some(updates) = input.tranches {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let mut updated = current.tranches;
        for update in updates {
            if let Some(tranche) = updated.iter_mut().find(|t| t.id == update.id) {
                if let Some(percent) = update.percent {
                    tranche.percent = percent;
                }
                if let Some(shares) = update.shares {
                    tranche.shares = shares;
                }
                if let Some(target_price) = update.target_price {
                    tranche.target_price = target_price;
                }
                if let Some(new_status) = update.status {
                    if new_status == "filled" && tranche.status != "filled" {
                        tranche.filled_at = Some(now.clone());
                    } else if new_status != "filled" {
                        tranche.filled_at = None;
                    }
                    tranche.status = new_status;
                }
            }
        }
        updated
    } else {
        current.tranches
    };

    let tranches_json = serde_json::to_string(&tranches)?;

    conn.execute(
        "UPDATE position_calculator_plans SET status = ?1, tranches_json = ?2, notes = ?3 WHERE id = ?4 AND user_id = ?5",
        libsql::params![
            status.as_str(),
            tranches_json.as_str(),
            notes.as_deref(),
            id,
            user_id,
        ],
    )
    .await
    .context("Failed to update position calculator plan")?;

    find_plan(conn, id, user_id)
        .await?
        .context("Plan not found after update")
}

pub async fn delete_plan(conn: &Connection, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM position_calculator_plans WHERE id = ?1 AND user_id = ?2",
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to delete position calculator plan")?;

    Ok(rows_affected > 0)
}
