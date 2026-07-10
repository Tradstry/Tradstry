use std::sync::Arc;
use std::time::Duration;

use log::{error, info};

use crate::service::db::Db;
use crate::service::db::schema::tables::notebook::crdt;

/// Both sweeps are idempotent and cheap when there is nothing to do, so the
/// interval only bounds how long a note stays stuck or oversized.
const SWEEP_INTERVAL_SECS: u64 = 60;

/// Heals notes stranded mid-seed and compacts overgrown update chains.
///
/// Neither job belongs on the write path: seeding and compaction each spawn the
/// projector subprocess (~130ms), and a user's keystroke must never wait on one.
pub async fn run_notebook_maintenance(
    db: Arc<Db>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    info!("[notebook] maintenance sweeper started");
    let mut ticker = tokio::time::interval(Duration::from_secs(SWEEP_INTERVAL_SECS));

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("[notebook] maintenance sweeper stopped");
                    return;
                }
                continue;
            }
        }

        match crdt::sweep_stale_seeding(db.pool()).await {
            Ok(0) => {}
            Ok(n) => info!("[notebook] re-seeded {n} note(s) stranded mid-seed"),
            Err(error) => error!("[notebook] stale-seeding sweep failed: {error:#}"),
        }

        match crdt::sweep_compaction(db.pool()).await {
            Ok(0) => {}
            Ok(n) => info!("[notebook] compacted {n} note(s)"),
            Err(error) => error!("[notebook] compaction sweep failed: {error:#}"),
        }

        // After compaction: it appends a merged blob at a higher seq, which leaves the
        // projection stale. Projecting last converges both in one tick.
        match crdt::sweep_projections(db.pool()).await {
            Ok(0) => {}
            Ok(n) => info!("[notebook] re-projected {n} note(s)"),
            Err(error) => error!("[notebook] projection sweep failed: {error:#}"),
        }
    }
}
