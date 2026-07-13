use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use chrono::NaiveDate;
use sqlx::{PgPool, Row};

use crate::service::equity::replay::{EquityPoint, ReplayHealth};

#[derive(Debug, Clone)]
pub struct RebuildHealthRow {
    pub rebuilt_at: String,
    pub replay_version: i32,
    pub reconstructed_equity: Option<f64>,
    pub reported_equity: Option<f64>,
    pub drift: Option<f64>,
    pub health: ReplayHealth,
}

/// Overwrites on conflict. Closes are split-adjusted, so a *new* split silently restates
/// every historical close for that symbol — a cached row is not immutable.
pub async fn upsert_price_history(pool: &PgPool, rows: &[(String, NaiveDate, f64)]) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let symbols: Vec<String> = rows.iter().map(|(s, _, _)| s.clone()).collect();
    let dates: Vec<NaiveDate> = rows.iter().map(|(_, d, _)| *d).collect();
    let closes: Vec<f64> = rows.iter().map(|(_, _, c)| *c).collect();

    sqlx::query(
        "INSERT INTO price_history (symbol, date, close) \
         SELECT * FROM UNNEST($1::text[], $2::date[], $3::double precision[]) \
         ON CONFLICT (symbol, date) DO UPDATE SET close = excluded.close, fetched_at = now()",
    )
    .bind(&symbols)
    .bind(&dates)
    .bind(&closes)
    .execute(pool)
    .await
    .context("Failed to cache price history")?;
    Ok(())
}

pub async fn cached_prices(
    pool: &PgPool,
    symbols: &[String],
    from: NaiveDate,
    to: NaiveDate,
) -> Result<HashMap<(String, NaiveDate), f64>> {
    if symbols.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query(
        "SELECT symbol, date, close FROM price_history \
         WHERE symbol = ANY($1) AND date >= $2 AND date <= $3",
    )
    .bind(symbols)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
    .context("Failed to read cached prices")?;

    let mut out = HashMap::with_capacity(rows.len());
    for row in &rows {
        let symbol: String = row.try_get("symbol")?;
        let date: NaiveDate = row.try_get("date")?;
        let close: f64 = row.try_get("close")?;
        out.insert((symbol, date), close);
    }
    Ok(out)
}

/// Delete-then-insert in one transaction: a rebuild is wholesale, so it must be idempotent.
pub async fn replace_equity_history(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    points: &[EquityPoint],
) -> Result<()> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM account_equity_history WHERE account_id = $1 AND user_id = $2")
        .bind(account_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("Failed to clear equity history")?;

    if !points.is_empty() {
        let dates: Vec<NaiveDate> = points.iter().map(|p| p.date).collect();
        let cash: Vec<f64> = points.iter().map(|p| p.cash).collect();
        let pos: Vec<f64> = points.iter().map(|p| p.positions_value).collect();
        let eq: Vec<f64> = points.iter().map(|p| p.equity).collect();
        let contrib: Vec<f64> = points.iter().map(|p| p.net_contributions).collect();
        let adj: Vec<f64> = points.iter().map(|p| p.funding_adjusted_equity).collect();

        sqlx::query(
            "INSERT INTO account_equity_history \
             (user_id, account_id, date, cash, positions_value, equity, net_contributions, funding_adjusted_equity) \
             SELECT $1, $2, * FROM UNNEST($3::date[], $4::double precision[], $5::double precision[], \
                                          $6::double precision[], $7::double precision[], $8::double precision[])",
        )
        .bind(user_id)
        .bind(account_id)
        .bind(&dates)
        .bind(&cash)
        .bind(&pos)
        .bind(&eq)
        .bind(&contrib)
        .bind(&adj)
        .execute(&mut *tx)
        .await
        .context("Failed to insert equity history")?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn equity_history(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    from: Option<NaiveDate>,
) -> Result<Vec<EquityPoint>> {
    let rows = sqlx::query(
        "SELECT date, cash, positions_value, equity, net_contributions, funding_adjusted_equity \
         FROM account_equity_history \
         WHERE user_id = $1 AND account_id = $2 AND ($3::date IS NULL OR date >= $3::date) \
         ORDER BY date ASC",
    )
    .bind(user_id)
    .bind(account_id)
    .bind(from)
    .fetch_all(pool)
    .await
    .context("Failed to read equity history")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(EquityPoint {
            date: row.try_get("date")?,
            cash: row.try_get("cash")?,
            positions_value: row.try_get("positions_value")?,
            equity: row.try_get("equity")?,
            net_contributions: row.try_get("net_contributions")?,
            funding_adjusted_equity: row.try_get("funding_adjusted_equity")?,
        });
    }
    Ok(out)
}

pub async fn save_rebuild_health(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    reconstructed: Option<f64>,
    reported: Option<f64>,
    health: &ReplayHealth,
    replay_version: i32,
) -> Result<()> {
    let drift = match (reconstructed, reported) {
        (Some(r), Some(p)) => Some(r - p),
        _ => None,
    };
    let health_json = serde_json::to_string(health)?;

    sqlx::query(
        "INSERT INTO account_equity_rebuild \
         (account_id, user_id, rebuilt_at, reconstructed_equity, reported_equity, drift, health_json, replay_version) \
         VALUES ($1, $2, now(), $3, $4, $5, $6, $7) \
         ON CONFLICT (account_id) DO UPDATE SET \
           rebuilt_at = now(), reconstructed_equity = excluded.reconstructed_equity, \
           reported_equity = excluded.reported_equity, drift = excluded.drift, \
           health_json = excluded.health_json, replay_version = excluded.replay_version",
    )
    .bind(account_id)
    .bind(user_id)
    .bind(reconstructed)
    .bind(reported)
    .bind(drift)
    .bind(health_json)
    .bind(replay_version)
    .execute(pool)
    .await
    .context("Failed to save equity rebuild health")?;
    Ok(())
}

pub async fn rebuild_health(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
) -> Result<Option<RebuildHealthRow>> {
    let row = sqlx::query(
        "SELECT to_char(rebuilt_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS rebuilt_at, \
                reconstructed_equity, reported_equity, drift, health_json, replay_version \
         FROM account_equity_rebuild WHERE account_id = $1 AND user_id = $2",
    )
    .bind(account_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to read equity rebuild health")?;

    let Some(row) = row else { return Ok(None) };
    let health_json: String = row.try_get("health_json")?;
    Ok(Some(RebuildHealthRow {
        rebuilt_at: row.try_get("rebuilt_at")?,
        replay_version: row.try_get("replay_version")?,
        reconstructed_equity: row.try_get("reconstructed_equity")?,
        reported_equity: row.try_get("reported_equity")?,
        drift: row.try_get("drift")?,
        health: serde_json::from_str(&health_json).unwrap_or_default(),
    }))
}

/// Symbols whose misses have hit `max_misses` and that were retried within `retry_after_days`.
/// These are treated as "expected to have no prices", so they stop forcing a fetch.
pub async fn suppressed_symbols(
    pool: &PgPool,
    max_misses: i32,
    retry_after_days: i32,
) -> Result<HashSet<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT symbol FROM price_fetch_failures \
         WHERE misses >= $1 AND last_attempt_at > now() - make_interval(days => $2)",
    )
    .bind(max_misses)
    .bind(retry_after_days)
    .fetch_all(pool)
    .await
    .context("Failed to read suppressed price symbols")?;
    Ok(rows.into_iter().map(|(s,)| s).collect())
}

/// A symbol that returned candles is healthy — clear it. One that did not gets another
/// miss, so it eventually stops driving fetches.
pub async fn record_fetch_outcome(
    pool: &PgPool,
    requested: &[String],
    returned: &HashSet<String>,
) -> Result<()> {
    let missing: Vec<String> = requested
        .iter()
        .filter(|s| !returned.contains(*s))
        .cloned()
        .collect();
    let healthy: Vec<String> = returned.iter().cloned().collect();

    if !healthy.is_empty() {
        sqlx::query("DELETE FROM price_fetch_failures WHERE symbol = ANY($1)")
            .bind(&healthy)
            .execute(pool)
            .await
            .context("Failed to clear price fetch failures")?;
    }

    if !missing.is_empty() {
        sqlx::query(
            "INSERT INTO price_fetch_failures (symbol, misses, last_attempt_at) \
             SELECT unnest($1::text[]), 1, now() \
             ON CONFLICT (symbol) DO UPDATE SET \
               misses = price_fetch_failures.misses + 1, last_attempt_at = now()",
        )
        .bind(&missing)
        .execute(pool)
        .await
        .context("Failed to record price fetch failures")?;
    }
    Ok(())
}
