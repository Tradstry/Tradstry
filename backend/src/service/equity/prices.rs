//! Daily closes + split events for the equity replay. Cache-first, but the cache is
//! refreshed rather than treated as immutable: a new split restates a symbol's whole
//! history, and the newest close moves every trading day.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use finance_query::{Interval, Tickers, TimeRange};
use sqlx::PgPool;

use crate::service::db::schema::tables::equity_table;
use crate::service::equity::replay::SplitEvent;

fn ts_to_date(timestamp: i64) -> Option<NaiveDate> {
    DateTime::<Utc>::from_timestamp(timestamp, 0).map(|dt| dt.date_naive())
}

fn split_to_event(timestamp: i64, numerator: f64, denominator: f64) -> Option<SplitEvent> {
    if denominator == 0.0 {
        return None;
    }
    Some(SplitEvent {
        date: ts_to_date(timestamp)?,
        ratio: numerator / denominator,
    })
}

/// Longest run of non-trading days we tolerate before deciding the cache is behind
/// (a long holiday weekend is 3; 4 leaves a little slack).
const STALE_AFTER_DAYS: i64 = 4;
/// Misses before a symbol is presumed dead and stops forcing fetches.
const MAX_MISSES: i32 = 3;
/// ...and how long before we give a presumed-dead symbol another chance.
const RETRY_AFTER_DAYS: i32 = 7;

/// Fetch when a symbol has no cached rows at all — a newly traded ticker would otherwise
/// never get prices — or when the newest close is too old to be the last trading day.
/// A plain row count can't see either case: it passes while a new symbol sits empty.
///
/// `suppressed` holds symbols the feed has repeatedly refused to serve (delisted tickers).
/// Without excluding them, "some symbol has no rows" is true forever and every sweep hits
/// the network for data that will never arrive.
fn needs_fetch(
    cached: &HashMap<(String, NaiveDate), f64>,
    symbols: &[String],
    to: NaiveDate,
    suppressed: &HashSet<String>,
) -> bool {
    let mut newest: Option<NaiveDate> = None;
    for symbol in symbols {
        if suppressed.contains(symbol) {
            continue;
        }
        let mut latest: Option<NaiveDate> = None;
        for (s, date) in cached.keys() {
            if s == symbol && Some(*date) > latest {
                latest = Some(*date);
            }
        }
        match latest {
            None => return true,
            Some(d) => {
                if Some(d) > newest {
                    newest = Some(d);
                }
            }
        }
    }
    // Every symbol suppressed: nothing left worth fetching.
    newest.is_some_and(|d| (to - d).num_days() > STALE_AFTER_DAYS)
}

/// `close` is split-adjusted but not dividend-adjusted, which is what the replay wants:
/// it restates share counts into the same current terms. `adj_close` would additionally
/// fold in dividends and distort a price-only valuation.
pub async fn load_closes(
    pool: &PgPool,
    symbols: &[String],
    from: NaiveDate,
    to: NaiveDate,
) -> Result<(HashMap<(String, NaiveDate), f64>, usize)> {
    if symbols.is_empty() {
        return Ok((HashMap::new(), 0));
    }

    let cached = equity_table::cached_prices(pool, symbols, from, to).await?;
    let suppressed = equity_table::suppressed_symbols(pool, MAX_MISSES, RETRY_AFTER_DAYS).await?;

    if !needs_fetch(&cached, symbols, to, &suppressed) {
        return Ok((cached, 0));
    }

    let start = from
        .and_hms_opt(0, 0, 0)
        .context("invalid from date")?
        .and_utc()
        .timestamp();
    let end = to
        .and_hms_opt(23, 59, 59)
        .context("invalid to date")?
        .and_utc()
        .timestamp();

    let tickers = Tickers::builder(symbols.to_vec())
        .build()
        .await
        .context("failed to build price client")?;

    let batch = match tickers.charts_range(Interval::OneDay, start, end).await {
        Ok(b) => b,
        Err(e) => {
            log::warn!("equity: price fetch failed, using cache only: {e}");
            return Ok((cached, 0));
        }
    };
    for (symbol, err) in &batch.errors {
        log::warn!("equity: no price history for {symbol}: {err}");
    }

    let mut rows: Vec<(String, NaiveDate, f64)> = Vec::new();
    for (symbol, chart) in &batch.charts {
        for candle in &chart.candles {
            if let Some(date) = ts_to_date(candle.timestamp) {
                rows.push((symbol.clone(), date, candle.close));
            }
        }
    }

    let returned: HashSet<String> = batch
        .charts
        .iter()
        .filter(|(_, chart)| !chart.candles.is_empty())
        .map(|(symbol, _)| symbol.clone())
        .collect();
    equity_table::record_fetch_outcome(pool, symbols, &returned).await?;

    equity_table::upsert_price_history(pool, &rows).await?;
    let fetched = rows.len();

    let mut out = cached;
    for (symbol, date, close) in rows {
        out.insert((symbol, date), close);
    }
    Ok((out, fetched))
}

/// Without these, every historical share count before a split is wrong.
pub async fn load_splits(symbols: &[String]) -> Result<HashMap<String, Vec<SplitEvent>>> {
    if symbols.is_empty() {
        return Ok(HashMap::new());
    }

    let tickers = Tickers::builder(symbols.to_vec())
        .build()
        .await
        .context("failed to build split client")?;

    let batch = match tickers.splits(TimeRange::Max).await {
        Ok(b) => b,
        Err(e) => {
            log::warn!("equity: split fetch failed; share counts may be unadjusted: {e}");
            return Ok(HashMap::new());
        }
    };

    let mut out: HashMap<String, Vec<SplitEvent>> = HashMap::new();
    for (symbol, splits) in &batch.splits {
        let mut events: Vec<SplitEvent> = splits
            .iter()
            .filter_map(|s| split_to_event(s.timestamp, s.numerator, s.denominator))
            .collect();
        events.sort_by_key(|e| e.date);
        if !events.is_empty() {
            out.insert(symbol.clone(), events);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(rows: &[(&str, &str)]) -> HashMap<(String, NaiveDate), f64> {
        rows.iter()
            .map(|(s, d)| ((s.to_string(), date(d)), 1.0))
            .collect()
    }

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn none() -> HashSet<String> {
        HashSet::new()
    }

    fn suppress(symbols: &[&str]) -> HashSet<String> {
        symbols.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_symbol_with_no_cached_prices_forces_a_fetch() {
        let c = cache(&[("AAPL", "2026-07-10")]);
        let symbols = ["AAPL".to_string(), "NVDA".to_string()];
        assert!(needs_fetch(&c, &symbols, date("2026-07-10"), &none()));
    }

    #[test]
    fn a_fresh_cache_skips_the_network() {
        let c = cache(&[("AAPL", "2026-07-10")]);
        let s = ["AAPL".to_string()];
        assert!(!needs_fetch(&c, &s, date("2026-07-10"), &none()));
        // Friday close still counts on Monday.
        assert!(!needs_fetch(&c, &s, date("2026-07-13"), &none()));
    }

    #[test]
    fn a_cache_behind_the_last_trading_day_refetches() {
        let c = cache(&[("AAPL", "2026-07-01")]);
        let s = ["AAPL".to_string()];
        assert!(needs_fetch(&c, &s, date("2026-07-13"), &none()));
    }

    /// The delisted-ticker case: without suppression this fetches forever, because the
    /// symbol never gains a cached row no matter how many times we ask.
    #[test]
    fn a_dead_symbol_stops_forcing_a_fetch() {
        let c = cache(&[("AAPL", "2026-07-10")]);
        let symbols = ["AAPL".to_string(), "DEADCO".to_string()];
        assert!(needs_fetch(&c, &symbols, date("2026-07-10"), &none()));
        assert!(!needs_fetch(
            &c,
            &symbols,
            date("2026-07-10"),
            &suppress(&["DEADCO"])
        ));
    }

    #[test]
    fn a_suppressed_symbol_still_lets_a_live_one_refresh() {
        let c = cache(&[("AAPL", "2026-07-01")]);
        let symbols = ["AAPL".to_string(), "DEADCO".to_string()];
        assert!(needs_fetch(
            &c,
            &symbols,
            date("2026-07-13"),
            &suppress(&["DEADCO"])
        ));
    }

    #[test]
    fn everything_suppressed_means_nothing_to_fetch() {
        let c = cache(&[]);
        let symbols = ["DEADCO".to_string()];
        assert!(!needs_fetch(
            &c,
            &symbols,
            date("2026-07-13"),
            &suppress(&["DEADCO"])
        ));
    }

    #[test]
    fn timestamp_maps_to_utc_date() {
        assert_eq!(
            ts_to_date(1767657600),
            Some(NaiveDate::from_ymd_opt(2026, 1, 6).unwrap())
        );
    }

    #[test]
    fn split_ratio_is_numerator_over_denominator() {
        let e = split_to_event(1767657600, 2.0, 1.0).unwrap();
        assert_eq!(e.ratio, 2.0);
        let e = split_to_event(1767657600, 1.0, 10.0).unwrap();
        assert_eq!(e.ratio, 0.1);
    }

    #[test]
    fn a_zero_denominator_is_dropped_not_divided_by() {
        assert!(split_to_event(1767657600, 2.0, 0.0).is_none());
    }
}
