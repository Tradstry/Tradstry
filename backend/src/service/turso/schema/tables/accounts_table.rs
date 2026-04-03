use anyhow::{Context, Result};
use async_graphql::{InputObject, SimpleObject};
use libsql::Connection;
use serde::{Deserialize, Serialize};
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
    pub snaptrade_user_secret_encrypted: Option<String>,
    pub snaptrade_connection_id: Option<String>,
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

fn opt_text(row: &libsql::Row, idx: i32) -> Option<String> {
    row.get::<libsql::Value>(idx).ok().and_then(|v| match v {
        libsql::Value::Text(s) if !s.is_empty() => Some(s),
        _ => None,
    })
}

fn row_to_account(row: &libsql::Row) -> Result<Account> {
    Ok(Account {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        name: row.get::<String>(2)?,
        icon: row.get::<String>(3)?,
        currency: row.get::<String>(4)?,
        broker: opt_text(row, 5),
        risk_profile: row.get::<String>(6)?,
        snaptrade_user_id: opt_text(row, 7),
        snaptrade_user_secret_encrypted: opt_text(row, 8),
        snaptrade_connection_id: opt_text(row, 9),
        created_at: row.get::<String>(10)?,
        updated_at: row.get::<String>(11)?,
    })
}

const SELECT_COLS: &str = "id, user_id, name, icon, currency, broker, risk_profile, \
    snaptrade_user_id, snaptrade_user_secret_encrypted, snaptrade_connection_id, \
    created_at, updated_at";

pub async fn list_accounts(conn: &Connection, user_id: &str) -> Result<Vec<Account>> {
    let mut rows = conn
        .query(
            &format!("SELECT {SELECT_COLS} FROM accounts WHERE user_id = ?1 ORDER BY created_at"),
            libsql::params![user_id],
        )
        .await
        .context("Failed to list accounts")?;

    let mut accounts = Vec::new();
    while let Some(row) = rows.next().await? {
        accounts.push(row_to_account(&row)?);
    }
    Ok(accounts)
}

pub async fn find_account(conn: &Connection, id: &str, user_id: &str) -> Result<Option<Account>> {
    let mut rows = conn
        .query(
            &format!("SELECT {SELECT_COLS} FROM accounts WHERE id = ?1 AND user_id = ?2"),
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to find account")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_account(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_account(
    conn: &Connection,
    user_id: &str,
    input: CreateAccountInput,
) -> Result<Account> {
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO accounts (id, user_id, name, icon, currency, broker, risk_profile) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        libsql::params![
            id.as_str(),
            user_id,
            input.name.as_str(),
            input.icon.as_str(),
            input.currency.as_str(),
            input.broker.as_deref().unwrap_or(""),
            input.risk_profile.as_str(),
        ],
    )
    .await
    .context("Failed to insert account")?;

    find_account(conn, &id, user_id)
        .await?
        .context("Account not found after insert")
}

pub async fn update_account(
    conn: &Connection,
    id: &str,
    user_id: &str,
    input: UpdateAccountInput,
) -> Result<Account> {
    let mut sets = Vec::new();
    let mut params: Vec<libsql::Value> = Vec::new();
    let mut idx = 1;

    if let Some(ref name) = input.name {
        sets.push(format!("name = ?{idx}"));
        params.push(libsql::Value::Text(name.clone()));
        idx += 1;
    }
    if let Some(ref icon) = input.icon {
        sets.push(format!("icon = ?{idx}"));
        params.push(libsql::Value::Text(icon.clone()));
        idx += 1;
    }
    if let Some(ref currency) = input.currency {
        sets.push(format!("currency = ?{idx}"));
        params.push(libsql::Value::Text(currency.clone()));
        idx += 1;
    }
    if let Some(ref broker) = input.broker {
        sets.push(format!("broker = ?{idx}"));
        params.push(libsql::Value::Text(broker.clone()));
        idx += 1;
    }
    if let Some(ref risk_profile) = input.risk_profile {
        sets.push(format!("risk_profile = ?{idx}"));
        params.push(libsql::Value::Text(risk_profile.clone()));
        idx += 1;
    }

    anyhow::ensure!(!sets.is_empty(), "No fields to update");

    let set_clause = sets.join(", ");
    params.push(libsql::Value::Text(id.to_string()));
    params.push(libsql::Value::Text(user_id.to_string()));

    let sql = format!(
        "UPDATE accounts SET {set_clause} WHERE id = ?{} AND user_id = ?{}",
        idx,
        idx + 1
    );

    conn.execute(&sql, libsql::params_from_iter(params))
        .await
        .context("Failed to update account")?;

    find_account(conn, id, user_id)
        .await?
        .context("Account not found after update")
}

pub async fn delete_account(conn: &Connection, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM accounts WHERE id = ?1 AND user_id = ?2",
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to delete account")?;

    Ok(rows_affected > 0)
}

pub async fn update_snaptrade_credentials(
    conn: &Connection,
    id: &str,
    user_id: &str,
    snaptrade_user_id: &str,
    snaptrade_user_secret_encrypted: &str,
    snaptrade_connection_id: Option<&str>,
) -> Result<Account> {
    conn.execute(
        "UPDATE accounts SET snaptrade_user_id = ?1, snaptrade_user_secret_encrypted = ?2, \
         snaptrade_connection_id = ?3 WHERE id = ?4 AND user_id = ?5",
        libsql::params![
            snaptrade_user_id,
            snaptrade_user_secret_encrypted,
            snaptrade_connection_id.unwrap_or(""),
            id,
            user_id,
        ],
    )
    .await
    .context("Failed to update snaptrade credentials")?;

    find_account(conn, id, user_id)
        .await?
        .context("Account not found after credential update")
}

pub async fn clear_snaptrade_credentials(
    conn: &Connection,
    id: &str,
    user_id: &str,
) -> Result<Account> {
    conn.execute(
        "UPDATE accounts SET snaptrade_user_id = NULL, snaptrade_user_secret_encrypted = NULL, \
         snaptrade_connection_id = NULL WHERE id = ?1 AND user_id = ?2",
        libsql::params![id, user_id],
    )
    .await
    .context("Failed to clear snaptrade credentials")?;

    find_account(conn, id, user_id)
        .await?
        .context("Account not found after clearing credentials")
}

pub async fn create_default_account(conn: &Connection, user_id: &str) -> Result<Account> {
    create_account(
        conn,
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
