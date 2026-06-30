use async_graphql::{Context, Object, Result, SimpleObject};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use chrono::Utc;

use crate::graphql::analytics::{AnalyticsRange, AnalyticsTimeFilterInput, map_time_filter};
use crate::service::brokerage::client::BrokerageClient;
use crate::service::brokerage::db::{decrypt_secret, encrypt_secret};
use crate::service::brokerage::transaction;
use crate::service::db::Db;
use crate::service::db::schema::tables::accounts_table;
use crate::service::db::schema::tables::brokerage_table::{
    BrokerageBalance, BrokerageHolding, BrokerageTransaction, TransactionFilters,
};
use crate::service::read_service::analytics::resolve_range_bounds;
use crate::service::read_service::brokerage as brokerage_service;
use crate::service::read_service::users::ensure_user;
use crate::service::redis::brokerage as brokerage_cache;
use crate::service::redis::client::RedisClient;

async fn get_user_db(ctx: &Context<'_>) -> Result<crate::service::db::client::UserDb> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let db = ctx.data::<Arc<Db>>()?;
    let pool = db.pool();

    let full_name = jwt
        .other
        .get("full_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let email = jwt
        .other
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let user = ensure_user(pool, &jwt.sub, full_name, email).await?;
    Ok(db.get_user_db(&user.id))
}

// ── Response types ──────────────────────────────────────────────────────────

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct BrokerageTransactionsPage {
    pub data: Vec<BrokerageTransaction>,
    pub total: i32,
    pub offset: i32,
    pub limit: i32,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct ConnectionPortal {
    pub redirect_url: String,
    pub connection_id: String,
    pub snaptrade_user_id: String,
    pub snaptrade_user_secret: String,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct SyncResult {
    pub transactions_synced: i32,
    pub holdings_synced: i32,
    pub balances_synced: i32,
}

// ── Query ───────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct BrokerageQuery;

#[Object]
impl BrokerageQuery {
    #[allow(clippy::too_many_arguments)]
    async fn brokerage_transactions(
        &self,
        ctx: &Context<'_>,
        account_id: String,
        range: Option<AnalyticsRange>,
        start_date: Option<String>,
        end_date: Option<String>,
        transaction_type: Option<String>,
        symbol: Option<String>,
        offset: Option<i32>,
        limit: Option<i32>,
        sort_by: Option<String>,
        is_journalled: Option<bool>,
    ) -> Result<BrokerageTransactionsPage> {
        let user_db = get_user_db(ctx).await?;

        // A preset `range` (ET-anchored) overrides explicit start/end dates.
        let (range_start, range_end) = match range {
            Some(r) => {
                let filter = map_time_filter(AnalyticsTimeFilterInput {
                    range: r,
                    start_date: start_date.clone(),
                    end_date: end_date.clone(),
                })?;
                let bounds = resolve_range_bounds(&filter, Utc::now())
                    .map_err(|e| async_graphql::Error::new(e.to_string()))?;
                (
                    bounds
                        .start_date_et
                        .map(|d| d.format("%Y-%m-%d").to_string()),
                    bounds.end_date_et.map(|d| d.format("%Y-%m-%d").to_string()),
                )
            }
            None => (start_date.clone(), end_date.clone()),
        };

        let filters = TransactionFilters {
            start_date: range_start,
            end_date: range_end,
            transaction_type: transaction_type.clone(),
            symbol: symbol.clone(),
            sort_by: sort_by.clone(),
            is_journalled,
            offset: offset.unwrap_or(0),
            limit: limit.unwrap_or(1000).min(1000),
        };

        let redis = ctx.data::<Arc<RedisClient>>().ok();
        let page = match redis {
            Some(redis) => {
                let user_id = user_db.user_id().to_string();
                let acct = account_id.clone();
                brokerage_cache::get_or_load_transactions(
                    redis,
                    &user_id,
                    &acct,
                    filters.start_date.as_deref(),
                    filters.end_date.as_deref(),
                    filters.transaction_type.as_deref(),
                    filters.symbol.as_deref(),
                    filters.sort_by.as_deref(),
                    filters.is_journalled,
                    filters.offset,
                    filters.limit,
                    || brokerage_service::list_transactions(&user_db, &account_id, &filters),
                )
                .await?
            }
            None => brokerage_service::list_transactions(&user_db, &account_id, &filters).await?,
        };

        Ok(BrokerageTransactionsPage {
            data: page.data,
            total: page.total,
            offset: page.offset,
            limit: page.limit,
        })
    }

    async fn brokerage_transaction(
        &self,
        ctx: &Context<'_>,
        id: String,
    ) -> Result<Option<BrokerageTransaction>> {
        let user_db = get_user_db(ctx).await?;
        Ok(brokerage_service::get_transaction(&user_db, &id).await?)
    }

    /// Fetch a batch of transactions by id (scoped to the requesting user).
    /// Used by the pending-trade prefill in the MergeTradesModal.
    async fn brokerage_transactions_by_ids(
        &self,
        ctx: &Context<'_>,
        ids: Vec<String>,
    ) -> Result<Vec<BrokerageTransaction>> {
        let user_db = get_user_db(ctx).await?;
        Ok(brokerage_service::get_transactions_by_ids(&user_db, &ids).await?)
    }

    /// Round-trip trade lifecycles that haven't been fully journaled. Groups
    /// fills across months/years so a position opened in April and closed
    /// in May shows up as one journaling target.
    async fn pending_trades(
        &self,
        ctx: &Context<'_>,
        account_id: String,
    ) -> Result<Vec<crate::service::brokerage::pending_trades::PendingTrade>> {
        let user_db = get_user_db(ctx).await?;
        Ok(brokerage_service::list_pending_trades(&user_db, &account_id).await?)
    }

    async fn brokerage_holdings(
        &self,
        ctx: &Context<'_>,
        account_id: String,
    ) -> Result<Vec<BrokerageHolding>> {
        let user_db = get_user_db(ctx).await?;
        let redis = ctx.data::<Arc<RedisClient>>().ok();
        match redis {
            Some(redis) => {
                let user_id = user_db.user_id().to_string();
                Ok(
                    brokerage_cache::get_or_load_holdings(redis, &user_id, &account_id, || {
                        brokerage_service::list_holdings(&user_db, &account_id)
                    })
                    .await?,
                )
            }
            None => Ok(brokerage_service::list_holdings(&user_db, &account_id).await?),
        }
    }

    async fn brokerage_balances(
        &self,
        ctx: &Context<'_>,
        account_id: String,
    ) -> Result<Vec<BrokerageBalance>> {
        let user_db = get_user_db(ctx).await?;
        let redis = ctx.data::<Arc<RedisClient>>().ok();
        match redis {
            Some(redis) => {
                let user_id = user_db.user_id().to_string();
                Ok(
                    brokerage_cache::get_or_load_balances(redis, &user_id, &account_id, || {
                        brokerage_service::list_balances(&user_db, &account_id)
                    })
                    .await?,
                )
            }
            None => Ok(brokerage_service::list_balances(&user_db, &account_id).await?),
        }
    }
}

// ── Mutation ────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct BrokerageMutation;

#[Object]
impl BrokerageMutation {
    /// Registers a SnapTrade user and initiates a brokerage connection.
    /// Returns a redirect URL for the SnapTrade portal plus credentials to store after callback.
    async fn initiate_brokerage_connection(
        &self,
        ctx: &Context<'_>,
        account_id: String,
        brokerage_id: Option<String>,
        custom_redirect: Option<String>,
        reconnect: Option<bool>,
    ) -> Result<ConnectionPortal> {
        let user_db = get_user_db(ctx).await?;
        let brokerage_client = ctx.data::<Arc<BrokerageClient>>()?;

        // Check if account already has snaptrade credentials
        let account = accounts_table::find_account(user_db.pool(), &account_id, user_db.user_id())
            .await?
            .ok_or_else(|| async_graphql::Error::new("Account not found"))?;

        // When reconnecting a disabled connection, repair the existing
        // authorization in place (pass its id as SnapTrade `reconnect`) instead
        // of creating a duplicate. Falls back to a fresh connect if we somehow
        // have no stored connection id.
        let reconnect_id: Option<String> = if reconnect.unwrap_or(false) {
            account.snaptrade_connection_id.clone()
        } else {
            None
        };

        let (mut snaptrade_user_id, mut user_secret) =
            if let Some(ref uid) = account.snaptrade_user_id {
                // Already registered — decrypt the existing secret
                let encrypted = account
                    .snaptrade_user_secret_encrypted
                    .ok_or_else(|| async_graphql::Error::new("No SnapTrade secret stored"))?;
                let secret = decrypt_secret(&encrypted)?;
                (uid.clone(), secret)
            } else {
                // Register a new SnapTrade user
                let reg = brokerage_client
                    .register_user(user_db.user_id())
                    .await
                    .map_err(|e| async_graphql::Error::new(format!("Failed to register: {e}")))?;

                // Persist credentials immediately so they aren't lost if the portal step fails
                let encrypted = encrypt_secret(&reg.user_secret)?;
                accounts_table::update_snaptrade_credentials(
                    user_db.pool(),
                    &account_id,
                    user_db.user_id(),
                    &reg.user_id,
                    &encrypted,
                    None,
                )
                .await?;

                (reg.user_id, reg.user_secret)
            };

        // First attempt with the (possibly stored) credentials.
        let portal = match brokerage_client
            .initiate_connection(
                &snaptrade_user_id,
                &user_secret,
                brokerage_id.as_deref().unwrap_or(""),
                None,
                reconnect_id.as_deref(),
                custom_redirect.as_deref(),
            )
            .await
        {
            Ok(p) => p,
            Err(e) => {
                // Detect SnapTrade's "Invalid userID or userSecret" (code 1083)
                // via typed downcast and self-heal. Happens when
                // SNAPTRADE_CLIENT_ID was rotated or the user was deleted on
                // SnapTrade's side — the stored credentials are zombies
                // pointing at a different tenant.
                let is_stale_creds = e
                    .downcast_ref::<crate::service::brokerage::client::SnapTradeError>()
                    .map(|err| {
                        matches!(
                            err,
                            crate::service::brokerage::client::SnapTradeError::StaleCredentials
                        )
                    })
                    .unwrap_or(false);

                if !is_stale_creds {
                    return Err(async_graphql::Error::new(format!(
                        "Failed to initiate connection: {e}"
                    )));
                }

                log::warn!(
                    "SnapTrade rejected stored credentials (code 1083) for account={} \
                     user={} — clearing creds and re-registering",
                    account_id,
                    user_db.user_id()
                );

                accounts_table::clear_snaptrade_credentials(
                    user_db.pool(),
                    &account_id,
                    user_db.user_id(),
                )
                .await?;

                // Drop historical transactions for this account. They were
                // upserted under the old SnapTrade user's snaptrade_id values;
                // the next sync against the new connection will produce fresh
                // snaptrade_ids for the same trades and would duplicate-insert
                // alongside the old rows (upsert key is snaptrade_id).
                if let Err(e) =
                    crate::service::db::schema::tables::brokerage_table::delete_transactions_for_account(
                        user_db.pool(),
                        user_db.user_id(),
                        &account_id,
                    )
                    .await
                {
                    log::warn!(
                        "Failed to clear stale brokerage_transactions for account={} during \
                         recovery: {e} — re-registration will proceed but duplicate fills may \
                         appear after the next sync",
                        account_id
                    );
                }

                let reg = brokerage_client
                    .register_user(user_db.user_id())
                    .await
                    .map_err(|e| {
                        async_graphql::Error::new(format!(
                            "Failed to re-register after stale-cred recovery: {e}"
                        ))
                    })?;

                let encrypted = encrypt_secret(&reg.user_secret)?;
                accounts_table::update_snaptrade_credentials(
                    user_db.pool(),
                    &account_id,
                    user_db.user_id(),
                    &reg.user_id,
                    &encrypted,
                    None,
                )
                .await?;

                let retry_portal = brokerage_client
                    .initiate_connection(
                        &reg.user_id,
                        &reg.user_secret,
                        brokerage_id.as_deref().unwrap_or(""),
                        None,
                        None,
                        custom_redirect.as_deref(),
                    )
                    .await
                    .map_err(|e| {
                        async_graphql::Error::new(format!(
                            "Failed to initiate connection after re-register: {e}"
                        ))
                    })?;

                // Rebind so the response carries the fresh credentials.
                snaptrade_user_id = reg.user_id;
                user_secret = reg.user_secret;
                retry_portal
            }
        };

        Ok(ConnectionPortal {
            redirect_url: portal.redirect_url,
            connection_id: portal.connection_id,
            snaptrade_user_id,
            snaptrade_user_secret: user_secret,
        })
    }

    /// Completes the connection by storing the connection ID after the user returns from the SnapTrade portal.
    async fn complete_brokerage_connection(
        &self,
        ctx: &Context<'_>,
        account_id: String,
        connection_id: String,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;

        // Update just the connection_id on the account
        let account = accounts_table::find_account(user_db.pool(), &account_id, user_db.user_id())
            .await?
            .ok_or_else(|| async_graphql::Error::new("Account not found"))?;

        let snaptrade_user_id = account
            .snaptrade_user_id
            .ok_or_else(|| async_graphql::Error::new("Account not registered with SnapTrade"))?;

        let encrypted = account
            .snaptrade_user_secret_encrypted
            .ok_or_else(|| async_graphql::Error::new("No SnapTrade secret stored"))?;

        accounts_table::update_snaptrade_credentials(
            user_db.pool(),
            &account_id,
            user_db.user_id(),
            &snaptrade_user_id,
            &encrypted,
            Some(&connection_id),
        )
        .await?;

        accounts_table::set_connection_disabled(
            user_db.pool(),
            &account_id,
            user_db.user_id(),
            false,
            None,
        )
        .await?;

        Ok(true)
    }

    async fn link_snaptrade_account(
        &self,
        ctx: &Context<'_>,
        account_id: String,
        snaptrade_user_id: String,
        snaptrade_user_secret: String,
        snaptrade_connection_id: Option<String>,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        let encrypted = encrypt_secret(&snaptrade_user_secret)?;

        accounts_table::update_snaptrade_credentials(
            user_db.pool(),
            &account_id,
            user_db.user_id(),
            &snaptrade_user_id,
            &encrypted,
            snaptrade_connection_id.as_deref(),
        )
        .await?;

        Ok(true)
    }

    /// Disconnects the brokerage by clearing all SnapTrade credentials from the account.
    async fn disconnect_brokerage(&self, ctx: &Context<'_>, account_id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        accounts_table::clear_snaptrade_credentials(user_db.pool(), &account_id, user_db.user_id())
            .await?;
        Ok(true)
    }

    async fn sync_brokerage_data(
        &self,
        ctx: &Context<'_>,
        account_id: String,
    ) -> Result<SyncResult> {
        let user_db = get_user_db(ctx).await?;
        let brokerage_client = ctx.data::<Arc<BrokerageClient>>()?;

        // Load account to get encrypted credentials
        let account = accounts_table::find_account(user_db.pool(), &account_id, user_db.user_id())
            .await?
            .ok_or_else(|| async_graphql::Error::new("Account not found"))?;

        let snaptrade_user_id = account
            .snaptrade_user_id
            .ok_or_else(|| async_graphql::Error::new("Account not linked to SnapTrade"))?;

        let encrypted_secret = account
            .snaptrade_user_secret_encrypted
            .ok_or_else(|| async_graphql::Error::new("No SnapTrade secret stored"))?;

        let user_secret = decrypt_secret(&encrypted_secret)?;

        // Discover SnapTrade account IDs (they differ from our internal account_id)
        let snaptrade_accounts = brokerage_client
            .list_snaptrade_accounts(&snaptrade_user_id, &user_secret)
            .await
            .map_err(|e| {
                async_graphql::Error::new(format!("Failed to list SnapTrade accounts: {e}"))
            })?;

        if snaptrade_accounts.is_empty() {
            log::warn!(
                "No SnapTrade accounts found for user_id={}",
                snaptrade_user_id
            );
            return Ok(SyncResult {
                transactions_synced: 0,
                holdings_synced: 0,
                balances_synced: 0,
            });
        }

        let mut total_tx = 0i32;
        let mut total_holdings = 0i32;
        let mut total_balances = 0i32;

        for st_account in &snaptrade_accounts {
            let st_account_id = match &st_account.id {
                Some(id) => id.clone(),
                None => continue,
            };

            log::info!(
                "Syncing SnapTrade account {} (name={:?}) for internal account {}",
                st_account_id,
                st_account.name,
                account_id
            );

            // Sync transactions (best-effort)
            let tx_count = transaction::sync_transactions(
                brokerage_client.as_ref(),
                user_db.pool(),
                &snaptrade_user_id,
                &user_secret,
                &st_account_id,
                &account_id,
            )
            .await
            .unwrap_or_else(|e| {
                log::warn!(
                    "Failed to sync transactions for st_account={}: {}",
                    st_account_id,
                    e
                );
                0
            });
            total_tx += tx_count as i32;

            // Sync holdings + balances (best-effort)
            let (holdings_count, balances_count) = transaction::sync_holdings(
                brokerage_client.as_ref(),
                user_db.pool(),
                &snaptrade_user_id,
                &user_secret,
                &st_account_id,
                &account_id,
            )
            .await
            .unwrap_or_else(|e| {
                log::warn!(
                    "Failed to sync holdings for st_account={}: {:?}",
                    st_account_id,
                    e
                );
                (0, 0)
            });
            total_holdings += holdings_count as i32;
            total_balances += balances_count as i32;
        }

        // Invalidate cache so next read fetches fresh data
        if let Ok(redis) = ctx.data::<Arc<RedisClient>>() {
            brokerage_cache::invalidate_account_cache(redis, user_db.user_id(), &account_id).await;
        }

        Ok(SyncResult {
            transactions_synced: total_tx,
            holdings_synced: total_holdings,
            balances_synced: total_balances,
        })
    }
}
