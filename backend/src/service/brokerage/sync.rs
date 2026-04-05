use std::sync::Arc;

use chrono::{Datelike, Timelike, Utc, Weekday};
use chrono_tz::US::Eastern;
use log::{error, info, warn};
use tokio::time::{Duration, sleep};

use super::client::BrokerageClient;
use super::db::decrypt_secret;
use super::transaction;
use crate::service::turso::TursoClient;

/// Timeout for syncing a single account (10 minutes).
const ACCOUNT_SYNC_TIMEOUT: Duration = Duration::from_secs(600);

/// How often the scheduler loop checks the clock (60 seconds).
const TICK_INTERVAL: Duration = Duration::from_secs(60);

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

async fn sync_all_accounts(turso: &TursoClient, brokerage: &BrokerageClient) {
    info!("[sync] Starting scheduled sync of all connected accounts");

    let conn = match turso.get_connection() {
        Ok(c) => c,
        Err(e) => {
            error!("[sync] Failed to get DB connection: {e}");
            return;
        }
    };

    // Find all users who have accounts with snaptrade credentials
    let rows = conn
        .query(
            "SELECT DISTINCT a.user_id, a.id, a.snaptrade_user_id, a.snaptrade_user_secret_encrypted \
             FROM accounts a \
             WHERE a.snaptrade_connection_id IS NOT NULL \
               AND a.snaptrade_user_id IS NOT NULL \
               AND a.snaptrade_user_secret_encrypted IS NOT NULL",
            libsql::params![],
        )
        .await;

    let mut rows = match rows {
        Ok(r) => r,
        Err(e) => {
            error!("[sync] Failed to query connected accounts: {e}");
            return;
        }
    };

    let mut accounts: Vec<(String, String, String, String)> = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let user_id: String = row.get(0).unwrap_or_default();
        let account_id: String = row.get(1).unwrap_or_default();
        let snaptrade_user_id: String = row.get(2).unwrap_or_default();
        let encrypted_secret: String = row.get(3).unwrap_or_default();
        if !user_id.is_empty() && !snaptrade_user_id.is_empty() {
            accounts.push((user_id, account_id, snaptrade_user_id, encrypted_secret));
        }
    }

    if accounts.is_empty() {
        info!("[sync] No connected accounts found, nothing to sync");
        return;
    }

    info!("[sync] Found {} connected accounts to sync", accounts.len());

    for (user_id, account_id, snaptrade_user_id, encrypted_secret) in &accounts {
        info!("[sync] Syncing account {} for user {}", account_id, user_id);

        let user_secret = match decrypt_secret(encrypted_secret) {
            Ok(s) => s,
            Err(e) => {
                error!(
                    "[sync] Failed to decrypt secret for account {}: {e}",
                    account_id
                );
                continue;
            }
        };

        // Get a fresh connection per account
        let conn = match turso.get_connection() {
            Ok(c) => c,
            Err(e) => {
                error!(
                    "[sync] Failed to get DB connection for account {}: {e}",
                    account_id
                );
                continue;
            }
        };

        // Enable foreign keys
        if let Err(e) = conn
            .execute("PRAGMA foreign_keys = ON", libsql::params![])
            .await
        {
            error!("[sync] Failed to enable FK for account {}: {e}", account_id);
            continue;
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
                warn!(
                    "[sync] Failed to list SnapTrade accounts for {}: {e}",
                    account_id
                );
                continue;
            }
            Err(_) => {
                error!(
                    "[sync] Timeout listing SnapTrade accounts for {}",
                    account_id
                );
                continue;
            }
        };

        for st_account in &st_accounts {
            let st_id = match &st_account.id {
                Some(id) => id.clone(),
                None => continue,
            };

            // Sync transactions with timeout
            match tokio::time::timeout(
                ACCOUNT_SYNC_TIMEOUT,
                transaction::sync_transactions(
                    brokerage,
                    &conn,
                    snaptrade_user_id,
                    &user_secret,
                    &st_id,
                    account_id,
                ),
            )
            .await
            {
                Ok(Ok(count)) => info!(
                    "[sync] Synced {} transactions for st_account={}",
                    count, st_id
                ),
                Ok(Err(e)) => warn!(
                    "[sync] Failed to sync transactions for st_account={}: {e}",
                    st_id
                ),
                Err(_) => error!(
                    "[sync] Timeout syncing transactions for st_account={}",
                    st_id
                ),
            }

            // Sync holdings + balances with timeout
            match tokio::time::timeout(
                ACCOUNT_SYNC_TIMEOUT,
                transaction::sync_holdings(
                    brokerage,
                    &conn,
                    snaptrade_user_id,
                    &user_secret,
                    &st_id,
                    account_id,
                ),
            )
            .await
            {
                Ok(Ok((h, b))) => info!(
                    "[sync] Synced {} holdings, {} balances for st_account={}",
                    h, b, st_id
                ),
                Ok(Err(e)) => warn!(
                    "[sync] Failed to sync holdings for st_account={}: {:?}",
                    st_id, e
                ),
                Err(_) => error!("[sync] Timeout syncing holdings for st_account={}", st_id),
            }
        }

        info!("[sync] Finished syncing account {}", account_id);
    }

    info!("[sync] Scheduled sync complete");
}

// ---------------------------------------------------------------------------
// Public entry point — spawned from main.rs
// ---------------------------------------------------------------------------

pub async fn run_sync_scheduler(turso: Arc<TursoClient>, brokerage: Arc<BrokerageClient>) {
    info!("[sync] Brokerage sync scheduler started");

    // Test mode: sync immediately on startup
    if std::env::var("SYNC_TEST_NOW").unwrap_or_default() == "true" {
        info!("[sync] SYNC_TEST_NOW=true — running immediate sync");
        sync_all_accounts(&turso, &brokerage).await;
        info!("[sync] Test sync complete");
    }

    // Track last sync minute to avoid double-syncing in the same minute
    let mut last_sync_minute: Option<(u32, u32, u32)> = None; // (day_of_year, hour, minute)

    loop {
        sleep(TICK_INTERVAL).await;

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
            sync_all_accounts(&turso, &brokerage).await;
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
