use std::sync::Arc;

use chrono::{Datelike, Timelike, Utc, Weekday};
use chrono_tz::US::Eastern;
use log::{error, info, warn};
use sqlx::Row;
use tokio::time::{Duration, sleep};

use super::client::{BrokerageClient, ConnectionStatus};
use super::db::decrypt_secret;
use super::transaction;
use crate::service::countly::{Countly, clerk_id_for_user};
use crate::service::db::Db;
use crate::service::db::schema::tables::workspaces_table;
use crate::service::redis::brokerage as brokerage_cache;
use crate::service::redis::client::RedisClient;

/// A connection is treated as disabled only when SnapTrade explicitly says so.
/// Missing/null `disabled` → fail-open (proceed with the normal sync).
fn is_disabled(status: &ConnectionStatus) -> bool {
    status.disabled.unwrap_or(false)
}

/// Timeout for syncing a single account (10 minutes).
const ACCOUNT_SYNC_TIMEOUT: Duration = Duration::from_secs(600);

/// How often the scheduler loop checks the clock (60 seconds).
const TICK_INTERVAL: Duration = Duration::from_secs(60);

/// The credentials and upstream account binding needed for one scheduled sync.
/// A named type keeps this safety-critical account mapping explicit instead of
/// relying on the position of values in a long tuple.
struct ScheduledAccount {
    user_id: String,
    workspace_id: String,
    snaptrade_user_id: String,
    encrypted_secret: String,
    connection_id: String,
    broker: String,
    snaptrade_account_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Schedule logic
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum SyncDecision {
    Sync,
    Skip,
}

/// Given Eastern Time hour/minute and weekday, decide whether to sync.
/// Rules:
///   - Weekday 9:00–16:00 ET: sync on the hour and half-hour (:00 and :30)
///   - Weekday 16:30 ET: final sync of the day
///   - Saturday 01:00 ET: weekend sync
///   - All other times: skip
fn should_sync(weekday: Weekday, hour: u32, minute: u32) -> SyncDecision {
    match weekday {
        Weekday::Mon | Weekday::Tue | Weekday::Wed | Weekday::Thu | Weekday::Fri => {
            // Market hours: 9:00 – 16:00, sync at :00 and :30
            if (9..=15).contains(&hour) && (minute == 0 || minute == 30) {
                return SyncDecision::Sync;
            }
            // 16:00 on the dot
            if hour == 16 && minute == 0 {
                return SyncDecision::Sync;
            }
            // Final sync at 16:30
            if hour == 16 && minute == 30 {
                return SyncDecision::Sync;
            }
            SyncDecision::Skip
        }
        Weekday::Sat => {
            if hour == 1 && minute == 0 {
                SyncDecision::Sync
            } else {
                SyncDecision::Skip
            }
        }
        Weekday::Sun => SyncDecision::Skip,
    }
}

// ---------------------------------------------------------------------------
// Sync all connected accounts
// ---------------------------------------------------------------------------

/// The scheduler holds only the internal user id, so every event has to resolve
/// the Clerk id first — anything else splits the person in two in Countly.
async fn capture(
    countly: Option<&Arc<Countly>>,
    db: &Db,
    user_id: &str,
    event: &str,
    props: serde_json::Value,
) {
    let Some(countly) = countly else {
        return;
    };
    let Some(clerk_id) = clerk_id_for_user(db.pool(), user_id).await else {
        return;
    };
    countly.capture(&clerk_id, event, props).await;
}

async fn sync_all_accounts(
    db: &Db,
    brokerage: &BrokerageClient,
    redis: Option<&RedisClient>,
    countly: Option<&Arc<Countly>>,
) {
    info!("[sync] Starting scheduled sync of all connected accounts");

    // Find all users who have accounts with snaptrade credentials
    let rows = sqlx::query(
        "SELECT DISTINCT w.user_id, w.id, bc.snaptrade_user_id, \
             bc.snaptrade_user_secret_encrypted, bc.snaptrade_connection_id, \
             COALESCE(bc.broker, 'your brokerage') AS broker, bc.snaptrade_account_id \
             FROM workspaces w \
             JOIN brokerage_connections bc ON bc.workspace_id = w.id AND bc.user_id = w.user_id \
             WHERE bc.snaptrade_connection_id IS NOT NULL \
               AND bc.snaptrade_user_id IS NOT NULL \
               AND bc.snaptrade_user_secret_encrypted IS NOT NULL",
    )
    .fetch_all(db.pool())
    .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => {
            error!("[sync] Failed to query connected accounts: {e}");
            return;
        }
    };

    let mut accounts = Vec::new();
    for row in &rows {
        let user_id: String = row.try_get(0).unwrap_or_default();
        let workspace_id: String = row.try_get(1).unwrap_or_default();
        let snaptrade_user_id: String = row.try_get(2).unwrap_or_default();
        let encrypted_secret: String = row.try_get(3).unwrap_or_default();
        let connection_id: String = row.try_get(4).unwrap_or_default();
        let broker: String = row.try_get(5).unwrap_or_default();
        let snaptrade_account_id: Option<String> = row.try_get(6).unwrap_or(None);
        if !user_id.is_empty() && !snaptrade_user_id.is_empty() {
            accounts.push(ScheduledAccount {
                user_id,
                workspace_id,
                snaptrade_user_id,
                encrypted_secret,
                connection_id,
                broker,
                snaptrade_account_id,
            });
        }
    }

    if accounts.is_empty() {
        info!("[sync] No connected accounts found, nothing to sync");
        return;
    }

    info!("[sync] Found {} connected accounts to sync", accounts.len());

    for ScheduledAccount {
        user_id,
        workspace_id,
        snaptrade_user_id,
        encrypted_secret,
        connection_id,
        broker,
        snaptrade_account_id: stored_snaptrade_account_id,
    } in &accounts
    {
        let mut freshness_mode = "unknown".to_string();
        info!(
            "[sync] Syncing account {} for user {}",
            workspace_id, user_id
        );

        let user_secret = match decrypt_secret(encrypted_secret) {
            Ok(s) => s,
            Err(e) => {
                error!(
                    "[sync] Failed to decrypt secret for account {}: {e}",
                    workspace_id
                );
                capture(
                    countly,
                    db,
                    user_id,
                    "brokerage_sync_failed",
                    serde_json::json!({
                        "broker": broker,
                        "workspace_id": workspace_id,
                        "reason": "decrypt_failed",
                    }),
                )
                .await;
                continue;
            }
        };

        // Check connection health before pulling. A disabled authorization keeps
        // returning the last good snapshot, so reads look fine but are frozen —
        // flag it and skip the pulls (last-known data stays in the DB). Clearing
        // the flag on a healthy connection makes reconnection auto-recover.
        if !connection_id.is_empty() {
            match tokio::time::timeout(
                ACCOUNT_SYNC_TIMEOUT,
                brokerage.get_connection_status(snaptrade_user_id, &user_secret, connection_id),
            )
            .await
            {
                Ok(Ok(status)) => {
                    freshness_mode = status.data_freshness_mode.clone();
                    if let Err(error) = workspaces_table::set_connection_freshness_mode(
                        db.pool(),
                        workspace_id,
                        user_id,
                        &freshness_mode,
                    )
                    .await
                    {
                        warn!(
                            "[sync] Failed to persist freshness mode for {workspace_id}: {error}"
                        );
                    }
                    if is_disabled(&status) {
                        warn!(
                            "[sync] Connection disabled for account {} (disabled_date={:?}); \
                             skipping pulls, keeping last-known data",
                            workspace_id, status.disabled_date
                        );
                        match workspaces_table::set_connection_disabled(
                            db.pool(),
                            workspace_id,
                            user_id,
                            true,
                            status.disabled_date.as_deref(),
                        )
                        .await
                        {
                            Ok(changed) => {
                                // Only on the transition into disabled: re-recording every
                                // half hour would produce a new ungrouped notification per
                                // tick for as long as the connection stays broken.
                                if changed {
                                    let event = crate::service::notifications::NotificationEvent::
                                        BrokerageConnectionDisabled {
                                            workspace_id: workspace_id.clone(),
                                            broker: broker.clone(),
                                        };
                                    let today = Utc::now().with_timezone(&Eastern).date_naive();
                                    if let Err(e) = crate::service::notifications::outbox::record(
                                        db.pool(),
                                        user_id,
                                        &event,
                                        today,
                                    )
                                    .await
                                    {
                                        warn!(
                                            "[sync] failed to record disabled-connection event: {e}"
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                error!(
                                    "[sync] Failed to set disabled flag for {}: {e}",
                                    workspace_id
                                );
                            }
                        }
                        if let Some(redis) = redis {
                            brokerage_cache::invalidate_account_cache(redis, user_id, workspace_id)
                                .await;
                        }
                        continue;
                    }
                    // Healthy — clear any prior disabled flag.
                    if let Err(e) = workspaces_table::set_connection_disabled(
                        db.pool(),
                        workspace_id,
                        user_id,
                        false,
                        None,
                    )
                    .await
                    {
                        error!(
                            "[sync] Failed to clear disabled flag for {}: {e}",
                            workspace_id
                        );
                    }
                }
                Ok(Err(e)) => {
                    // Fail-open: a status hiccup must not block data sync.
                    warn!(
                        "[sync] Connection status check failed for {} ({e}); proceeding with sync",
                        workspace_id
                    );
                }
                Err(_) => {
                    warn!(
                        "[sync] Connection status check timed out for {}; proceeding with sync",
                        workspace_id
                    );
                }
            }
        }

        // Discover SnapTrade accounts
        let st_accounts = match tokio::time::timeout(
            ACCOUNT_SYNC_TIMEOUT,
            brokerage.list_snaptrade_accounts(snaptrade_user_id, &user_secret),
        )
        .await
        {
            Ok(Ok(accs)) => accs,
            Ok(Err(e)) => {
                // Stale credentials never resolve on their own, so retrying every
                // half hour just burns requests. Flag the connection instead; the
                // user-driven reconnect path re-registers.
                if e.downcast_ref::<crate::service::brokerage::client::SnapTradeError>()
                    .is_some_and(|err| {
                        matches!(
                            err,
                            crate::service::brokerage::client::SnapTradeError::StaleCredentials
                        )
                    })
                {
                    warn!(
                        "[sync] SnapTrade rejected stored credentials for {} — flagging \
                         connection as disabled; user must reconnect",
                        workspace_id
                    );
                    if let Err(e) = workspaces_table::set_connection_disabled(
                        db.pool(),
                        workspace_id,
                        user_id,
                        true,
                        None,
                    )
                    .await
                    {
                        error!("[sync] Failed to flag disabled connection for {workspace_id}: {e}");
                    }
                    capture(
                        countly,
                        db,
                        user_id,
                        "brokerage_sync_failed",
                        serde_json::json!({
                            "broker": broker,
                            "workspace_id": workspace_id,
                            "reason": "stale_credentials",
                        }),
                    )
                    .await;
                    continue;
                }

                warn!(
                    "[sync] Failed to list SnapTrade accounts for {}: {e}",
                    workspace_id
                );
                capture(
                    countly,
                    db,
                    user_id,
                    "brokerage_sync_failed",
                    serde_json::json!({
                        "broker": broker,
                        "workspace_id": workspace_id,
                        "reason": "list_accounts_failed",
                    }),
                )
                .await;
                continue;
            }
            Err(_) => {
                error!(
                    "[sync] Timeout listing SnapTrade accounts for {}",
                    workspace_id
                );
                capture(
                    countly,
                    db,
                    user_id,
                    "brokerage_sync_failed",
                    serde_json::json!({
                        "broker": broker,
                        "workspace_id": workspace_id,
                        "reason": "list_accounts_timeout",
                    }),
                )
                .await;
                continue;
            }
        };

        let snaptrade_account_id = match stored_snaptrade_account_id {
            Some(id) => id.clone(),
            None => {
                if let Err(error) =
                    crate::service::brokerage::workspaces::bind_workspace_brokerage_account(
                        db.pool(),
                        user_id,
                        workspace_id,
                        &st_accounts,
                    )
                    .await
                {
                    error!(
                        "[sync] Failed to materialize SnapTrade accounts for {workspace_id}: {error}"
                    );
                    continue;
                }
                match workspaces_table::find_workspace(db.pool(), workspace_id, user_id).await {
                    Ok(Some(account)) => match account.snaptrade_account_id {
                        Some(id) => id,
                        None => {
                            warn!("[sync] No SnapTrade account is available for {workspace_id}");
                            continue;
                        }
                    },
                    Ok(None) => continue,
                    Err(error) => {
                        error!(
                            "[sync] Failed to reload {workspace_id} after materializing accounts: {error}"
                        );
                        continue;
                    }
                }
            }
        };
        let st_account = match st_accounts
            .iter()
            .find(|candidate| candidate.id.as_deref() == Some(snaptrade_account_id.as_str()))
        {
            Some(account) => account,
            None => {
                warn!(
                    "[sync] Stored SnapTrade account {} is missing for {}; reconnect to refresh it",
                    snaptrade_account_id, workspace_id
                );
                continue;
            }
        };

        let (txn_res, hold_res) = tokio::join!(
            tokio::time::timeout(
                ACCOUNT_SYNC_TIMEOUT,
                transaction::sync_transactions_if_advanced(
                    brokerage,
                    db.pool(),
                    snaptrade_user_id,
                    &user_secret,
                    &snaptrade_account_id,
                    user_id,
                    workspace_id,
                    broker,
                    st_account
                        .sync_status
                        .as_ref()
                        .and_then(|s| s.transactions.as_ref()),
                    false,
                ),
            ),
            tokio::time::timeout(
                ACCOUNT_SYNC_TIMEOUT,
                transaction::sync_holdings_if_advanced(
                    brokerage,
                    db.pool(),
                    snaptrade_user_id,
                    &user_secret,
                    &snaptrade_account_id,
                    user_id,
                    workspace_id,
                    st_account
                        .sync_status
                        .as_ref()
                        .and_then(|status| status.holdings.as_ref()),
                    &freshness_mode,
                    false,
                ),
            ),
        );

        match txn_res {
            Ok(Ok(Some(count))) => info!(
                "[sync] Synced {} transactions for st_account={}",
                count, snaptrade_account_id
            ),
            Ok(Ok(None)) => info!(
                "[sync] No new transactions upstream for st_account={}; fetch skipped",
                snaptrade_account_id
            ),
            Ok(Err(e)) => warn!(
                "[sync] Failed to sync transactions for st_account={}: {e}",
                snaptrade_account_id
            ),
            Err(_) => error!(
                "[sync] Timeout syncing transactions for st_account={}",
                snaptrade_account_id
            ),
        }

        match hold_res {
            Ok(Ok(Some((h, b)))) => info!(
                "[sync] Synced {} holdings, {} balances for st_account={}",
                h, b, snaptrade_account_id
            ),
            Ok(Ok(None)) => info!(
                "[sync] Holdings have not advanced for st_account={}; fetch skipped",
                snaptrade_account_id
            ),
            Ok(Err(e)) => warn!(
                "[sync] Failed to sync holdings for st_account={}: {:?}",
                snaptrade_account_id, e
            ),
            Err(_) => error!(
                "[sync] Timeout syncing holdings for st_account={}",
                snaptrade_account_id
            ),
        }

        info!("[sync] Finished syncing account {}", workspace_id);

        capture(
            countly,
            db,
            user_id,
            "brokerage_sync_completed",
            serde_json::json!({ "broker": broker, "workspace_id": workspace_id }),
        )
        .await;

        // Invalidate cache for this account
        if let Some(redis) = redis {
            brokerage_cache::invalidate_account_cache(redis, user_id, workspace_id).await;
        }
    }

    info!("[sync] Scheduled sync complete");
}

// ---------------------------------------------------------------------------
// Public entry point — spawned from main.rs
// ---------------------------------------------------------------------------

pub async fn run_sync_scheduler(
    db: Arc<Db>,
    brokerage: Arc<BrokerageClient>,
    redis: Option<Arc<RedisClient>>,
    countly: Option<Arc<Countly>>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    info!("[sync] Brokerage sync scheduler started");

    // Test mode: sync immediately on startup
    if std::env::var("SYNC_TEST_NOW").unwrap_or_default() == "true" {
        info!("[sync] SYNC_TEST_NOW=true — running immediate sync");
        sync_all_accounts(
            &db,
            &brokerage,
            redis.as_ref().map(|r| r.as_ref()),
            countly.as_ref(),
        )
        .await;
        info!("[sync] Test sync complete");
    }

    // Track last sync minute to avoid double-syncing in the same minute
    let mut last_sync_minute: Option<(u32, u32, u32)> = None; // (day_of_year, hour, minute)

    loop {
        // Tick, but wake immediately on shutdown so we don't sit out a full interval.
        tokio::select! {
            _ = sleep(TICK_INTERVAL) => {}
            _ = shutdown.changed() => {}
        }
        // Stop between ticks (never mid-sync) so shutdown can't tear a replica write.
        if *shutdown.borrow() {
            info!("[sync] Shutdown requested; exiting sync scheduler");
            return;
        }

        let now_et = Utc::now().with_timezone(&Eastern);
        let weekday = now_et.weekday();
        let hour = now_et.hour();
        let minute = now_et.minute();
        let day_of_year = now_et.ordinal();

        let key = (day_of_year, hour, minute);
        if last_sync_minute == Some(key) {
            continue; // Already synced this minute
        }

        if should_sync(weekday, hour, minute) == SyncDecision::Sync {
            info!(
                "[sync] Schedule triggered: {:?} {:02}:{:02} ET",
                weekday, hour, minute
            );
            last_sync_minute = Some(key);
            sync_all_accounts(
                &db,
                &brokerage,
                redis.as_ref().map(|r| r.as_ref()),
                countly.as_ref(),
            )
            .await;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_disabled_reads_flag() {
        use crate::service::brokerage::client::ConnectionStatus;
        let mk = |d: Option<bool>| ConnectionStatus {
            id: None,
            name: None,
            connection_type: None,
            disabled: d,
            disabled_date: None,
            data_freshness_mode: "realtime".to_string(),
        };
        assert!(is_disabled(&mk(Some(true))));
        assert!(!is_disabled(&mk(Some(false))));
        assert!(!is_disabled(&mk(None))); // absent → treat as enabled (fail-open)
    }

    #[test]
    fn weekday_market_hours_sync() {
        // 9:00 Monday
        assert_eq!(should_sync(Weekday::Mon, 9, 0), SyncDecision::Sync);
        // 9:30 Tuesday
        assert_eq!(should_sync(Weekday::Tue, 9, 30), SyncDecision::Sync);
        // 12:00 Wednesday
        assert_eq!(should_sync(Weekday::Wed, 12, 0), SyncDecision::Sync);
        // 15:30 Thursday
        assert_eq!(should_sync(Weekday::Thu, 15, 30), SyncDecision::Sync);
        // 16:00 Friday
        assert_eq!(should_sync(Weekday::Fri, 16, 0), SyncDecision::Sync);
    }

    #[test]
    fn weekday_final_sync() {
        // 16:30 Monday — final sync
        assert_eq!(should_sync(Weekday::Mon, 16, 30), SyncDecision::Sync);
    }

    #[test]
    fn weekday_off_hours_skip() {
        // 8:59 Monday — before market
        assert_eq!(should_sync(Weekday::Mon, 8, 59), SyncDecision::Skip);
        // 9:15 — not on :00 or :30
        assert_eq!(should_sync(Weekday::Mon, 9, 15), SyncDecision::Skip);
        // 17:00 — after final sync
        assert_eq!(should_sync(Weekday::Mon, 17, 0), SyncDecision::Skip);
        // 20:00
        assert_eq!(should_sync(Weekday::Fri, 20, 0), SyncDecision::Skip);
    }

    #[test]
    fn saturday_morning_sync() {
        assert_eq!(should_sync(Weekday::Sat, 1, 0), SyncDecision::Sync);
        // Other Saturday times skip
        assert_eq!(should_sync(Weekday::Sat, 2, 0), SyncDecision::Skip);
        assert_eq!(should_sync(Weekday::Sat, 1, 30), SyncDecision::Skip);
    }

    #[test]
    fn sunday_always_skip() {
        assert_eq!(should_sync(Weekday::Sun, 1, 0), SyncDecision::Skip);
        assert_eq!(should_sync(Weekday::Sun, 12, 0), SyncDecision::Skip);
    }
}
