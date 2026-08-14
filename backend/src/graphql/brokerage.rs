use async_graphql::{Context, InputObject, Object, Result, SimpleObject};
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

use chrono::Utc;
use uuid::Uuid;

use crate::graphql::analytics::{AnalyticsRange, AnalyticsTimeFilterInput, map_time_filter};
use crate::service::brokerage::client::{BrokerageClient, SnapTradeAccount, SnapTradeError};
use crate::service::brokerage::db::decrypt_secret;
use crate::service::brokerage::transaction;
use crate::service::db::schema::tables::brokerage_table::{
    BrokerageBalance, BrokerageHolding, BrokerageTransaction, TransactionFilters,
};
use crate::service::db::schema::tables::{
    brokerage_data_report_table, brokerage_reconciliation_table, trade_review_table,
    workspaces_table,
};
use crate::service::read_service::analytics::resolve_range_bounds;
use crate::service::read_service::brokerage as brokerage_service;
use crate::service::redis::brokerage as brokerage_cache;
use crate::service::redis::client::RedisClient;

async fn get_user_db(ctx: &Context<'_>) -> Result<crate::service::db::client::UserDb> {
    crate::graphql::auth::user_db(ctx).await
}

fn is_stale_credentials(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<SnapTradeError>()
        .is_some_and(|error| matches!(error, SnapTradeError::StaleCredentials))
}

const DELAYED_REFRESH_POLL_DELAYS: [Duration; 4] = [
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(15),
    Duration::from_secs(30),
];

static ACTIVE_SYNCS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn active_syncs() -> &'static Mutex<HashSet<String>> {
    ACTIVE_SYNCS.get_or_init(|| Mutex::new(HashSet::new()))
}

async fn begin_sync(key: &str) -> bool {
    active_syncs().lock().await.insert(key.to_string())
}

async fn finish_sync(key: &str) {
    active_syncs().lock().await.remove(key);
}

fn safe_sync_error(error: &str) -> String {
    error
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(500)
        .collect()
}

fn holdings_sync_mark(account: &SnapTradeAccount) -> Option<String> {
    account
        .sync_status
        .as_ref()
        .and_then(|status| status.holdings.as_ref())
        .and_then(|status| status.last_successful_sync.clone())
}

fn holdings_refresh_completed(baseline: Option<&str>, account: &SnapTradeAccount) -> bool {
    let Some(status) = account
        .sync_status
        .as_ref()
        .and_then(|status| status.holdings.as_ref())
    else {
        return false;
    };
    if status.initial_sync_completed == Some(false) {
        return false;
    }
    let Some(current) = status.last_successful_sync.as_deref() else {
        return false;
    };
    baseline.is_none_or(|baseline| current > baseline)
}

struct DelayedRefreshTask {
    key: String,
    diagnostic_id: String,
    brokerage: Arc<BrokerageClient>,
    redis: Option<Arc<RedisClient>>,
    pool: PgPool,
    user_id: String,
    workspace_id: String,
    snaptrade_user_id: String,
    user_secret: String,
    snaptrade_account_id: String,
    broker: String,
    baseline_holdings_mark: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct SyncCounts {
    transactions: i32,
    holdings: i32,
    balances: i32,
}

fn delayed_sync_counts(
    transactions: anyhow::Result<Option<transaction::TransactionSyncReport>>,
    portfolio: anyhow::Result<Option<transaction::PortfolioSyncReport>>,
) -> anyhow::Result<SyncCounts> {
    let transactions = transactions?.map_or(0, |report| report.mapped_count);
    let portfolio = portfolio?.unwrap_or_default();
    Ok(SyncCounts {
        transactions,
        holdings: portfolio.holdings_synced,
        balances: portfolio.balances_synced,
    })
}

async fn run_delayed_refresh_follow_up(task: &DelayedRefreshTask) -> anyhow::Result<SyncCounts> {
    for delay in DELAYED_REFRESH_POLL_DELAYS {
        sleep(delay).await;
        let accounts = match task
            .brokerage
            .list_snaptrade_accounts(&task.snaptrade_user_id, &task.user_secret)
            .await
        {
            Ok(accounts) => accounts,
            Err(error) => {
                log::warn!(
                    "Delayed SnapTrade refresh poll failed for account={}: {error}",
                    task.workspace_id
                );
                continue;
            }
        };
        let Some(account) = accounts
            .iter()
            .find(|candidate| candidate.id.as_deref() == Some(task.snaptrade_account_id.as_str()))
        else {
            log::warn!(
                "Delayed SnapTrade refresh could not find the bound account for workspace={}",
                task.workspace_id
            );
            continue;
        };
        if !holdings_refresh_completed(task.baseline_holdings_mark.as_deref(), account) {
            continue;
        }

        let transaction_status = account
            .sync_status
            .as_ref()
            .and_then(|status| status.transactions.as_ref());
        let holdings_status = account
            .sync_status
            .as_ref()
            .and_then(|status| status.holdings.as_ref());
        let (transactions, portfolio) = tokio::join!(
            transaction::sync_transactions_if_advanced(
                task.brokerage.as_ref(),
                &task.pool,
                &task.snaptrade_user_id,
                &task.user_secret,
                &task.snaptrade_account_id,
                &task.user_id,
                &task.workspace_id,
                &task.broker,
                transaction_status,
                false,
            ),
            transaction::sync_holdings_if_advanced(
                task.brokerage.as_ref(),
                &task.pool,
                &task.snaptrade_user_id,
                &task.user_secret,
                &task.snaptrade_account_id,
                &task.user_id,
                &task.workspace_id,
                holdings_status,
                "delayed",
                false,
            ),
        );

        let counts = delayed_sync_counts(transactions, portfolio)?;
        if let Some(redis) = &task.redis {
            brokerage_cache::invalidate_account_cache(redis, &task.user_id, &task.workspace_id)
                .await;
        }
        log::info!(
            "Delayed SnapTrade refresh completed for workspace={}: {} transactions, {} holdings, {} balances",
            task.workspace_id,
            counts.transactions,
            counts.holdings,
            counts.balances,
        );
        return Ok(counts);
    }

    anyhow::bail!("SnapTrade did not advance the holdings refresh marker within 60 seconds")
}

fn spawn_delayed_refresh_follow_up(task: DelayedRefreshTask) {
    tokio::spawn(async move {
        let key = task.key.clone();
        match run_delayed_refresh_follow_up(&task).await {
            Ok(counts) => {
                if let Err(error) = workspaces_table::mark_brokerage_sync_completed(
                    &task.pool,
                    &task.workspace_id,
                    &task.user_id,
                    &task.diagnostic_id,
                    counts.transactions,
                    counts.holdings,
                    counts.balances,
                )
                .await
                {
                    log::warn!(
                        "Failed to record completed brokerage sync for workspace={}: {error}",
                        task.workspace_id
                    );
                }
            }
            Err(error) => {
                let message = safe_sync_error(&error.to_string());
                log::warn!(
                    "Delayed SnapTrade refresh follow-up stopped for workspace={}: {message}",
                    task.workspace_id
                );
                if let Err(record_error) = workspaces_table::mark_brokerage_sync_failed(
                    &task.pool,
                    &task.workspace_id,
                    &task.user_id,
                    &task.diagnostic_id,
                    &message,
                )
                .await
                {
                    log::warn!(
                        "Failed to record brokerage sync failure for workspace={}: {record_error}",
                        task.workspace_id
                    );
                }
            }
        }
        finish_sync(&key).await;
    });
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
pub struct BrokerageConnectionAccount {
    pub id: String,
    pub name: String,
    pub institution_name: Option<String>,
    pub linked_workspace_id: Option<String>,
    pub linked_workspace_name: Option<String>,
    pub current: bool,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct SyncResult {
    pub status: String,
    pub transactions_synced: i32,
    pub holdings_synced: i32,
    pub balances_synced: i32,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct BrokerageSyncOutcome {
    pub diagnostic_id: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub succeeded_at: Option<String>,
    pub next_scheduled_at: Option<String>,
    pub transactions_synced: i32,
    pub holdings_synced: i32,
    pub balances_synced: i32,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct BrokerageReconciliation {
    pub diagnostic_id: String,
    pub transaction_status: String,
    pub transaction_checked_at: Option<String>,
    pub broker_transaction_count: i32,
    pub mapped_transaction_count: i32,
    pub imported_transaction_count: i32,
    pub duplicate_transaction_count: i32,
    pub skipped_transaction_count: i32,
    pub pending_transaction_count: i32,
    pub failed_transaction_count: i32,
    pub local_transaction_count: i32,
    pub missing_transaction_count: i32,
    pub extra_transaction_count: i32,
    pub portfolio_status: String,
    pub portfolio_checked_at: Option<String>,
    pub broker_holding_count: i32,
    pub mapped_holding_count: i32,
    pub local_holding_count: i32,
    pub broker_balance_count: i32,
    pub local_balance_count: i32,
    pub balance_discrepancy_count: i32,
    pub transaction_error: Option<String>,
    pub portfolio_error: Option<String>,
}

#[derive(InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct ReportBrokerageDataIssueInput {
    pub workspace_id: String,
    pub category: String,
    pub note: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct BrokerageDataIssueReport {
    pub id: String,
    pub diagnostic_id: String,
    pub created_at: String,
}

const BROKERAGE_REPORT_CATEGORIES: [&str; 5] =
    ["transactions", "holdings", "balances", "account", "other"];

fn normalize_brokerage_report_input(
    category: &str,
    note: Option<&str>,
) -> Result<(String, Option<String>)> {
    let category = category.trim();
    if !BROKERAGE_REPORT_CATEGORIES.contains(&category) {
        return Err(async_graphql::Error::new("Choose a valid brokerage issue"));
    }
    let note = note
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if note
        .as_ref()
        .is_some_and(|value| value.chars().count() > 1000)
    {
        return Err(async_graphql::Error::new(
            "The report note must be 1,000 characters or fewer",
        ));
    }
    if category == "other" && note.is_none() {
        return Err(async_graphql::Error::new(
            "Add a short note describing what looks wrong",
        ));
    }
    Ok((category.to_string(), note))
}

fn brokerage_report_snapshot(
    broker: Option<&str>,
    snaptrade_account_id: &str,
    outcome: Option<&workspaces_table::BrokerageSyncOutcome>,
    reconciliation: Option<&brokerage_reconciliation_table::BrokerageReconciliationState>,
) -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "broker": broker,
        "snaptradeAccountId": snaptrade_account_id,
        "sync": outcome.map(|value| serde_json::json!({
            "diagnosticId": value.diagnostic_id,
            "status": value.status,
            "startedAt": value.started_at,
            "finishedAt": value.finished_at,
            "succeededAt": value.succeeded_at,
            "transactionsSynced": value.transactions_synced,
            "holdingsSynced": value.holdings_synced,
            "balancesSynced": value.balances_synced,
        })),
        "reconciliation": reconciliation.map(|value| serde_json::json!({
            "diagnosticId": value.diagnostic_id,
            "transactionStatus": value.transaction_status,
            "transactionCheckedAt": value.transaction_checked_at,
            "brokerTransactionCount": value.broker_transaction_count,
            "mappedTransactionCount": value.mapped_transaction_count,
            "importedTransactionCount": value.imported_transaction_count,
            "duplicateTransactionCount": value.duplicate_transaction_count,
            "skippedTransactionCount": value.skipped_transaction_count,
            "pendingTransactionCount": value.pending_transaction_count,
            "failedTransactionCount": value.failed_transaction_count,
            "localTransactionCount": value.local_transaction_count,
            "missingTransactionCount": value.missing_transaction_count,
            "extraTransactionCount": value.extra_transaction_count,
            "portfolioStatus": value.portfolio_status,
            "portfolioCheckedAt": value.portfolio_checked_at,
            "brokerHoldingCount": value.broker_holding_count,
            "mappedHoldingCount": value.mapped_holding_count,
            "localHoldingCount": value.local_holding_count,
            "brokerBalanceCount": value.broker_balance_count,
            "localBalanceCount": value.local_balance_count,
            "balanceDiscrepancyCount": value.balance_discrepancy_count,
        })),
    })
}

// ── Query ───────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct BrokerageQuery;

#[Object]
impl BrokerageQuery {
    async fn brokerage_reconciliation(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<Option<BrokerageReconciliation>> {
        let user_db = get_user_db(ctx).await?;
        let Some(workspace) =
            workspaces_table::find_workspace(user_db.pool(), &workspace_id, user_db.user_id())
                .await?
        else {
            return Ok(None);
        };
        let Some(snaptrade_account_id) = workspace.snaptrade_account_id else {
            return Ok(None);
        };

        Ok(brokerage_reconciliation_table::get_for_workspace(
            user_db.pool(),
            user_db.user_id(),
            &workspace_id,
            &snaptrade_account_id,
        )
        .await?
        .map(|state| BrokerageReconciliation {
            diagnostic_id: state.diagnostic_id,
            transaction_status: state.transaction_status,
            transaction_checked_at: state.transaction_checked_at,
            broker_transaction_count: state.broker_transaction_count,
            mapped_transaction_count: state.mapped_transaction_count,
            imported_transaction_count: state.imported_transaction_count,
            duplicate_transaction_count: state.duplicate_transaction_count,
            skipped_transaction_count: state.skipped_transaction_count,
            pending_transaction_count: state.pending_transaction_count,
            failed_transaction_count: state.failed_transaction_count,
            local_transaction_count: state.local_transaction_count,
            missing_transaction_count: state.missing_transaction_count,
            extra_transaction_count: state.extra_transaction_count,
            portfolio_status: state.portfolio_status,
            portfolio_checked_at: state.portfolio_checked_at,
            broker_holding_count: state.broker_holding_count,
            mapped_holding_count: state.mapped_holding_count,
            local_holding_count: state.local_holding_count,
            broker_balance_count: state.broker_balance_count,
            local_balance_count: state.local_balance_count,
            balance_discrepancy_count: state.balance_discrepancy_count,
            transaction_error: state.transaction_error,
            portfolio_error: state.portfolio_error,
        }))
    }

    async fn brokerage_sync_outcome(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<Option<BrokerageSyncOutcome>> {
        let user_db = get_user_db(ctx).await?;
        let Some(workspace) =
            workspaces_table::find_workspace(user_db.pool(), &workspace_id, user_db.user_id())
                .await?
        else {
            return Ok(None);
        };
        let next_scheduled_at = if workspace.snaptrade_connection_id.is_some()
            && !workspace.snaptrade_connection_disabled
        {
            crate::service::brokerage::sync::next_scheduled_sync(Utc::now())
                .map(|date| date.to_rfc3339())
        } else {
            None
        };
        Ok(workspaces_table::brokerage_sync_outcome(
            user_db.pool(),
            &workspace_id,
            user_db.user_id(),
        )
        .await?
        .map(|outcome| BrokerageSyncOutcome {
            diagnostic_id: outcome.diagnostic_id,
            status: outcome.status,
            error: outcome.error,
            started_at: outcome.started_at,
            finished_at: outcome.finished_at,
            succeeded_at: outcome.succeeded_at,
            next_scheduled_at,
            transactions_synced: outcome.transactions_synced,
            holdings_synced: outcome.holdings_synced,
            balances_synced: outcome.balances_synced,
        }))
    }

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

    /// Lists every upstream account exposed by this workspace's brokerage
    /// authorization and whether it is already linked to a Tradstry workspace.
    async fn brokerage_connection_accounts(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<Vec<BrokerageConnectionAccount>> {
        let user_db = get_user_db(ctx).await?;
        let brokerage_client = ctx.data::<Arc<BrokerageClient>>()?;
        let workspace =
            workspaces_table::find_workspace(user_db.pool(), &workspace_id, user_db.user_id())
                .await?
                .ok_or_else(|| async_graphql::Error::new("Workspace not found"))?;
        let snaptrade_user_id = workspace
            .snaptrade_user_id
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("Workspace not linked to SnapTrade"))?;
        let encrypted = workspace
            .snaptrade_user_secret_encrypted
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("No SnapTrade secret stored"))?;
        let connection_id = workspace
            .snaptrade_connection_id
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("Workspace has no brokerage connection"))?;

        let user_secret = decrypt_secret(encrypted)?;
        let (accounts, workspaces) = tokio::try_join!(
            brokerage_client.list_snaptrade_accounts(snaptrade_user_id, &user_secret),
            workspaces_table::list_workspaces(user_db.pool(), user_db.user_id()),
        )?;

        let mut options: Vec<_> = accounts
            .into_iter()
            .filter(|account| {
                account.id.is_some()
                    && account.brokerage_authorization.as_deref() == Some(connection_id)
            })
            .map(|account| {
                let id = account
                    .id
                    .as_deref()
                    .expect("filtered accounts have ids")
                    .to_string();
                let linked = workspaces.iter().find(|candidate| {
                    candidate.snaptrade_account_id.as_deref() == Some(id.as_str())
                });
                BrokerageConnectionAccount {
                    current: workspace.snaptrade_account_id.as_deref() == Some(id.as_str()),
                    name: crate::service::brokerage::workspaces::brokerage_account_name(&account),
                    institution_name: account.institution_name,
                    linked_workspace_id: linked.map(|workspace| workspace.id.clone()),
                    linked_workspace_name: linked.map(|workspace| workspace.name.clone()),
                    id,
                }
            })
            .collect();
        options.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        Ok(options)
    }
}

// ── Mutation ────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct BrokerageMutation;

#[Object]
impl BrokerageMutation {
    /// Records a user-confirmed data issue with a server-built, sanitized
    /// diagnostic snapshot. The client never supplies broker payloads, counts,
    /// account identifiers, or credentials.
    async fn report_brokerage_data_issue(
        &self,
        ctx: &Context<'_>,
        input: ReportBrokerageDataIssueInput,
    ) -> Result<BrokerageDataIssueReport> {
        let user_db = get_user_db(ctx).await?;
        let (category, note) =
            normalize_brokerage_report_input(&input.category, input.note.as_deref())?;
        let workspace = workspaces_table::find_workspace(
            user_db.pool(),
            &input.workspace_id,
            user_db.user_id(),
        )
        .await?
        .ok_or_else(|| async_graphql::Error::new("Workspace not found"))?;
        let snaptrade_account_id = workspace
            .snaptrade_account_id
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("No brokerage account is linked here"))?;

        let (outcome, reconciliation) = tokio::try_join!(
            workspaces_table::brokerage_sync_outcome(
                user_db.pool(),
                &input.workspace_id,
                user_db.user_id(),
            ),
            brokerage_reconciliation_table::get_for_workspace(
                user_db.pool(),
                user_db.user_id(),
                &input.workspace_id,
                snaptrade_account_id,
            ),
        )?;
        let diagnostic_id = reconciliation
            .as_ref()
            .map(|value| value.diagnostic_id.clone())
            .or_else(|| {
                outcome
                    .as_ref()
                    .and_then(|value| value.diagnostic_id.clone())
            })
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let snapshot = brokerage_report_snapshot(
            workspace.broker.as_deref(),
            snaptrade_account_id,
            outcome.as_ref(),
            reconciliation.as_ref(),
        );
        let id = Uuid::new_v4().to_string();
        let report = brokerage_data_report_table::create(
            user_db.pool(),
            brokerage_data_report_table::CreateBrokerageDataReport {
                id: &id,
                user_id: user_db.user_id(),
                workspace_id: &input.workspace_id,
                snaptrade_account_id,
                diagnostic_id: &diagnostic_id,
                category: &category,
                note: note.as_deref(),
                diagnostic_snapshot: &snapshot,
            },
        )
        .await?;
        log::info!(
            "Brokerage data issue reported: report_id={} diagnostic_id={} workspace_id={} category={}",
            report.id,
            report.diagnostic_id,
            input.workspace_id,
            category,
        );

        Ok(BrokerageDataIssueReport {
            id: report.id,
            diagnostic_id: report.diagnostic_id,
            created_at: report.created_at,
        })
    }

    /// Replaces an automatic trade grouping with the user's selected broker
    /// fills. The executions remain immutable; only episode membership changes.
    async fn regroup_brokerage_episode(
        &self,
        ctx: &Context<'_>,
        episode_id: String,
        transaction_ids: Vec<String>,
    ) -> Result<String> {
        let user_db = get_user_db(ctx).await?;
        Ok(trade_review_table::regroup_episode(
            user_db.pool(),
            user_db.user_id(),
            &episode_id,
            transaction_ids,
        )
        .await?)
    }

    /// Removes a manual grouping and returns its fills to deterministic
    /// brokerage grouping.
    async fn reset_brokerage_episode_grouping(
        &self,
        ctx: &Context<'_>,
        episode_id: String,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(trade_review_table::reset_episode_grouping(
            user_db.pool(),
            user_db.user_id(),
            &episode_id,
        )
        .await?)
    }

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

        let (snaptrade_user_id, user_secret, encrypted_secret) = if let Some(ref uid) =
            account.snaptrade_user_id
        {
            // Already registered — decrypt the existing secret
            let encrypted = account
                .snaptrade_user_secret_encrypted
                .clone()
                .ok_or_else(|| async_graphql::Error::new("No SnapTrade secret stored"))?;
            let secret = decrypt_secret(&encrypted)?;
            (uid.clone(), secret, encrypted)
        } else if let Some(existing) =
            workspaces_table::find_with_snaptrade_credentials(user_db.pool(), user_db.user_id())
                .await?
        {
            let user_id = existing.snaptrade_user_id.ok_or_else(|| {
                async_graphql::Error::new("Existing SnapTrade user ID is missing")
            })?;
            let encrypted = existing
                .snaptrade_user_secret_encrypted
                .ok_or_else(|| async_graphql::Error::new("Existing SnapTrade secret is missing"))?;
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
            (user_id, secret, encrypted)
        } else {
            let reg = crate::service::brokerage::db::register_and_store(
                brokerage_client,
                user_db.pool(),
                &workspace_id,
                user_db.user_id(),
            )
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to register: {e}")))?;

            let encrypted =
                workspaces_table::find_workspace(user_db.pool(), &workspace_id, user_db.user_id())
                    .await?
                    .and_then(|workspace| workspace.snaptrade_user_secret_encrypted)
                    .ok_or_else(|| {
                        async_graphql::Error::new("Registered SnapTrade secret was not stored")
                    })?;
            (reg.user_id, reg.user_secret, encrypted)
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
                if !is_stale_credentials(&e) {
                    return Err(async_graphql::Error::new(format!(
                        "Failed to initiate connection: {e}"
                    )));
                }

                log::warn!(
                    "SnapTrade rejected stored credentials for account={} user={} — \
                     resetting the shared registration before re-registration",
                    workspace_id,
                    user_db.user_id()
                );

                if let Err(delete_error) = brokerage_client.delete_user(&snaptrade_user_id).await {
                    // Code 1083 here means the old user is already absent from
                    // the current SnapTrade tenant. That is the state this
                    // recovery handles, so local cleanup must still continue.
                    if is_stale_credentials(&delete_error) {
                        log::info!(
                            "Stale SnapTrade user {} is already absent; continuing local recovery",
                            snaptrade_user_id
                        );
                    } else {
                        return Err(async_graphql::Error::new(format!(
                            "Failed to reset stale SnapTrade credentials: {delete_error}"
                        )));
                    }
                }

                let cleared = workspaces_table::clear_shared_snaptrade_credentials(
                    user_db.pool(),
                    user_db.user_id(),
                    &encrypted_secret,
                )
                .await?;
                log::info!(
                    "Cleared stale SnapTrade credentials from {cleared} workspace(s) for user={}",
                    user_db.user_id()
                );

                let reg = crate::service::brokerage::db::register_and_store(
                    brokerage_client,
                    user_db.pool(),
                    &workspace_id,
                    user_db.user_id(),
                )
                .await
                .map_err(|register_error| {
                    async_graphql::Error::new(format!(
                        "Failed to re-register the brokerage connection: {register_error}"
                    ))
                })?;

                brokerage_client
                    .initiate_connection(
                        &reg.user_id,
                        &reg.user_secret,
                        brokerage_id.as_deref().unwrap_or(""),
                        None,
                        None,
                        custom_redirect.as_deref(),
                    )
                    .await
                    .map_err(|restart_error| {
                        async_graphql::Error::new(format!(
                            "Failed to restart the brokerage connection: {restart_error}"
                        ))
                    })?
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

    /// Creates separate workspaces for selected, currently unlinked accounts
    /// exposed by the same brokerage authorization as the source workspace.
    async fn create_brokerage_account_workspaces(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        snaptrade_account_ids: Vec<String>,
    ) -> Result<Vec<workspaces_table::Workspace>> {
        let user_db = get_user_db(ctx).await?;
        let brokerage_client = ctx.data::<Arc<BrokerageClient>>()?;
        let workspace =
            workspaces_table::find_workspace(user_db.pool(), &workspace_id, user_db.user_id())
                .await?
                .ok_or_else(|| async_graphql::Error::new("Workspace not found"))?;
        let snaptrade_user_id = workspace
            .snaptrade_user_id
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("Workspace not linked to SnapTrade"))?;
        let encrypted = workspace
            .snaptrade_user_secret_encrypted
            .as_deref()
            .ok_or_else(|| async_graphql::Error::new("No SnapTrade secret stored"))?;
        let requested: HashSet<String> = snaptrade_account_ids
            .into_iter()
            .filter(|id| !id.trim().is_empty())
            .collect();
        if requested.is_empty() {
            return Ok(Vec::new());
        }
        if requested.len() > 25 {
            return Err(async_graphql::Error::new(
                "A maximum of 25 brokerage accounts can be imported at once",
            ));
        }

        let user_secret = decrypt_secret(encrypted)?;
        let accounts = brokerage_client
            .list_snaptrade_accounts(snaptrade_user_id, &user_secret)
            .await?;
        let created =
            crate::service::brokerage::workspaces::create_workspaces_for_connection_accounts(
                user_db.pool(),
                user_db.user_id(),
                &workspace_id,
                &accounts,
                &requested,
            )
            .await?;

        if let Ok(redis) = ctx.data::<Arc<RedisClient>>() {
            for workspace in &created {
                brokerage_cache::invalidate_account_cache(redis, user_db.user_id(), &workspace.id)
                    .await;
            }
        }
        Ok(created)
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

        let sync_key = format!("{}:{workspace_id}", user_db.user_id());
        if !begin_sync(&sync_key).await {
            return Ok(SyncResult {
                status: "queued".to_string(),
                transactions_synced: 0,
                holdings_synced: 0,
                balances_synced: 0,
            });
        }

        let diagnostic_id = Uuid::new_v4().to_string();
        if let Err(error) = workspaces_table::mark_brokerage_sync_started(
            user_db.pool(),
            &workspace_id,
            user_db.user_id(),
            &diagnostic_id,
        )
        .await
        {
            finish_sync(&sync_key).await;
            return Err(async_graphql::Error::new(format!(
                "Failed to track brokerage refresh: {error}"
            )));
        }

        let result: Result<SyncResult> = async {
        let mut snaptrade_account_id = account.snaptrade_account_id.clone();
        let connection_id = account
            .snaptrade_connection_id
            .clone()
            .ok_or_else(|| async_graphql::Error::new("Workspace has no SnapTrade connection"))?;

        let broker_was_missing = account
            .broker
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty);
        let mut broker = account
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

        let connection = match brokerage_client
            .get_connection_status(&snaptrade_user_id, &user_secret, &connection_id)
            .await
        {
            Ok(connection) => connection,
            Err(error) if is_stale_credentials(&error) => {
                workspaces_table::set_connection_disabled(
                    user_db.pool(),
                    &workspace_id,
                    user_db.user_id(),
                    true,
                    None,
                )
                .await?;
                return Err(async_graphql::Error::new(
                    "Your brokerage connection needs to be reauthorized. Please reconnect the account to resume syncing.",
                ));
            }
            Err(error) => {
                return Err(async_graphql::Error::new(format!(
                    "Failed to inspect SnapTrade connection: {error}"
                )));
            }
        };
        if connection.disabled == Some(true) {
            workspaces_table::set_connection_disabled(
                user_db.pool(),
                &workspace_id,
                user_db.user_id(),
                true,
                None,
            )
            .await?;
            return Err(async_graphql::Error::new(
                "Your brokerage connection needs to be reauthorized before it can sync.",
            ));
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
                if is_stale_credentials(&e) {
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
            return Err(async_graphql::Error::new(
                "No brokerage accounts are available yet. Wait a moment, then retry.",
            ));
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

        if broker_was_missing
            && let Some(institution) = st_account
                .institution_name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        {
            workspaces_table::set_broker(
                user_db.pool(),
                &workspace_id,
                user_db.user_id(),
                institution,
            )
            .await?;
            broker = institution.to_string();
        }

        if connection.data_freshness_mode == "delayed" {
            if let Err(error) = brokerage_client
                .refresh_connection(&snaptrade_user_id, &user_secret, &connection_id)
                .await
            {
                return Err(async_graphql::Error::new(format!(
                    "Failed to queue SnapTrade refresh: {error}"
                )));
            }

            spawn_delayed_refresh_follow_up(DelayedRefreshTask {
                key: sync_key.clone(),
                diagnostic_id: diagnostic_id.clone(),
                brokerage: Arc::clone(brokerage_client),
                redis: ctx.data::<Arc<RedisClient>>().ok().cloned(),
                pool: user_db.pool().clone(),
                user_id: user_db.user_id().to_string(),
                workspace_id: workspace_id.clone(),
                snaptrade_user_id: snaptrade_user_id.clone(),
                user_secret,
                snaptrade_account_id: snaptrade_account_id.clone(),
                broker: broker.clone(),
                baseline_holdings_mark: holdings_sync_mark(st_account),
            });
            return Ok(SyncResult {
                status: "queued".to_string(),
                transactions_synced: 0,
                holdings_synced: 0,
                balances_synced: 0,
            });
        }

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
            true,
        )
        .await
        .map_err(|error| {
            async_graphql::Error::new(format!(
                "Failed to sync brokerage transactions: {error}"
            ))
        })?
        .map_or(0, |report| report.mapped_count);

        let portfolio = transaction::sync_holdings(
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
                "Failed to sync brokerage portfolio: {error}"
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
            holdings_synced: portfolio.holdings_synced,
            balances_synced: portfolio.balances_synced,
        })
        }
        .await;

        match result {
            Ok(result) if result.status == "queued" => Ok(result),
            Ok(result) => {
                let recorded = workspaces_table::mark_brokerage_sync_completed(
                    user_db.pool(),
                    &workspace_id,
                    user_db.user_id(),
                    &diagnostic_id,
                    result.transactions_synced,
                    result.holdings_synced,
                    result.balances_synced,
                )
                .await;
                finish_sync(&sync_key).await;
                recorded.map_err(|error| {
                    async_graphql::Error::new(format!(
                        "Failed to record brokerage refresh: {error}"
                    ))
                })?;
                Ok(result)
            }
            Err(error) => {
                let message = safe_sync_error(&error.message);
                let recorded = workspaces_table::mark_brokerage_sync_failed(
                    user_db.pool(),
                    &workspace_id,
                    user_db.user_id(),
                    &diagnostic_id,
                    &message,
                )
                .await;
                finish_sync(&sync_key).await;
                if let Err(record_error) = recorded {
                    log::warn!(
                        "Failed to record brokerage sync failure for workspace={workspace_id}: {record_error}"
                    );
                }
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        brokerage_report_snapshot, delayed_sync_counts, holdings_refresh_completed,
        is_stale_credentials, normalize_brokerage_report_input, safe_sync_error,
    };
    use crate::service::brokerage::client::{
        HoldingsSyncStatus, SnapTradeAccount, SnapTradeError, SnapTradeSyncStatus,
    };
    use crate::service::brokerage::transaction::PortfolioSyncReport;

    fn account(mark: Option<&str>, initial_sync_completed: Option<bool>) -> SnapTradeAccount {
        SnapTradeAccount {
            id: Some("account".to_string()),
            brokerage_authorization: None,
            name: None,
            number: None,
            institution_name: None,
            sync_status: Some(SnapTradeSyncStatus {
                transactions: None,
                holdings: Some(HoldingsSyncStatus {
                    initial_sync_completed,
                    last_successful_sync: mark.map(str::to_string),
                    holdings_unavailable: false,
                }),
            }),
        }
    }

    #[test]
    fn delayed_transaction_failure_fails_the_complete_attempt() {
        let result = delayed_sync_counts(
            Err(anyhow::anyhow!("transaction import failed")),
            Ok(Some(PortfolioSyncReport {
                holdings_synced: 3,
                balances_synced: 1,
                ..Default::default()
            })),
        );
        assert!(result.is_err());
    }

    #[test]
    fn stored_sync_errors_are_single_line_and_bounded() {
        let message = safe_sync_error(&format!("provider\n{}", "x".repeat(600)));
        assert!(!message.contains('\n'));
        assert_eq!(message.chars().count(), 500);
    }

    #[test]
    fn delayed_refresh_requires_an_advanced_holdings_marker() {
        assert!(!holdings_refresh_completed(
            Some("2026-08-11T13:00:00Z"),
            &account(Some("2026-08-11T13:00:00Z"), Some(true)),
        ));
        assert!(!holdings_refresh_completed(
            Some("2026-08-11T13:00:00Z"),
            &account(Some("2026-08-11T12:59:59Z"), Some(true)),
        ));
        assert!(holdings_refresh_completed(
            Some("2026-08-11T13:00:00Z"),
            &account(Some("2026-08-11T13:00:01Z"), Some(true)),
        ));
    }

    #[test]
    fn delayed_refresh_waits_for_initial_sync_and_accepts_a_first_marker() {
        assert!(!holdings_refresh_completed(
            None,
            &account(Some("2026-08-11T13:00:01Z"), Some(false)),
        ));
        assert!(!holdings_refresh_completed(
            None,
            &account(None, Some(true)),
        ));
        assert!(holdings_refresh_completed(
            None,
            &account(Some("2026-08-11T13:00:01Z"), Some(true)),
        ));
    }

    #[test]
    fn stale_credentials_are_detected_through_anyhow() {
        let stale = anyhow::Error::new(SnapTradeError::StaleCredentials);
        assert!(is_stale_credentials(&stale));

        let upstream = anyhow::Error::new(SnapTradeError::Upstream {
            code: "UPSTREAM_UNAVAILABLE".to_string(),
            message: "temporary".to_string(),
            retryable: true,
            status: 503,
            upstream_code: None,
        });
        assert!(!is_stale_credentials(&upstream));
    }

    #[test]
    fn brokerage_report_input_is_bounded_and_other_requires_context() {
        assert!(normalize_brokerage_report_input("balances", None).is_ok());
        assert!(normalize_brokerage_report_input("invalid", None).is_err());
        assert!(normalize_brokerage_report_input("other", None).is_err());
        assert!(normalize_brokerage_report_input("other", Some("Wrong total")).is_ok());
        assert!(normalize_brokerage_report_input("balances", Some(&"x".repeat(1001))).is_err());
    }

    #[test]
    fn brokerage_report_snapshot_has_only_safe_server_fields() {
        let snapshot = brokerage_report_snapshot(Some("Webull"), "account-cash", None, None);
        let encoded = snapshot.to_string();
        assert!(encoded.contains("account-cash"));
        assert!(encoded.contains("Webull"));
        for forbidden in ["secret", "credential", "authorization", "rawPayload"] {
            assert!(!encoded.contains(forbidden));
        }
    }
}
