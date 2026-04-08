use anyhow::{Context, Result};
use async_graphql::{InputObject, SimpleObject};
use libsql::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PositionCalculatorRule {
    pub id: String,
    pub user_id: String,
    pub account_balance: f64,
    pub account_risk: f64,
    pub max_stop_loss_pct: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct UpsertPositionCalculatorRuleInput {
    pub account_balance: f64,
    pub account_risk: f64,
    pub max_stop_loss_pct: f64,
}

const SELECT_COLS: &str =
    "id, user_id, account_balance, account_risk, max_stop_loss_pct, created_at, updated_at";

fn row_to_rule(row: &libsql::Row) -> Result<PositionCalculatorRule> {
    Ok(PositionCalculatorRule {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        account_balance: row.get::<f64>(2)?,
        account_risk: row.get::<f64>(3)?,
        max_stop_loss_pct: row.get::<f64>(4)?,
        created_at: row.get::<String>(5)?,
        updated_at: row.get::<String>(6)?,
    })
}

pub async fn get_rule(
    conn: &Connection,
    user_id: &str,
) -> Result<Option<PositionCalculatorRule>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {SELECT_COLS} FROM position_calculator_rules WHERE user_id = ?1"
            ),
            libsql::params![user_id],
        )
        .await
        .context("Failed to get position calculator rule")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_rule(&row)?)),
        None => Ok(None),
    }
}

pub async fn upsert_rule(
    conn: &Connection,
    user_id: &str,
    input: UpsertPositionCalculatorRuleInput,
) -> Result<PositionCalculatorRule> {
    let existing = get_rule(conn, user_id).await?;

    match existing {
        Some(rule) => {
            conn.execute(
                "UPDATE position_calculator_rules SET account_balance = ?1, account_risk = ?2, max_stop_loss_pct = ?3 WHERE id = ?4 AND user_id = ?5",
                libsql::params![
                    input.account_balance,
                    input.account_risk,
                    input.max_stop_loss_pct,
                    rule.id.as_str(),
                    user_id,
                ],
            )
            .await
            .context("Failed to update position calculator rule")?;

            get_rule(conn, user_id)
                .await?
                .context("Rule not found after update")
        }
        None => {
            let id = Uuid::new_v4().to_string();

            conn.execute(
                "INSERT INTO position_calculator_rules (id, user_id, account_balance, account_risk, max_stop_loss_pct) VALUES (?1, ?2, ?3, ?4, ?5)",
                libsql::params![
                    id.as_str(),
                    user_id,
                    input.account_balance,
                    input.account_risk,
                    input.max_stop_loss_pct,
                ],
            )
            .await
            .context("Failed to insert position calculator rule")?;

            get_rule(conn, user_id)
                .await?
                .context("Rule not found after insert")
        }
    }
}
