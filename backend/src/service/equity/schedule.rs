use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Datelike, Timelike, Utc, Weekday};
use chrono_tz::America::New_York;
use log::{error, info};

use crate::service::db::Db;
use crate::service::equity::rebuild;

const TICK_SECS: u64 = 60;
const FIRST_HOUR_ET: u32 = 9;
const LAST_HOUR_ET: u32 = 17;

/// The slot a moment belongs to: `(ET date, ET hour)` inside the weekday 09:00–17:00 ET
/// window, or `None` outside it. Anchored to New York — not the server's local zone, which
/// is UTC in the container — so the window tracks US market hours across DST.
fn market_slot(now: DateTime<Utc>) -> Option<(chrono::NaiveDate, u32)> {
    let et = now.with_timezone(&New_York);
    if matches!(et.weekday(), Weekday::Sat | Weekday::Sun) {
        return None;
    }
    let hour = et.hour();
    if !(FIRST_HOUR_ET..=LAST_HOUR_ET).contains(&hour) {
        return None;
    }
    Some((et.date_naive(), hour))
}

/// Rebuilds every account once per hour during US market hours. Covers the case the
/// transaction-sync hook cannot: an open position whose value moved because prices moved,
/// with no new trades to trigger a rebuild.
pub async fn run_equity_scheduler(db: Arc<Db>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
    info!("[equity] scheduler started (hourly, 09:00-17:00 ET, weekdays)");
    let mut ticker = tokio::time::interval(Duration::from_secs(TICK_SECS));
    let mut last_slot: Option<(chrono::NaiveDate, u32)> = None;

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("[equity] scheduler stopped");
                    return;
                }
                continue;
            }
        }

        let Some(slot) = market_slot(Utc::now()) else {
            continue;
        };
        if last_slot == Some(slot) {
            continue;
        }
        last_slot = Some(slot);

        match rebuild::rebuild_every_account(db.pool()).await {
            Ok(0) => {}
            Ok(n) => info!("[equity] rebuilt {n} account(s) for {}:00 ET", slot.1),
            Err(e) => error!("[equity] scheduled sweep failed: {e:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn weekdays_inside_the_et_window_get_one_slot_per_hour() {
        // 14:00Z = 10:00 EDT on a Monday.
        let slot = market_slot(utc("2026-07-13T14:00:00Z")).expect("inside window");
        assert_eq!(slot.1, 10);
        // Same ET hour, later minute: same slot, so the sweep runs once.
        let same = market_slot(utc("2026-07-13T14:59:00Z")).unwrap();
        assert_eq!(slot, same);
        // Next hour is a new slot.
        let next = market_slot(utc("2026-07-13T15:00:00Z")).unwrap();
        assert_ne!(slot, next);
    }

    #[test]
    fn the_window_is_inclusive_of_9am_and_5pm_et() {
        assert!(market_slot(utc("2026-07-13T13:00:00Z")).is_some()); // 09:00 EDT
        assert!(market_slot(utc("2026-07-13T21:00:00Z")).is_some()); // 17:00 EDT
        assert!(market_slot(utc("2026-07-13T12:59:00Z")).is_none()); // 08:59 EDT
        assert!(market_slot(utc("2026-07-13T22:00:00Z")).is_none()); // 18:00 EDT
    }

    #[test]
    fn weekends_never_run() {
        assert!(market_slot(utc("2026-07-11T14:00:00Z")).is_none()); // Saturday
        assert!(market_slot(utc("2026-07-12T14:00:00Z")).is_none()); // Sunday
    }

    /// The window must follow New York, not UTC: in winter 14:00Z is 09:00 EST (in), but
    /// in summer the same wall clock in UTC is 10:00 EDT. A UTC-fixed window would drift
    /// by an hour twice a year.
    #[test]
    fn the_window_tracks_dst() {
        assert_eq!(market_slot(utc("2026-01-05T14:00:00Z")).unwrap().1, 9); // EST
        assert_eq!(market_slot(utc("2026-07-13T14:00:00Z")).unwrap().1, 10); // EDT
        // 13:00Z is inside in summer (09:00 EDT) but before the window in winter (08:00 EST).
        assert!(market_slot(utc("2026-07-13T13:00:00Z")).is_some());
        assert!(market_slot(utc("2026-01-05T13:00:00Z")).is_none());
    }
}
