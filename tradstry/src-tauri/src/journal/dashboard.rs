// Dashboard/analytics commands for the Journal app. Computed fully on-device
// from the local trade cache (`db::journal::list_journal_entries`) + the
// pull-only tag caches (`tags_cache`/`tag_categories_cache`) — no network
// call. The compute formulas are ported VERBATIM from the backend in
// `journal::analytics::compute`; see that module and
// docs/superpowers/plans/2026-07-11-analytics-offline-first.md.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;
use tauri::State;

use crate::AppState;
use crate::db::journal::{self as store, JournalRow};
use crate::journal::analytics::compute::{
    self, AdvancedAnalytics, CalendarAnalytics, JournalAnalytics, TagInfo,
};
use crate::journal::analytics::timefilter::{self, AnalyticsTimeFilterInput};

/// Parse a date string to UTC. RFC3339 first (preserves offset), then a
/// date-only `%Y-%m-%d` (midnight UTC). Used to compare `close_date` against
/// the resolved range bounds without naive string comparison — local
/// `close_date` values and the ISO bounds may carry different (but
/// equivalent) UTC representations.
fn parse_utc(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    let end = 10.min(s.len());
    NaiveDate::parse_from_str(&s[..end], "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|ndt| DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
}

/// Parse the `{ range, startDate?, endDate? }` JSON value the frontend sends
/// and resolve it into `(start_iso, end_iso, range_start_label, range_end_label)`.
fn resolve_time_filter(
    time_filter: Value,
) -> Result<(String, String, Option<String>, Option<String>), String> {
    let filter: AnalyticsTimeFilterInput =
        serde_json::from_value(time_filter).map_err(|e| e.to_string())?;
    timefilter::resolve_bounds(
        filter.range,
        filter.start_date.as_deref(),
        filter.end_date.as_deref(),
        Utc::now(),
    )
}

/// Filter `rows` to those whose `close_date` falls within `[start_iso, end_iso]`
/// (inclusive), parsing both sides to `DateTime<Utc>` for a correct compare.
/// Rows with an unparseable `close_date` are excluded.
fn filter_by_range(rows: Vec<JournalRow>, start_iso: &str, end_iso: &str) -> Vec<JournalRow> {
    let Some(start) = parse_utc(start_iso) else {
        return Vec::new();
    };
    let Some(end) = parse_utc(end_iso) else {
        return Vec::new();
    };
    rows.into_iter()
        .filter(|r| {
            parse_utc(&r.close_date)
                .map(|d| d >= start && d <= end)
                .unwrap_or(false)
        })
        .collect()
}

/// Resolve each row's `tag_ids` into hydrated `TagInfo`s via the pull-only
/// `tags_cache`/`tag_categories_cache` local mirrors, keyed by trade id. Trades
/// with no resolvable tags are simply absent from the map (matches the
/// server's `tags_for_trades` semantics for untagged/legacy trades).
fn load_tags_by_trade(
    conn: &Connection,
    rows: &[JournalRow],
) -> Result<HashMap<String, Vec<TagInfo>>, String> {
    let mut tag_lookup: HashMap<String, (String, Option<String>)> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, name, category_id FROM tags_cache")
            .map_err(|e| e.to_string())?;
        let mapped = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in mapped {
            let (id, name, category_id) = row.map_err(|e| e.to_string())?;
            tag_lookup.insert(id, (name, category_id));
        }
    }

    let mut cat_lookup: HashMap<String, (String, Option<String>)> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, name, role FROM tag_categories_cache")
            .map_err(|e| e.to_string())?;
        let mapped = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in mapped {
            let (id, name, role) = row.map_err(|e| e.to_string())?;
            cat_lookup.insert(id, (name, role));
        }
    }

    let mut out: HashMap<String, Vec<TagInfo>> = HashMap::new();
    for row in rows {
        if row.tag_ids.is_empty() {
            continue;
        }
        let mut infos = Vec::new();
        for tag_id in &row.tag_ids {
            let Some((tag_name, category_id)) = tag_lookup.get(tag_id) else {
                continue;
            };
            let Some(category_id) = category_id else {
                continue;
            };
            let Some((category_name, role)) = cat_lookup.get(category_id) else {
                continue;
            };
            infos.push(TagInfo {
                name: tag_name.clone(),
                category_id: category_id.clone(),
                category_name: category_name.clone(),
                role: role.clone(),
            });
        }
        if !infos.is_empty() {
            out.insert(row.id.clone(), infos);
        }
    }
    Ok(out)
}

// --- Journal analytics (upper metric cards) -------------------------------

/// `timeFilter`: `{ range, startDate?, endDate? }` (see `AnalyticsTimeFilter` in
/// `backend.ts`). Computed on-device from the local trade cache.
#[tauri::command]
pub fn journal_analytics(
    state: State<'_, AppState>,
    account_id: String,
    time_filter: Value,
) -> Result<JournalAnalytics, String> {
    let (start_iso, end_iso, range_start, range_end) = resolve_time_filter(time_filter)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let rows = store::list_journal_entries(&conn, &account_id)?;
    let filtered = filter_by_range(rows, &start_iso, &end_iso);
    Ok(compute::compute_journal_analytics(&filtered, range_start, range_end))
}

// --- Calendar analytics (trading calendar) --------------------------------

#[tauri::command]
pub fn calendar_analytics(
    state: State<'_, AppState>,
    account_id: String,
    year: i32,
    month: u32,
) -> Result<CalendarAnalytics, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let rows = store::list_journal_entries(&conn, &account_id)?;
    Ok(compute::compute_calendar(&rows, year, month))
}

// --- Advanced analytics (the Analytics page) ------------------------------

/// `timeFilter`: `{ range, startDate?, endDate? }`. `startingEquity`/
/// `accountEquity` come from the account's cached `total_value`
/// (`accounts_cache`, refreshed by `sync::refresh_accounts_cache`) — `null`
/// for a manual account with no synced balance, in which case
/// `maxDrawdownPct` falls back to the peak-cumulative-PnL denominator,
/// matching the server's behavior.
#[tauri::command]
pub fn advanced_analytics(
    state: State<'_, AppState>,
    account_id: String,
    time_filter: Value,
) -> Result<AdvancedAnalytics, String> {
    let (start_iso, end_iso, range_start, range_end) = resolve_time_filter(time_filter)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let rows = store::list_journal_entries(&conn, &account_id)?;
    let filtered = filter_by_range(rows, &start_iso, &end_iso);
    let tags_by_trade = load_tags_by_trade(&conn, &filtered)?;
    let current_equity: Option<f64> = conn
        .query_row(
            "SELECT total_value FROM accounts_cache WHERE id = ?1",
            rusqlite::params![account_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();
    Ok(compute::compute_advanced(
        &filtered,
        current_equity,
        &tags_by_trade,
        range_start,
        range_end,
    ))
}
