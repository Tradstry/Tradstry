use anyhow::{Context, Result};
use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
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

const SELECT_COLS: &str = "id, user_id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, total_shares, position_value, status, tranches_json, notes, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

fn nullable_text(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

fn row_to_plan(row: &sqlx::postgres::PgRow) -> Result<PositionCalculatorPlan> {
    let tranches_json = row.try_get::<String, _>(11)?;
    let tranches: Vec<Tranche> = serde_json::from_str(&tranches_json).unwrap_or_default();

    Ok(PositionCalculatorPlan {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        symbol: row.try_get::<String, _>(2)?,
        position_type: row.try_get::<String, _>(3)?,
        entry_price: row.try_get::<f64, _>(4)?,
        stop_loss: row.try_get::<f64, _>(5)?,
        account_balance: row.try_get::<f64, _>(6)?,
        account_risk: row.try_get::<f64, _>(7)?,
        total_shares: row.try_get::<f64, _>(8)?,
        position_value: row.try_get::<f64, _>(9)?,
        status: row.try_get::<String, _>(10)?,
        tranches,
        notes: nullable_text(row.try_get::<Option<String>, _>(12)?),
        created_at: row.try_get::<String, _>(13)?,
        updated_at: row.try_get::<String, _>(14)?,
    })
}

pub async fn list_plans(pool: &PgPool, user_id: &str) -> Result<Vec<PositionCalculatorPlan>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM position_calculator_plans WHERE user_id = $1 ORDER BY created_at DESC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .fetch_all(pool)
        .await
        .context("Failed to list position calculator plans")?;

    let mut plans = Vec::new();
    for row in &rows {
        plans.push(row_to_plan(row)?);
    }

    Ok(plans)
}

pub async fn find_plan(
    pool: &PgPool,
    id: &str,
    user_id: &str,
) -> Result<Option<PositionCalculatorPlan>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM position_calculator_plans WHERE id = $1 AND user_id = $2"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("Failed to find position calculator plan")?;

    match row {
        Some(row) => Ok(Some(row_to_plan(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_plan(
    pool: &PgPool,
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

    sqlx::query(
        "INSERT INTO position_calculator_plans (id, user_id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk, total_shares, position_value, tranches_json, notes) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(input.symbol.trim())
    .bind(input.position_type.as_str())
    .bind(input.entry_price)
    .bind(input.stop_loss)
    .bind(input.account_balance)
    .bind(input.account_risk)
    .bind(input.total_shares)
    .bind(input.position_value)
    .bind(tranches_json.as_str())
    .bind(input.notes.as_deref())
    .execute(pool)
    .await
    .context("Failed to insert position calculator plan")?;

    find_plan(pool, &id, user_id)
        .await?
        .context("Plan not found after insert")
}

pub async fn update_plan(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    input: UpdatePositionCalculatorPlanInput,
) -> Result<PositionCalculatorPlan> {
    let current = find_plan(pool, id, user_id)
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

    sqlx::query(
        "UPDATE position_calculator_plans SET status = $1, tranches_json = $2, notes = $3 WHERE id = $4 AND user_id = $5",
    )
    .bind(status.as_str())
    .bind(tranches_json.as_str())
    .bind(notes.as_deref())
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Failed to update position calculator plan")?;

    find_plan(pool, id, user_id)
        .await?
        .context("Plan not found after update")
}

pub async fn delete_plan(pool: &PgPool, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected =
        sqlx::query("DELETE FROM position_calculator_plans WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await
            .context("Failed to delete position calculator plan")?
            .rows_affected();

    Ok(rows_affected > 0)
}
