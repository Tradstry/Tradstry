use crate::service::db::util::parse_flexible_datetime;
use anyhow::{Context, Result};
use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct Account {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub icon: String,
    pub currency: String,
    pub broker: Option<String>,
    pub risk_profile: String,
    pub snaptrade_user_id: Option<String>,
    #[graphql(skip)]
    #[serde(default, skip_serializing)]
    pub snaptrade_user_secret_encrypted: Option<String>,
    pub snaptrade_connection_id: Option<String>,
    /// SnapTrade's authoritative total market value (`account.balance.total`),
    /// persisted on each holdings sync. Null until first sync.
    pub total_value: Option<f64>,
    pub total_value_currency: Option<String>,
    /// True when SnapTrade has disabled the brokerage authorization (expired
    /// session / credential change). Reads still return the last snapshot, so
    /// this is what tells the UI the data is frozen.
    pub snaptrade_connection_disabled: bool,
    pub snaptrade_connection_disabled_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, InputObject)]
pub struct CreateAccountInput {
    pub name: String,
    #[graphql(default_with = "\"chart-line-data-01\".to_string()")]
    pub icon: String,
    #[graphql(default_with = "\"USD\".to_string()")]
    pub currency: String,
    pub broker: Option<String>,
    #[graphql(default_with = "\"moderate\".to_string()")]
    pub risk_profile: String,
}

#[derive(Debug, InputObject)]
pub struct UpdateAccountInput {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub currency: Option<String>,
    pub broker: Option<String>,
    pub risk_profile: Option<String>,
}

/// Normalize a nullable text column to `Option<String>`, treating empty strings
/// as absent. The credential setters persist `""` for absent values (rather than
/// NULL), so collapsing empty strings preserves the original semantics.
fn opt_text(row: &sqlx::postgres::PgRow, idx: usize) -> Option<String> {
    row.try_get::<Option<String>, _>(idx)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
}

fn row_to_account(row: &sqlx::postgres::PgRow) -> Result<Account> {
    Ok(Account {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        name: row.try_get::<String, _>(2)?,
        icon: row.try_get::<String, _>(3)?,
        currency: row.try_get::<String, _>(4)?,
        broker: opt_text(row, 5),
        risk_profile: row.try_get::<String, _>(6)?,
        snaptrade_user_id: opt_text(row, 7),
        snaptrade_user_secret_encrypted: opt_text(row, 8),
        snaptrade_connection_id: opt_text(row, 9),
        total_value: row.try_get::<Option<f64>, _>(10)?,
        total_value_currency: opt_text(row, 11),
        created_at: row.try_get::<String, _>(12)?,
        updated_at: row.try_get::<String, _>(13)?,
        snaptrade_connection_disabled: row.try_get::<bool, _>(14)?,
        snaptrade_connection_disabled_at: row.try_get::<Option<String>, _>(15)?,
    })
}

const SELECT_COLS: &str = "id, user_id, name, icon, currency, broker, risk_profile, \
    snaptrade_user_id, snaptrade_user_secret_encrypted, snaptrade_connection_id, \
    total_value, total_value_currency, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at, \
    snaptrade_connection_disabled, \
    to_char(snaptrade_connection_disabled_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS snaptrade_connection_disabled_at";

pub async fn list_accounts(pool: &PgPool, user_id: &str) -> Result<Vec<Account>> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {SELECT_COLS} FROM accounts WHERE user_id = $1 ORDER BY created_at"
    )))
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to list accounts")?;

    let mut accounts = Vec::new();
    for row in &rows {
        accounts.push(row_to_account(row)?);
    }
    Ok(accounts)
}

pub async fn find_account<'e, E>(executor: E, id: &str, user_id: &str) -> Result<Option<Account>>
where
    E: sqlx::PgExecutor<'e>,
{
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {SELECT_COLS} FROM accounts WHERE id = $1 AND user_id = $2"
    )))
    .bind(id)
    .bind(user_id)
    .fetch_optional(executor)
    .await
    .context("Failed to find account")?;

    match row {
        Some(row) => Ok(Some(row_to_account(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_account(
    pool: &PgPool,
    user_id: &str,
    input: CreateAccountInput,
) -> Result<Account> {
    let id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO accounts (id, user_id, name, icon, currency, broker, risk_profile) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(input.name.as_str())
    .bind(input.icon.as_str())
    .bind(input.currency.as_str())
    .bind(input.broker.as_deref().unwrap_or(""))
    .bind(input.risk_profile.as_str())
    .execute(pool)
    .await
    .context("Failed to insert account")?;

    find_account(pool, &id, user_id)
        .await?
        .context("Account not found after insert")
}

pub async fn update_account(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    input: UpdateAccountInput,
) -> Result<Account> {
    let mut sets = Vec::new();
    let mut params: Vec<String> = Vec::new();
    let mut idx = 1;

    if let Some(ref name) = input.name {
        sets.push(format!("name = ${idx}"));
        params.push(name.clone());
        idx += 1;
    }
    if let Some(ref icon) = input.icon {
        sets.push(format!("icon = ${idx}"));
        params.push(icon.clone());
        idx += 1;
    }
    if let Some(ref currency) = input.currency {
        sets.push(format!("currency = ${idx}"));
        params.push(currency.clone());
        idx += 1;
    }
    if let Some(ref broker) = input.broker {
        sets.push(format!("broker = ${idx}"));
        params.push(broker.clone());
        idx += 1;
    }
    if let Some(ref risk_profile) = input.risk_profile {
        sets.push(format!("risk_profile = ${idx}"));
        params.push(risk_profile.clone());
        idx += 1;
    }

    anyhow::ensure!(!sets.is_empty(), "No fields to update");

    let set_clause = sets.join(", ");
    let sql = format!(
        "UPDATE accounts SET {set_clause} WHERE id = ${} AND user_id = ${}",
        idx,
        idx + 1
    );

    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for param in &params {
        query = query.bind(param);
    }
    query = query.bind(id).bind(user_id);

    query
        .execute(pool)
        .await
        .context("Failed to update account")?;

    find_account(pool, id, user_id)
        .await?
        .context("Account not found after update")
}

pub async fn delete_account(pool: &PgPool, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = sqlx::query("DELETE FROM accounts WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to delete account")?
        .rows_affected();

    Ok(rows_affected > 0)
}

pub async fn update_snaptrade_credentials(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    snaptrade_user_id: &str,
    snaptrade_user_secret_encrypted: &str,
    snaptrade_connection_id: Option<&str>,
) -> Result<Account> {
    sqlx::query(
        "UPDATE accounts SET snaptrade_user_id = $1, snaptrade_user_secret_encrypted = $2, \
         snaptrade_connection_id = $3 WHERE id = $4 AND user_id = $5",
    )
    .bind(snaptrade_user_id)
    .bind(snaptrade_user_secret_encrypted)
    .bind(snaptrade_connection_id.unwrap_or(""))
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Failed to update snaptrade credentials")?;

    find_account(pool, id, user_id)
        .await?
        .context("Account not found after credential update")
}

pub async fn clear_snaptrade_credentials(
    pool: &PgPool,
    id: &str,
    user_id: &str,
) -> Result<Account> {
    sqlx::query(
        "UPDATE accounts SET snaptrade_user_id = NULL, snaptrade_user_secret_encrypted = NULL, \
         snaptrade_connection_id = NULL, snaptrade_connection_disabled = $1, \
         snaptrade_connection_disabled_at = NULL WHERE id = $2 AND user_id = $3",
    )
    .bind(false)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Failed to clear snaptrade credentials")?;

    find_account(pool, id, user_id)
        .await?
        .context("Account not found after clearing credentials")
}

/// Persist SnapTrade's authoritative total market value on the account row.
/// `currency` is optional; when absent the existing currency column is left
/// untouched. Only call this when an amount is actually present — None should
/// leave the column null rather than overwriting it with 0.
pub async fn update_total_value(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    total_value: f64,
    currency: Option<&str>,
) -> Result<()> {
    match currency {
        Some(c) => {
            sqlx::query(
                "UPDATE accounts SET total_value = $1, total_value_currency = $2 \
                 WHERE id = $3 AND user_id = $4",
            )
            .bind(total_value)
            .bind(c)
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await
            .context("Failed to update account total value")?;
        }
        None => {
            sqlx::query("UPDATE accounts SET total_value = $1 WHERE id = $2 AND user_id = $3")
                .bind(total_value)
                .bind(id)
                .bind(user_id)
                .execute(pool)
                .await
                .context("Failed to update account total value")?;
        }
    }
    Ok(())
}

pub async fn create_default_account(pool: &PgPool, user_id: &str) -> Result<Account> {
    create_account(
        pool,
        user_id,
        CreateAccountInput {
            name: "Main Portfolio".to_string(),
            icon: "chart-line-data-01".to_string(),
            currency: "USD".to_string(),
            broker: None,
            risk_profile: "moderate".to_string(),
        },
    )
    .await
}

/// Persist the disabled state of the account's brokerage connection. `disabled_at`
/// is SnapTrade's `disabled_date`; pass `None` (with `disabled = false`) to clear.
pub async fn set_connection_disabled(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    disabled: bool,
    disabled_at: Option<&str>,
) -> Result<()> {
    let disabled_at_ts = match disabled_at {
        Some(s) => Some(parse_flexible_datetime(s).context("Invalid connection disabled_at")?),
        None => None,
    };

    sqlx::query(
        "UPDATE accounts SET snaptrade_connection_disabled = $1, \
         snaptrade_connection_disabled_at = $2 WHERE id = $3 AND user_id = $4",
    )
    .bind(disabled)
    .bind(disabled_at_ts)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Failed to update connection disabled flag")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The encrypted SnapTrade user secret must never appear in serialized
    /// `Account` output — the MCP `list_accounts` tool serializes `Account`
    /// with `serde_json` and ships the result to the LLM.
    #[test]
    fn encrypted_secret_is_never_serialized() {
        let account = Account {
            id: "acct-1".to_string(),
            user_id: "user-1".to_string(),
            name: "Main Portfolio".to_string(),
            icon: "chart-line-data-01".to_string(),
            currency: "USD".to_string(),
            broker: Some("snaptrade".to_string()),
            risk_profile: "moderate".to_string(),
            snaptrade_user_id: Some("st-user-1".to_string()),
            snaptrade_user_secret_encrypted: Some("SUPER_SECRET_VALUE".into()),
            snaptrade_connection_id: Some("conn-1".to_string()),
            total_value: Some(1234.56),
            total_value_currency: Some("USD".to_string()),
            snaptrade_connection_disabled: false,
            snaptrade_connection_disabled_at: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-02T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&account).expect("Account should serialize");

        assert!(
            !json.contains("snaptrade_user_secret_encrypted"),
            "serialized Account must not expose the secret field name: {json}"
        );
        assert!(
            !json.contains("SUPER_SECRET_VALUE"),
            "serialized Account must not expose the secret value: {json}"
        );
    }
}
