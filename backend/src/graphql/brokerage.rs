use async_graphql::{Context, Object, Result, SimpleObject};
use std::sync::Arc;

use chrono::Utc;

use crate::graphql::analytics::{AnalyticsRange, AnalyticsTimeFilterInput, map_time_filter};
use crate::service::brokerage::client::{BrokerageClient, SnapTradeError};
use crate::service::brokerage::db::decrypt_secret;
use crate::service::brokerage::transaction;
use crate::service::db::schema::tables::brokerage_table::{
    BrokerageBalance, BrokerageHolding, BrokerageTransaction, TransactionFilters,
};
use crate::service::db::schema::tables::workspaces_table;
use crate::service::read_service::analytics::resolve_range_bounds;
use crate::service::read_service::brokerage as brokerage_service;
use crate::service::redis::brokerage as brokerage_cache;
use crate::service::redis::client::RedisClient;

async fn get_user_db(ctx: &Context<'_>) -> Result<crate::service::db::client::UserDb> {
    crate::graphql::auth::user_db(ctx).await
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
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct SyncResult {
    pub status: String,
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
        workspace_id: String,
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
            limit: limit.unwrap_or(100).clamp(1, 500),
        };

        let redis = ctx.data::<Arc<RedisClient>>().ok();
        let page = match redis {
            Some(redis) => {
                let user_id = user_db.user_id().to_string();
                let acct = workspace_id.clone();
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
                    || brokerage_service::list_transactions(&user_db, &workspace_id, &filters),
                )
                .await?
            }
            None => brokerage_service::list_transactions(&user_db, &workspace_id, &filters).await?,
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
        workspace_id: String,
    ) -> Result<Vec<crate::service::brokerage::pending_trades::PendingTrade>> {
        let user_db = get_user_db(ctx).await?;
        Ok(brokerage_service::list_pending_trades(&user_db, &workspace_id).await?)
    }

    async fn brokerage_holdings(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<Vec<BrokerageHolding>> {
        let user_db = get_user_db(ctx).await?;
        let redis = ctx.data::<Arc<RedisClient>>().ok();
        match redis {
            Some(redis) => {
                let user_id = user_db.user_id().to_string();
                Ok(
                    brokerage_cache::get_or_load_holdings(redis, &user_id, &workspace_id, || {
                        brokerage_service::list_holdings(&user_db, &workspace_id)
                    })
                    .await?,
                )
            }
            None => Ok(brokerage_service::list_holdings(&user_db, &workspace_id).await?),
        }
    }

    async fn brokerage_balances(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<Vec<BrokerageBalance>> {
        let user_db = get_user_db(ctx).await?;
        let redis = ctx.data::<Arc<RedisClient>>().ok();
        match redis {
            Some(redis) => {
                let user_id = user_db.user_id().to_string();
                Ok(
                    brokerage_cache::get_or_load_balances(redis, &user_id, &workspace_id, || {
                        brokerage_service::list_balances(&user_db, &workspace_id)
                    })
                    .await?,
                )
            }
            None => Ok(brokerage_service::list_balances(&user_db, &workspace_id).await?),
        }
    }
}

// ── Mutation ────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct BrokerageMutation;

#[Object]
impl BrokerageMutation {
    /// Registers a SnapTrade user and initiates a brokerage connection.
    /// Returns only the short-lived portal URL. SnapTrade credentials never leave the backend.
    async fn initiate_brokerage_connection(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        brokerage_id: Option<String>,
        custom_redirect: Option<String>,
        reconnect: Option<bool>,
    ) -> Result<ConnectionPortal> {
        let user_db = get_user_db(ctx).await?;
        let brokerage_client = ctx.data::<Arc<BrokerageClient>>()?;

        // Check if account already has snaptrade credentials
        let account =
            workspaces_table::find_workspace(user_db.pool(), &workspace_id, user_db.user_id())
                .await?
                .ok_or_else(|| async_graphql::Error::new("Workspace not found"))?;

        // When reconnecting a disabled connection, repair the existing
        // authorization in place (pass its id as SnapTrade `reconnect`) instead
        // of creating a duplicate. Falls back to a fresh connect if we somehow
        // have no stored connection id.
        let reconnect_id: Option<String> = if reconnect.unwrap_or(false) {
            account.snaptrade_connection_id.clone()
        } else {
            None
        };

        let (snaptrade_user_id, user_secret) = if let Some(ref uid) = account.snaptrade_user_id {
            // Already registered — decrypt the existing secret
            let encrypted = account
                .snaptrade_user_secret_encrypted
                .ok_or_else(|| async_graphql::Error::new("No SnapTrade secret stored"))?;
            let secret = decrypt_secret(&encrypted)?;
            (uid.clone(), secret)
        } else {
            if let Some(existing) =
                workspaces_table::find_with_snaptrade_credentials(user_db.pool(), user_db.user_id())
                    .await?
            {
                let user_id = existing.snaptrade_user_id.ok_or_else(|| {
                    async_graphql::Error::new("Existing SnapTrade user ID is missing")
                })?;
                let encrypted = existing.snaptrade_user_secret_encrypted.ok_or_else(|| {
                    async_graphql::Error::new("Existing SnapTrade secret is missing")
                })?;
                let secret = decrypt_secret(&encrypted)?;
                workspaces_table::update_snaptrade_credentials(
                    user_db.pool(),
                    &workspace_id,
                    user_db.user_id(),
                    &user_id,
                    &encrypted,
                    None,
                )
                .await?;
                (user_id, secret)
            } else {
                let reg = crate::service::brokerage::db::register_and_store(
                    brokerage_client,
                    user_db.pool(),
                    &workspace_id,
                    user_db.user_id(),
                )
                .await
                .map_err(|e| async_graphql::Error::new(format!("Failed to register: {e}")))?;

                (reg.user_id, reg.user_secret)
            }
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
                    "SnapTrade rejected stored credentials for account={} user={} — \
                     queueing explicit user deletion before re-registration",
                    workspace_id,
                    user_db.user_id()
                );

                brokerage_client
                    .delete_user(&snaptrade_user_id)
                    .await
                    .map_err(|delete_error| {
                        async_graphql::Error::new(format!(
                            "Failed to reset stale SnapTrade credentials: {delete_error}"
                        ))
                    })?;

                workspaces_table::clear_snaptrade_credentials(
                    user_db.pool(),
                    &workspace_id,
                    user_db.user_id(),
                )
                .await?;
                return Err(async_graphql::Error::new(
                    "Your brokerage credentials are being reset. Please retry connecting shortly.",
                ));
            }
        };

        Ok(ConnectionPortal {
            redirect_url: portal.redirect_url,
        })
    }

    /// Completes the connection by storing the connection ID after the user returns from the SnapTrade portal.
    async fn complete_brokerage_connection(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        connection_id: String,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        let brokerage_client = ctx.data::<Arc<BrokerageClient>>()?;

        // Update just the connection_id on the account
        let account =
            workspaces_table::find_workspace(user_db.pool(), &workspace_id, user_db.user_id())
                .await?
                .ok_or_else(|| async_graphql::Error::new("Workspace not found"))?;

        let snaptrade_user_id = account
            .snaptrade_user_id
            .ok_or_else(|| async_graphql::Error::new("Workspace not registered with SnapTrade"))?;

        let encrypted = account
            .snaptrade_user_secret_encrypted
            .ok_or_else(|| async_graphql::Error::new("No SnapTrade secret stored"))?;

        let user_secret = decrypt_secret(&encrypted)?;
        let connection = brokerage_client
            .get_connection_status(&snaptrade_user_id, &user_secret, &connection_id)
            .await
            .map_err(|error| {
                async_graphql::Error::new(format!(
                    "SnapTrade did not confirm this connection for the current user: {error}"
                ))
            })?;
        if connection.id.as_deref() != Some(connection_id.as_str()) {
            return Err(async_graphql::Error::new(
                "SnapTrade returned a different connection identity",
            ));
        }

        workspaces_table::update_snaptrade_credentials(
            user_db.pool(),
            &workspace_id,
            user_db.user_id(),
            &snaptrade_user_id,
            &encrypted,
            Some(&connection_id),
        )
        .await?;

        workspaces_table::set_connection_freshness_mode(
            user_db.pool(),
            &workspace_id,
            user_db.user_id(),
            &connection.data_freshness_mode,
        )
        .await?;

        workspaces_table::set_connection_disabled(
            user_db.pool(),
            &workspace_id,
            user_db.user_id(),
            false,
            None,
        )
        .await?;

        match brokerage_client
            .list_snaptrade_accounts(&snaptrade_user_id, &user_secret)
            .await
        {
            Ok(snaptrade_accounts) => {
                crate::service::brokerage::workspaces::bind_workspace_brokerage_account(
                    user_db.pool(),
                    user_db.user_id(),
                    &workspace_id,
                    &snaptrade_accounts,
                )
                .await?;
            }
            Err(error) => {
                log::warn!(
                    "Connected brokerage but could not discover its accounts for account={workspace_id}: {error}"
                );
            }
        }

        Ok(true)
    }

    /// Removes the upstream connection before clearing local credentials.
    async fn disconnect_brokerage(&self, ctx: &Context<'_>, workspace_id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        let brokerage_client = ctx.data::<Arc<BrokerageClient>>()?;
        let workspace =
            workspaces_table::find_workspace(user_db.pool(), &workspace_id, user_db.user_id())
                .await?
                .ok_or_else(|| async_graphql::Error::new("Workspace not found"))?;
        if let (Some(snaptrade_user_id), Some(encrypted), Some(connection_id)) = (
            workspace.snaptrade_user_id.as_deref(),
            workspace.snaptrade_user_secret_encrypted.as_deref(),
            workspace.snaptrade_connection_id.as_deref(),
        ) {
            let other_references: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM brokerage_connections \
                 WHERE user_id = $1 AND snaptrade_connection_id = $2 AND workspace_id <> $3",
            )
            .bind(user_db.user_id())
            .bind(connection_id)
            .bind(&workspace_id)
            .fetch_one(user_db.pool())
            .await?;
            if other_references == 0 {
                brokerage_client
                    .delete_connection(
                        snaptrade_user_id,
                        &decrypt_secret(encrypted)?,
                        connection_id,
                    )
                    .await
                    .map_err(|error| {
                        async_graphql::Error::new(format!(
                            "Failed to disconnect the brokerage at SnapTrade: {error}"
                        ))
                    })?;
            }
        }
        workspaces_table::clear_snaptrade_credentials(
            user_db.pool(),
            &workspace_id,
            user_db.user_id(),
        )
        .await?;
        Ok(true)
    }

    async fn sync_brokerage_data(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<SyncResult> {
        let user_db = get_user_db(ctx).await?;
        let brokerage_client = ctx.data::<Arc<BrokerageClient>>()?;

        // Load account to get encrypted credentials
        let account =
            workspaces_table::find_workspace(user_db.pool(), &workspace_id, user_db.user_id())
                .await?
                .ok_or_else(|| async_graphql::Error::new("Workspace not found"))?;
        let mut snaptrade_account_id = account.snaptrade_account_id.clone();
        let connection_id = account
            .snaptrade_connection_id
            .clone()
            .ok_or_else(|| async_graphql::Error::new("Workspace has no SnapTrade connection"))?;

        let broker = account
            .broker
            .clone()
            .unwrap_or_else(|| "your brokerage".to_string());

        let snaptrade_user_id = account
            .snaptrade_user_id
            .ok_or_else(|| async_graphql::Error::new("Workspace not linked to SnapTrade"))?;

        let encrypted_secret = account
            .snaptrade_user_secret_encrypted
            .ok_or_else(|| async_graphql::Error::new("No SnapTrade secret stored"))?;

        let user_secret = decrypt_secret(&encrypted_secret)?;

        let connection = brokerage_client
            .get_connection_status(&snaptrade_user_id, &user_secret, &connection_id)
            .await
            .map_err(|error| {
                async_graphql::Error::new(format!(
                    "Failed to inspect SnapTrade connection: {error}"
                ))
            })?;
        if connection.disabled == Some(true) {
            return Err(async_graphql::Error::new(
                "Your brokerage connection needs to be reauthorized before it can sync.",
            ));
        }
        if connection.data_freshness_mode == "delayed" {
            brokerage_client
                .refresh_connection(&snaptrade_user_id, &user_secret, &connection_id)
                .await
                .map_err(|error| {
                    async_graphql::Error::new(format!("Failed to queue SnapTrade refresh: {error}"))
                })?;
            return Ok(SyncResult {
                status: "queued".to_string(),
                transactions_synced: 0,
                holdings_synced: 0,
                balances_synced: 0,
            });
        }

        // Discover SnapTrade account IDs (they differ from our internal workspace_id)
        let snaptrade_accounts = match brokerage_client
            .list_snaptrade_accounts(&snaptrade_user_id, &user_secret)
            .await
        {
            Ok(accounts) => accounts,
            Err(e) => {
                // Recovery here deliberately stops at flagging the account rather
                // than re-registering. Re-registration deletes the SnapTrade user
                // and with it the brokerage authorization, which is not something
                // a sync — often a background one — should do unprompted. Marking
                // the connection disabled surfaces the existing "reconnect" path,
                // which re-registers with the user's knowledge.
                if e.downcast_ref::<SnapTradeError>()
                    .is_some_and(|err| matches!(err, SnapTradeError::StaleCredentials))
                {
                    log::warn!(
                        "SnapTrade rejected stored credentials for account={} — flagging the \
                         connection as disabled so the user is prompted to reconnect",
                        workspace_id
                    );
                    workspaces_table::set_connection_disabled(
                        user_db.pool(),
                        &workspace_id,
                        user_db.user_id(),
                        true,
                        None,
                    )
                    .await?;

                    return Err(async_graphql::Error::new(
                        "Your brokerage connection needs to be reauthorized. \
                         Please reconnect the account to resume syncing.",
                    ));
                }

                return Err(async_graphql::Error::new(format!(
                    "Failed to list SnapTrade accounts: {e}"
                )));
            }
        };

        if snaptrade_accounts.is_empty() {
            log::warn!(
                "No SnapTrade accounts found for user_id={}",
                snaptrade_user_id
            );
            return Ok(SyncResult {
                status: "completed".to_string(),
                transactions_synced: 0,
                holdings_synced: 0,
                balances_synced: 0,
            });
        }

        if snaptrade_account_id.is_none() {
            crate::service::brokerage::workspaces::bind_workspace_brokerage_account(
                user_db.pool(),
                user_db.user_id(),
                &workspace_id,
                &snaptrade_accounts,
            )
            .await?;
            snaptrade_account_id =
                workspaces_table::find_workspace(user_db.pool(), &workspace_id, user_db.user_id())
                    .await?
                    .and_then(|account| account.snaptrade_account_id);
        }

        let snaptrade_account_id = snaptrade_account_id.ok_or_else(|| {
            async_graphql::Error::new("No SnapTrade account is available yet; try again shortly.")
        })?;
        let st_account = snaptrade_accounts
            .iter()
            .find(|candidate| candidate.id.as_deref() == Some(snaptrade_account_id.as_str()))
            .ok_or_else(|| {
                async_graphql::Error::new(
                    "This brokerage account is no longer available. Reconnect to refresh it.",
                )
            })?;

        log::info!(
            "Syncing SnapTrade account {} (name={:?}) for internal account {}",
            snaptrade_account_id,
            st_account.name,
            workspace_id
        );

        let total_tx = transaction::sync_transactions_if_advanced(
            brokerage_client.as_ref(),
            user_db.pool(),
            &snaptrade_user_id,
            &user_secret,
            &snaptrade_account_id,
            user_db.user_id(),
            &workspace_id,
            &broker,
            st_account
                .sync_status
                .as_ref()
                .and_then(|s| s.transactions.as_ref()),
            false,
        )
        .await
        .map_err(|error| {
            async_graphql::Error::new(format!(
                "Failed to sync transactions for SnapTrade account {snaptrade_account_id}: {error}"
            ))
        })?
        .unwrap_or(0) as i32;

        let (total_holdings, total_balances) = transaction::sync_holdings(
            brokerage_client.as_ref(),
            user_db.pool(),
            &snaptrade_user_id,
            &user_secret,
            &snaptrade_account_id,
            user_db.user_id(),
            &workspace_id,
        )
        .await
        .map_err(|error| {
            async_graphql::Error::new(format!(
                "Failed to sync portfolio for SnapTrade account {snaptrade_account_id}: {error}"
            ))
        })?;

        // Invalidate cache so next read fetches fresh data
        if let Ok(redis) = ctx.data::<Arc<RedisClient>>() {
            brokerage_cache::invalidate_account_cache(redis, user_db.user_id(), &workspace_id)
                .await;
        }

        Ok(SyncResult {
            status: "completed".to_string(),
            transactions_synced: total_tx,
            holdings_synced: total_holdings as i32,
            balances_synced: total_balances as i32,
        })
    }
}
