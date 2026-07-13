use anyhow::{Context, Result};
use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use super::accounts_table;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PositionCalculatorRule {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub account_balance: f64,
    pub account_risk: f64,
    pub max_stop_loss_pct: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct UpsertPositionCalculatorRuleInput {
    pub account_id: String,
    pub account_balance: f64,
    pub account_risk: f64,
    pub max_stop_loss_pct: f64,
}

const SELECT_COLS: &str = "id, user_id, account_id, account_balance, account_risk, max_stop_loss_pct, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

fn row_to_rule(row: &sqlx::postgres::PgRow) -> Result<PositionCalculatorRule> {
    Ok(PositionCalculatorRule {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        account_id: row.try_get::<String, _>(2)?,
        account_balance: row.try_get::<f64, _>(3)?,
        account_risk: row.try_get::<f64, _>(4)?,
        max_stop_loss_pct: row.try_get::<f64, _>(5)?,
        created_at: row.try_get::<String, _>(6)?,
        updated_at: row.try_get::<String, _>(7)?,
    })
}

pub async fn get_rule(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
) -> Result<Option<PositionCalculatorRule>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM position_calculator_rules \
         WHERE user_id = $1 AND account_id = $2"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .fetch_optional(pool)
        .await
        .context("Failed to get position calculator rule")?;

    match row {
        Some(row) => Ok(Some(row_to_rule(&row)?)),
        None => Ok(None),
    }
}

pub async fn upsert_rule(
    pool: &PgPool,
    user_id: &str,
    input: UpsertPositionCalculatorRuleInput,
) -> Result<PositionCalculatorRule> {
    // The `accounts(id)` foreign key proves the account exists, not that this
    // user owns it. Without this check any caller could write a rule against
    // any account id in the system.
    accounts_table::find_account(pool, &input.account_id, user_id)
        .await?
        .with_context(|| format!("account {} not found", input.account_id))?;

    let id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO position_calculator_rules \
         (id, user_id, account_id, account_balance, account_risk, max_stop_loss_pct) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (user_id, account_id) DO UPDATE SET \
            account_balance = EXCLUDED.account_balance, \
            account_risk = EXCLUDED.account_risk, \
            max_stop_loss_pct = EXCLUDED.max_stop_loss_pct, \
            updated_at = now()",
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(input.account_id.as_str())
    .bind(input.account_balance)
    .bind(input.account_risk)
    .bind(input.max_stop_loss_pct)
    .execute(pool)
    .await
    .context("Failed to upsert position calculator rule")?;

    get_rule(pool, user_id, &input.account_id)
        .await?
        .context("Rule not found after upsert")
}

// ---- Offline-first sync (whole-row LWW, keyed by (user_id, account_id)) --

/// The editable payload an `upsertPositionCalculatorRule` mutation carries.
/// The server is a dumb last-writer: it writes these fields verbatim + the
/// client's `hlc`; conflict resolution is client-side.
pub struct RuleWriteArgs {
    pub id: String,
    pub account_id: String,
    pub account_balance: f64,
    pub account_risk: f64,
    pub max_stop_loss_pct: f64,
}

#[derive(Debug, Clone)]
pub struct RuleDelta {
    pub id: String,
    pub account_id: String,
    pub account_balance: f64,
    pub account_risk: f64,
    pub max_stop_loss_pct: f64,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

const DELTA_COLS: &str = "id, account_id, account_balance, account_risk, max_stop_loss_pct, hlc, \
    to_char(deleted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS deleted_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS updated_at";

/// Upserts on `(user_id, account_id)`, the same key the whole-row upsert
/// resolver uses — a rule is one row per (user, account), regardless of
/// which device wrote it or what `id` that device minted.
pub async fn upsert_rule_tx(
    conn: &mut PgConnection,
    user_id: &str,
    args: &RuleWriteArgs,
    hlc: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO position_calculator_rules \
         (id, user_id, account_id, account_balance, account_risk, max_stop_loss_pct, hlc) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (user_id, account_id) DO UPDATE SET \
            account_balance = EXCLUDED.account_balance, \
            account_risk = EXCLUDED.account_risk, \
            max_stop_loss_pct = EXCLUDED.max_stop_loss_pct, \
            hlc = EXCLUDED.hlc, \
            updated_at = now()",
    )
    .bind(&args.id)
    .bind(user_id)
    .bind(&args.account_id)
    .bind(args.account_balance)
    .bind(args.account_risk)
    .bind(args.max_stop_loss_pct)
    .bind(hlc)
    .execute(&mut *conn)
    .await
    .context("upsert_rule_tx")?;
    Ok(())
}

/// User-scoped pull deltas (user-wide even though a rule is per-account: the
/// desktop pulls its whole rule set in one cycle). Deliberately does NOT
/// filter `deleted_at IS NULL` — see `playbook_table::playbooks_since`.
pub async fn rules_since(
    pool: &PgPool,
    user_id: &str,
    cookie: Option<&str>,
) -> Result<Vec<RuleDelta>> {
    // A first pull that saw no rows returns `""` as the cursor, and
    // `''::timestamptz` throws. Treat an empty cookie as "no cursor".
    let cookie = cookie.filter(|c| !c.is_empty());
    let sql = format!(
        "SELECT {DELTA_COLS} FROM position_calculator_rules \
         WHERE user_id = $1 AND ($2::text IS NULL OR updated_at >= $2::timestamptz) \
         ORDER BY updated_at ASC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(cookie)
        .fetch_all(pool)
        .await
        .context("Failed to read position calculator rule deltas")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(RuleDelta {
            id: row.try_get("id")?,
            account_id: row.try_get("account_id")?,
            account_balance: row.try_get("account_balance")?,
            account_risk: row.try_get("account_risk")?,
            max_stop_loss_pct: row.try_get("max_stop_loss_pct")?,
            hlc: row.try_get("hlc")?,
            deleted_at: row.try_get("deleted_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(out)
}
