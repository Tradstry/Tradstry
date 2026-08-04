use anyhow::{Context, Result};
use sqlx::PgPool;

use super::client::SnapTradeAccount;
use crate::service::db::schema::tables::accounts_table::{self, Account, CreateAccountInput};

pub async fn materialize_connection_accounts(
    pool: &PgPool,
    user_id: &str,
    primary_account_id: &str,
    snaptrade_accounts: &[SnapTradeAccount],
) -> Result<Vec<Account>> {
    let mut primary = accounts_table::find_account(pool, primary_account_id, user_id)
        .await?
        .expect("primary account was checked before materializing SnapTrade accounts");
    let snaptrade_user_id = primary
        .snaptrade_user_id
        .clone()
        .context("primary account has no SnapTrade user ID")?;
    let encrypted_secret = primary
        .snaptrade_user_secret_encrypted
        .clone()
        .context("primary account has no encrypted SnapTrade secret")?;
    let connection_id = primary
        .snaptrade_connection_id
        .clone()
        .filter(|connection_id| !connection_id.is_empty());
    let mut accounts = Vec::new();

    for snaptrade_account in snaptrade_accounts {
        // The accounts endpoint includes every brokerage connection for this
        // SnapTrade user. Only materialize accounts from the connection that
        // was just completed (or the legacy, connection-less case).
        if let Some(connection_id) = connection_id.as_deref()
            && snaptrade_account.brokerage_authorization.as_deref() != Some(connection_id)
        {
            continue;
        }

        let Some(snaptrade_account_id) = snaptrade_account.id.as_deref() else {
            continue;
        };

        if let Some(existing) =
            accounts_table::find_by_snaptrade_account_id(pool, user_id, snaptrade_account_id)
                .await?
        {
            accounts.push(
                accounts_table::update_snaptrade_credentials(
                    pool,
                    &existing.id,
                    user_id,
                    &snaptrade_user_id,
                    &encrypted_secret,
                    primary.snaptrade_connection_id.as_deref(),
                )
                .await?,
            );
            continue;
        }

        if primary.snaptrade_account_id.is_none() {
            primary = accounts_table::set_snaptrade_account_id(
                pool,
                &primary.id,
                user_id,
                snaptrade_account_id,
            )
            .await?;
            accounts.push(primary.clone());
            continue;
        }

        let created = accounts_table::create_account(
            pool,
            user_id,
            CreateAccountInput {
                name: snaptrade_account
                    .name
                    .clone()
                    .unwrap_or_else(|| "Brokerage Account".to_string()),
                icon: primary.icon.clone(),
                currency: primary.currency.clone(),
                broker: primary.broker.clone(),
                risk_profile: primary.risk_profile.clone(),
            },
        )
        .await?;
        let created = accounts_table::update_snaptrade_credentials(
            pool,
            &created.id,
            user_id,
            &snaptrade_user_id,
            &encrypted_secret,
            primary.snaptrade_connection_id.as_deref(),
        )
        .await?;
        let created = accounts_table::set_snaptrade_account_id(
            pool,
            &created.id,
            user_id,
            snaptrade_account_id,
        )
        .await?;
        accounts.push(created);
    }

    Ok(accounts)
}
