//! Periodic `data_bytes_used` recompute.
//!
//! Incremental accounting covers media, but stored data grows and shrinks
//! through too many paths to hook individually. A slow sweep keeps the number
//! honest; the per-sync recompute keeps it fresh for active users.

use std::sync::Arc;
use std::time::Duration;

use log::{error, info};

use super::usage;
use crate::service::db::Db;

/// Storage caps are measured in hundreds of megabytes, so hourly is ample —
/// nobody crosses a cap and back inside one sweep.
const SWEEP_INTERVAL_SECS: u64 = 3600;

pub async fn run_usage_sweeper(db: Arc<Db>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    info!("[billing] usage sweeper started");
    let mut ticker = tokio::time::interval(Duration::from_secs(SWEEP_INTERVAL_SECS));

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("[billing] usage sweeper stopped");
                    return;
                }
                continue;
            }
        }

        let users = match usage::users_with_accounts(db.pool()).await {
            Ok(users) => users,
            Err(e) => {
                error!("[billing] failed to list users for usage sweep: {e:#}");
                continue;
            }
        };

        let mut swept = 0usize;
        for user_id in users {
            // One user's failure must not abort the sweep for everyone else.
            match usage::recompute_user_bytes(db.pool(), &user_id).await {
                Ok(_) => swept += 1,
                Err(e) => error!("[billing] usage recompute failed for {user_id}: {e:#}"),
            }
        }

        if swept > 0 {
            info!("[billing] recomputed usage for {swept} user(s)");
        }
    }
}
