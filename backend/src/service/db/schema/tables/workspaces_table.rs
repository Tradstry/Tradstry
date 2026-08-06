use crate::service::db::util::parse_flexible_datetime;
use anyhow::{Context, Result, ensure};
use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub icon: String,
    pub currency: String,
    pub risk_profile: String,
    pub asset_class: String,
    pub broker: Option<String>,
    pub snaptrade_user_id: Option<String>,
    #[graphql(skip)]
    #[serde(default, skip_serializing)]
    pub snaptrade_user_secret_encrypted: Option<String>,
    pub snaptrade_connection_id: Option<String>,
    pub snaptrade_account_id: Option<String>,
    pub total_value: Option<f64>,
    pub total_value_currency: Option<String>,
    pub snaptrade_connection_disabled: bool,
    pub snaptrade_connection_disabled_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, InputObject)]
pub struct CreateWorkspaceInput {
    pub name: String,
    #[graphql(default_with = "\"chart-line-data-01\".to_string()")]
    pub icon: String,
    #[graphql(default_with = "\"USD\".to_string()")]
    pub currency: String,
    #[graphql(default_with = "\"mixed\".to_string()")]
    pub asset_class: String,
    pub broker: Option<String>,
    #[graphql(default_with = "\"moderate\".to_string()")]
    pub risk_profile: String,
}

#[derive(Debug, InputObject)]
pub struct UpdateWorkspaceInput {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub currency: Option<String>,
    pub asset_class: Option<String>,
    pub broker: Option<String>,
    pub risk_profile: Option<String>,
}

fn opt_text(row: &sqlx::postgres::PgRow, idx: usize) -> Option<String> {
    row.try_get::<Option<String>, _>(idx)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
}

fn row_to_workspace(row: &sqlx::postgres::PgRow) -> Result<Workspace> {
    Ok(Workspace {
        id: row.try_get(0)?,
        user_id: row.try_get(1)?,
        name: row.try_get(2)?,
        icon: row.try_get(3)?,
        currency: row.try_get(4)?,
        risk_profile: row.try_get(5)?,
        asset_class: row.try_get(6)?,
        broker: opt_text(row, 7),
        snaptrade_user_id: opt_text(row, 8),
        snaptrade_user_secret_encrypted: opt_text(row, 9),
        snaptrade_connection_id: opt_text(row, 10),
        snaptrade_account_id: opt_text(row, 11),
        total_value: row.try_get(12)?,
        total_value_currency: opt_text(row, 13),
        created_at: row.try_get(14)?,
        updated_at: row.try_get(15)?,
        snaptrade_connection_disabled: row.try_get::<Option<bool>, _>(16)?.unwrap_or(false),
        snaptrade_connection_disabled_at: row.try_get(17)?,
    })
}

const SELECT_COLS: &str = "w.id, w.user_id, w.name, w.icon, w.currency, w.risk_profile, w.asset_class, \
    bc.broker, bc.snaptrade_user_id, bc.snaptrade_user_secret_encrypted, \
    bc.snaptrade_connection_id, bc.snaptrade_account_id, bc.total_value, bc.total_value_currency, \
    to_char(w.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(w.updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at, \
    bc.connection_disabled, \
    to_char(bc.connection_disabled_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS connection_disabled_at";

const FROM_JOIN: &str =
    "FROM workspaces w LEFT JOIN brokerage_connections bc ON bc.workspace_id = w.id";

fn validate_asset_class(value: &str) -> Result<()> {
    ensure!(
        matches!(
            value,
            "futures" | "options" | "stocks" | "forex" | "crypto" | "mixed" | "other"
        ),
        "unsupported asset class"
    );
    Ok(())
}

pub async fn list_workspaces(pool: &PgPool, user_id: &str) -> Result<Vec<Workspace>> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {SELECT_COLS} {FROM_JOIN} WHERE w.user_id = $1 ORDER BY w.created_at, w.id"
    )))
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to list workspaces")?;
    rows.iter().map(row_to_workspace).collect()
}

pub async fn find_workspace<'e, E>(
    executor: E,
    id: &str,
    user_id: &str,
) -> Result<Option<Workspace>>
where
    E: sqlx::PgExecutor<'e>,
{
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {SELECT_COLS} {FROM_JOIN} WHERE w.id = $1 AND w.user_id = $2"
    )))
    .bind(id)
    .bind(user_id)
    .fetch_optional(executor)
    .await
    .context("Failed to find workspace")?;
    row.as_ref().map(row_to_workspace).transpose()
}

pub async fn find_by_snaptrade_account_id(
    pool: &PgPool,
    user_id: &str,
    snaptrade_account_id: &str,
) -> Result<Option<Workspace>> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {SELECT_COLS} {FROM_JOIN} WHERE w.user_id = $1 AND bc.snaptrade_account_id = $2"
    )))
    .bind(user_id)
    .bind(snaptrade_account_id)
    .fetch_optional(pool)
    .await
    .context("Failed to find workspace by SnapTrade account ID")?;
    row.as_ref().map(row_to_workspace).transpose()
}

pub async fn find_with_snaptrade_credentials(
    pool: &PgPool,
    user_id: &str,
) -> Result<Option<Workspace>> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {SELECT_COLS} {FROM_JOIN} WHERE w.user_id = $1 \
         AND bc.snaptrade_user_id IS NOT NULL \
         AND bc.snaptrade_user_secret_encrypted IS NOT NULL \
         ORDER BY w.created_at, w.id LIMIT 1"
    )))
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to find SnapTrade credentials")?;
    row.as_ref().map(row_to_workspace).transpose()
}

pub async fn create_workspace(
    pool: &PgPool,
    user_id: &str,
    input: CreateWorkspaceInput,
) -> Result<Workspace> {
    validate_asset_class(&input.asset_class)?;
    let id = Uuid::new_v4().to_string();
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO workspaces (id, user_id, name, icon, currency, risk_profile, asset_class) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(input.name.trim())
    .bind(&input.icon)
    .bind(&input.currency)
    .bind(&input.risk_profile)
    .bind(&input.asset_class)
    .execute(&mut *tx)
    .await
    .context("Failed to insert workspace")?;
    if let Some(broker) = input
        .broker
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        sqlx::query(
            "INSERT INTO brokerage_connections (workspace_id, user_id, broker) VALUES ($1, $2, $3)",
        )
        .bind(&id)
        .bind(user_id)
        .bind(broker.trim())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    crate::service::db::schema::tables::notebook::folders::ensure_system_folder(pool, user_id, &id)
        .await?;
    find_workspace(pool, &id, user_id)
        .await?
        .context("Workspace not found after insert")
}

pub async fn update_workspace(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    input: UpdateWorkspaceInput,
) -> Result<Workspace> {
    if let Some(asset_class) = input.asset_class.as_deref() {
        validate_asset_class(asset_class)?;
    }
    let current = find_workspace(pool, id, user_id)
        .await?
        .context("Workspace not found")?;
    let name = input.name.unwrap_or(current.name);
    let icon = input.icon.unwrap_or(current.icon);
    let currency = input.currency.unwrap_or(current.currency);
    let risk_profile = input.risk_profile.unwrap_or(current.risk_profile);
    let asset_class = input.asset_class.unwrap_or(current.asset_class);
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE workspaces SET name=$1, icon=$2, currency=$3, risk_profile=$4, asset_class=$5 \
         WHERE id=$6 AND user_id=$7",
    )
    .bind(name.trim())
    .bind(icon)
    .bind(currency)
    .bind(risk_profile)
    .bind(asset_class)
    .bind(id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Failed to update workspace")?;
    if let Some(broker) = input.broker {
        sqlx::query(
            "INSERT INTO brokerage_connections (workspace_id, user_id, broker) VALUES ($1,$2,$3) \
             ON CONFLICT (workspace_id) DO UPDATE SET broker=EXCLUDED.broker",
        )
        .bind(id)
        .bind(user_id)
        .bind(broker.trim())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    find_workspace(pool, id, user_id)
        .await?
        .context("Workspace not found after update")
}

pub async fn delete_workspace(pool: &PgPool, id: &str, user_id: &str) -> Result<bool> {
    let mut tx = pool.begin().await?;

    // Serialize workspace deletions for this user. Without this lock, two
    // concurrent requests could both observe two workspaces and delete one
    // each, bypassing the "keep at least one" rule.
    let locked_user: Option<String> =
        sqlx::query_scalar("SELECT id FROM users WHERE id=$1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    ensure!(locked_user.is_some(), "User not found");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces WHERE user_id=$1")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
    ensure!(count > 1, "You must keep at least one workspace");
    let result = sqlx::query("DELETE FROM workspaces WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_snaptrade_credentials(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    snaptrade_user_id: &str,
    encrypted_secret: &str,
    connection_id: Option<&str>,
) -> Result<Workspace> {
    ensure!(
        find_workspace(pool, id, user_id).await?.is_some(),
        "Workspace not found"
    );
    sqlx::query(
        "INSERT INTO brokerage_connections \
         (workspace_id,user_id,snaptrade_user_id,snaptrade_user_secret_encrypted,snaptrade_connection_id) \
         VALUES ($1,$2,$3,$4,$5) ON CONFLICT (workspace_id) DO UPDATE SET \
         snaptrade_user_id=EXCLUDED.snaptrade_user_id, \
         snaptrade_user_secret_encrypted=EXCLUDED.snaptrade_user_secret_encrypted, \
         snaptrade_connection_id=EXCLUDED.snaptrade_connection_id",
    )
    .bind(id)
    .bind(user_id)
    .bind(snaptrade_user_id)
    .bind(encrypted_secret)
    .bind(connection_id)
    .execute(pool)
    .await?;
    Ok(find_workspace(pool, id, user_id)
        .await?
        .expect("workspace exists"))
}

pub async fn set_snaptrade_account_id(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    snaptrade_account_id: &str,
) -> Result<Workspace> {
    let result = sqlx::query(
        "UPDATE brokerage_connections SET snaptrade_account_id=$1 WHERE workspace_id=$2 AND user_id=$3",
    )
    .bind(snaptrade_account_id)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    ensure!(
        result.rows_affected() == 1,
        "Workspace has no brokerage connection"
    );
    Ok(find_workspace(pool, id, user_id)
        .await?
        .expect("workspace exists"))
}

pub async fn clear_snaptrade_credentials(
    pool: &PgPool,
    id: &str,
    user_id: &str,
) -> Result<Workspace> {
    sqlx::query("DELETE FROM brokerage_connections WHERE workspace_id=$1 AND user_id=$2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    find_workspace(pool, id, user_id)
        .await?
        .context("Workspace not found")
}

pub async fn update_total_value(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    total_value: f64,
    currency: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE brokerage_connections SET total_value=$1, \
         total_value_currency=COALESCE($2,total_value_currency) WHERE workspace_id=$3 AND user_id=$4",
    )
    .bind(total_value)
    .bind(currency)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn create_default_workspace(pool: &PgPool, user_id: &str) -> Result<Workspace> {
    create_workspace(
        pool,
        user_id,
        CreateWorkspaceInput {
            name: "Main Workspace".into(),
            icon: "chart-line-data-01".into(),
            currency: "USD".into(),
            asset_class: "mixed".into(),
            broker: None,
            risk_profile: "moderate".into(),
        },
    )
    .await
}

pub async fn set_connection_disabled(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    disabled: bool,
    disabled_at: Option<&str>,
) -> Result<bool> {
    let disabled_at = disabled_at.map(parse_flexible_datetime).transpose()?;
    let result = sqlx::query(
        "UPDATE brokerage_connections SET connection_disabled=$1, connection_disabled_at=$2 \
         WHERE workspace_id=$3 AND user_id=$4 AND connection_disabled IS DISTINCT FROM $1",
    )
    .bind(disabled)
    .bind(disabled_at)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_secret_is_never_serialized() {
        let workspace = Workspace {
            id: "ws-1".into(),
            user_id: "user-1".into(),
            name: "Futures".into(),
            icon: "chart-line-data-01".into(),
            currency: "USD".into(),
            risk_profile: "moderate".into(),
            asset_class: "futures".into(),
            broker: Some("Tradovate".into()),
            snaptrade_user_id: Some("st-user".into()),
            snaptrade_user_secret_encrypted: Some("SECRET".into()),
            snaptrade_connection_id: Some("conn".into()),
            snaptrade_account_id: Some("st-account".into()),
            total_value: Some(1.0),
            total_value_currency: Some("USD".into()),
            snaptrade_connection_disabled: false,
            snaptrade_connection_disabled_at: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&workspace).unwrap();
        assert!(!json.contains("SECRET"));
    }
}
