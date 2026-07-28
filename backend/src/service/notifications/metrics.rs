use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Thresholds live together so the whole accuracy policy is reviewable in one
/// place. They gate estimates, never counts — a count is true at n=1.
pub const DISPOSITION_MIN_TRADES: i64 = 20;
pub const DISPOSITION_MIN_PER_SIDE: i64 = 5;
pub const DISPOSITION_WINDOW_DAYS: i64 = 90;
pub const SETUP_SAMPLE_TARGET: i64 = 100;
pub const SETUP_PROGRESS_LIMIT: i64 = 3;

/// Ratios inside this band are reported as "about the same". A 1.05 is noise,
/// and dressing it up as a direction invents a pattern the trader doesn't have.
pub const NEUTRAL_BAND_LOW: f64 = 0.9;
pub const NEUTRAL_BAND_HIGH: f64 = 1.1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WeeklyCounts {
    pub trades: i64,
    pub journaled: i64,
    pub violations: i64,
    pub top_principle: Option<String>,
    pub top_principle_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Asymmetry {
    /// Median loser duration over median winner duration.
    pub ratio: f64,
    pub wins: i64,
    pub losses: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetupProgress {
    pub name: String,
    pub closed: i64,
    pub target: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WeeklyStats {
    pub counts: Option<WeeklyCounts>,
    pub asymmetry: Option<Asymmetry>,
    pub setups: Vec<SetupProgress>,
}

impl WeeklyStats {
    /// Nothing worth sending. Counts with no trades are not content — the week
    /// simply did not happen.
    pub fn is_empty(&self) -> bool {
        let no_counts = self.counts.as_ref().is_none_or(|c| c.trades == 0);
        no_counts && self.asymmetry.is_none() && self.setups.is_empty()
    }
}

/// Symbols traded on the local day whose fills are not all linked to a journal
/// entry. Counted per symbol rather than per fill so the copy cannot overstate
/// the work — one round trip is two fills but one thing to write up.
pub async fn symbols_to_journal(
    pool: &PgPool,
    user_id: &str,
    day_start: DateTime<Utc>,
    day_end: DateTime<Utc>,
) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT t.symbol) \
         FROM brokerage_transactions t \
         LEFT JOIN journal_brokerage_links l ON l.brokerage_transaction_id = t.id \
         WHERE t.user_id = $1 \
           AND t.transaction_type IN ('BUY', 'SELL') \
           AND t.symbol IS NOT NULL \
           AND t.trade_date >= $2 AND t.trade_date < $3 \
           AND l.id IS NULL",
    )
    .bind(user_id)
    .bind(day_start)
    .bind(day_end)
    .fetch_one(pool)
    .await
    .context("failed to count unjournaled symbols")?;
    Ok(row.0)
}

pub async fn weekly_counts(
    pool: &PgPool,
    user_id: &str,
    week_start: DateTime<Utc>,
    week_end: DateTime<Utc>,
) -> Result<WeeklyCounts> {
    let trades: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM journal_entries \
         WHERE user_id = $1 AND close_date >= $2 AND close_date < $3",
    )
    .bind(user_id)
    .bind(week_start)
    .bind(week_end)
    .fetch_one(pool)
    .await
    .context("failed to count weekly trades")?;

    let journaled: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT j.id) FROM journal_entries j \
         JOIN journal_brokerage_links l ON l.journal_entry_id = j.id \
         WHERE j.user_id = $1 AND j.close_date >= $2 AND j.close_date < $3",
    )
    .bind(user_id)
    .bind(week_start)
    .bind(week_end)
    .fetch_one(pool)
    .await
    .context("failed to count journaled trades")?;

    let violations: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM trade_principle_violations v \
         JOIN journal_entries j ON j.id = v.journal_entry_id \
         WHERE j.user_id = $1 AND j.close_date >= $2 AND j.close_date < $3",
    )
    .bind(user_id)
    .bind(week_start)
    .bind(week_end)
    .fetch_one(pool)
    .await
    .context("failed to count violations")?;

    let top: Option<(String, i64)> = sqlx::query_as(
        "SELECT p.title, COUNT(*) AS n FROM trade_principle_violations v \
         JOIN journal_entries j ON j.id = v.journal_entry_id \
         JOIN trading_principles p ON p.id = v.principle_id \
         WHERE j.user_id = $1 AND j.close_date >= $2 AND j.close_date < $3 \
         GROUP BY p.title ORDER BY n DESC, p.title LIMIT 1",
    )
    .bind(user_id)
    .bind(week_start)
    .bind(week_end)
    .fetch_optional(pool)
    .await
    .context("failed to read top violated principle")?;

    let (top_principle, top_principle_count) = match top {
        Some((title, n)) => (Some(title), n),
        None => (None, 0),
    };

    Ok(WeeklyCounts {
        trades: trades.0,
        journaled: journaled.0,
        violations: violations.0,
        top_principle,
        top_principle_count,
    })
}

/// Holding-period asymmetry — the disposition effect. Median, not mean: one
/// overnight hold otherwise dominates the ratio.
///
/// Measured over a trailing window rather than the delivery week, because a
/// single week yields too few closed trades for the ratio to be stable.
pub async fn holding_asymmetry(
    pool: &PgPool,
    user_id: &str,
    since: DateTime<Utc>,
) -> Result<Option<Asymmetry>> {
    let row: Option<(Option<f64>, Option<f64>, i64, i64)> = sqlx::query_as(
        "SELECT \
           PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY duration) \
             FILTER (WHERE status = 'profit') AS win_median, \
           PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY duration) \
             FILTER (WHERE status = 'loss') AS loss_median, \
           COUNT(*) FILTER (WHERE status = 'profit') AS wins, \
           COUNT(*) FILTER (WHERE status = 'loss') AS losses \
         FROM journal_entries \
         WHERE user_id = $1 AND close_date >= $2",
    )
    .bind(user_id)
    .bind(since)
    .fetch_optional(pool)
    .await
    .context("failed to compute holding asymmetry")?;

    let Some((win_median, loss_median, wins, losses)) = row else {
        return Ok(None);
    };

    if wins + losses < DISPOSITION_MIN_TRADES
        || wins < DISPOSITION_MIN_PER_SIDE
        || losses < DISPOSITION_MIN_PER_SIDE
    {
        return Ok(None);
    }

    let (Some(win_median), Some(loss_median)) = (win_median, loss_median) else {
        return Ok(None);
    };
    if win_median <= 0.0 {
        return Ok(None);
    }

    Ok(Some(Asymmetry {
        ratio: loss_median / win_median,
        wins,
        losses,
    }))
}

/// Playbooks that have not yet earned a performance claim, with how far off they
/// are. The gap is the content — it is what makes the review say something true
/// that improves every week.
pub async fn setup_progress(pool: &PgPool, user_id: &str) -> Result<Vec<SetupProgress>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT p.name, COUNT(j.id) AS n \
         FROM playbooks p \
         JOIN journal_entries j ON j.playbook_id = p.id AND j.user_id = $1 \
         WHERE p.user_id = $1 \
         GROUP BY p.id, p.name \
         HAVING COUNT(j.id) < $2 \
         ORDER BY n DESC, p.name \
         LIMIT $3",
    )
    .bind(user_id)
    .bind(SETUP_SAMPLE_TARGET)
    .bind(SETUP_PROGRESS_LIMIT)
    .fetch_all(pool)
    .await
    .context("failed to read setup progress")?;

    Ok(rows
        .into_iter()
        .map(|(name, closed)| SetupProgress {
            name,
            closed,
            target: SETUP_SAMPLE_TARGET,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_stats_are_empty() {
        assert!(WeeklyStats::default().is_empty());
    }

    #[test]
    fn zero_trade_counts_are_still_empty() {
        let stats = WeeklyStats {
            counts: Some(WeeklyCounts {
                trades: 0,
                journaled: 0,
                violations: 0,
                top_principle: None,
                top_principle_count: 0,
            }),
            ..Default::default()
        };
        assert!(stats.is_empty());
    }

    #[test]
    fn a_setup_block_alone_is_worth_sending() {
        let stats = WeeklyStats {
            setups: vec![SetupProgress {
                name: "Breakout".into(),
                closed: 23,
                target: 100,
            }],
            ..Default::default()
        };
        assert!(!stats.is_empty());
    }
}
