use crate::service::turso::client::UserDb;
use crate::service::turso::schema::tables::accounts_table;
use crate::service::turso::schema::tables::journal_table::{
    self, CalendarDayAggregateRow, ExtremeKind, JournalAggregateRow, TradeOutcomeRow,
};
use crate::service::turso::schema::tables::tags_table;

use anyhow::{Result, anyhow, ensure};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Utc};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TradeOutcome {
    pub symbol: String,
    pub symbol_name: String,
    pub amount: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JournalAnalytics {
    pub win_rate: f64,
    pub cumulative_profit: f64,
    pub average_risk_to_reward: f64,
    pub average_gain: f64,
    pub average_loss: f64,
    pub profit_factor: Option<f64>,
    pub biggest_win: Option<TradeOutcome>,
    pub biggest_loss: Option<TradeOutcome>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CalendarDaySummary {
    pub date: String,
    pub profit: f64,
    pub trade_count: usize,
    pub win_rate: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CalendarWeekSummary {
    pub week_index: usize,
    pub week_start: String,
    pub week_end: String,
    pub profit: f64,
    pub trade_count: usize,
    pub trading_days: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CalendarAnalytics {
    pub year: i32,
    pub month: u32,
    pub month_profit: f64,
    pub trade_count: usize,
    pub trading_days: usize,
    pub grid_start: String,
    pub grid_end: String,
    pub days: Vec<CalendarDaySummary>,
    pub weeks: Vec<CalendarWeekSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalyticsTimeFilter {
    Last7Days,
    Last30Days,
    YearToDate,
    Last1Year,
    Custom {
        start_date: String,
        end_date: String,
    },
}

pub async fn get_journal_analytics(
    user_db: &UserDb,
    account_id: &str,
    time_filter: &AnalyticsTimeFilter,
) -> Result<JournalAnalytics> {
    let (start, end) = resolve_time_bounds(time_filter, Utc::now())?;
    let start_iso = start.to_rfc3339();
    let end_iso = end.to_rfc3339();

    let agg = journal_table::aggregate_journal_analytics(
        user_db.conn(),
        user_db.user_id(),
        account_id,
        &start_iso,
        &end_iso,
    )
    .await?;

    if agg.total_trades == 0 {
        return Ok(JournalAnalytics {
            win_rate: 0.0,
            cumulative_profit: 0.0,
            average_risk_to_reward: 0.0,
            average_gain: 0.0,
            average_loss: 0.0,
            profit_factor: None,
            biggest_win: None,
            biggest_loss: None,
        });
    }

    let biggest_win = journal_table::find_extreme_trade(
        user_db.conn(),
        user_db.user_id(),
        account_id,
        &start_iso,
        &end_iso,
        ExtremeKind::Best,
    )
    .await?;
    let biggest_loss = journal_table::find_extreme_trade(
        user_db.conn(),
        user_db.user_id(),
        account_id,
        &start_iso,
        &end_iso,
        ExtremeKind::Worst,
    )
    .await?;

    Ok(build_journal_analytics(agg, biggest_win, biggest_loss))
}

fn build_journal_analytics(
    agg: JournalAggregateRow,
    biggest_win: Option<TradeOutcomeRow>,
    biggest_loss: Option<TradeOutcomeRow>,
) -> JournalAnalytics {
    let total_trades = agg.total_trades as f64;
    let win_rate = (agg.winning_trades as f64 / total_trades) * 100.0;
    let average_risk_to_reward = agg.sum_risk_reward / total_trades;
    let average_gain = if agg.winning_trades == 0 {
        0.0
    } else {
        agg.gross_profit / agg.winning_trades as f64
    };
    let average_loss = if agg.losing_trades == 0 {
        0.0
    } else {
        agg.gross_loss / agg.losing_trades as f64
    };
    let profit_factor = if agg.gross_loss > 0.0 {
        Some(agg.gross_profit / agg.gross_loss)
    } else {
        None
    };

    JournalAnalytics {
        win_rate,
        cumulative_profit: agg.cumulative_profit,
        average_risk_to_reward,
        average_gain,
        average_loss,
        profit_factor,
        biggest_win: biggest_win.map(trade_outcome_from_row),
        biggest_loss: biggest_loss.map(trade_outcome_from_row),
    }
}

fn trade_outcome_from_row(row: TradeOutcomeRow) -> TradeOutcome {
    TradeOutcome {
        symbol: row.symbol,
        symbol_name: row.symbol_name,
        amount: row.amount,
    }
}

pub async fn get_advanced_analytics(
    user_db: &UserDb,
    account_id: &str,
    time_filter: &AnalyticsTimeFilter,
) -> Result<crate::service::read_service::analytics_advanced::AdvancedAnalytics> {
    let (start, end) = resolve_time_bounds(time_filter, Utc::now())?;
    let start_iso = start.to_rfc3339();
    let end_iso = end.to_rfc3339();

    let entries = journal_table::list_journal_entries_for_account_in_range(
        user_db.conn(),
        user_db.user_id(),
        account_id,
        &start_iso,
        &end_iso,
    )
    .await?;

    // Use SnapTrade's authoritative current total equity as the drawdown-%
    // denominator basis. Manual accounts (total_value NULL) yield None, which
    // falls back to the peak-cumulative-PnL denominator.
    let current_equity =
        accounts_table::find_account(user_db.conn(), account_id, user_db.user_id())
            .await?
            .and_then(|account| account.total_value);

    // Hydrate per-trade tags for the behavioral (clean/flawed, per-category)
    // metrics. Trades with no tags are simply absent from the map.
    let entry_ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
    let trade_tags = tags_table::tags_for_trades(user_db.conn(), &entry_ids).await?;

    Ok(
        crate::service::read_service::analytics_advanced::compute_advanced_analytics(
            &entries,
            current_equity,
            &trade_tags,
        ),
    )
}

pub async fn get_calendar_analytics(
    user_db: &UserDb,
    account_id: &str,
    year: i32,
    month: u32,
) -> Result<CalendarAnalytics> {
    ensure!((1..=12).contains(&month), "month must be between 1 and 12");

    let month_start =
        NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| anyhow!("Invalid calendar month"))?;
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).ok_or_else(|| anyhow!("Invalid next month"))?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).ok_or_else(|| anyhow!("Invalid next month"))?
    };
    let month_end = next_month - Duration::days(1);
    let grid_start = start_of_calendar_week(month_start);
    let grid_end = end_of_calendar_week(month_end);

    let day_rows = journal_table::aggregate_calendar_days(
        user_db.conn(),
        user_db.user_id(),
        account_id,
        &month_start.format("%Y-%m-%d").to_string(),
        &month_end.format("%Y-%m-%d").to_string(),
    )
    .await?;

    let mut days_by_date: BTreeMap<NaiveDate, CalendarDayAggregateRow> = BTreeMap::new();
    for row in day_rows {
        let parsed = NaiveDate::parse_from_str(&row.date, "%Y-%m-%d")
            .map_err(|e| anyhow!("Invalid date '{}' from aggregate: {e}", row.date))?;
        days_by_date.insert(parsed, row);
    }

    let mut days = Vec::new();
    let mut cursor = month_start;
    while cursor <= month_end {
        let summary = match days_by_date.get(&cursor) {
            Some(row) => CalendarDaySummary {
                date: cursor.format("%Y-%m-%d").to_string(),
                profit: row.profit,
                trade_count: row.trade_count as usize,
                win_rate: if row.trade_count == 0 {
                    0.0
                } else {
                    (row.winning_trade_count as f64 / row.trade_count as f64) * 100.0
                },
            },
            None => CalendarDaySummary {
                date: cursor.format("%Y-%m-%d").to_string(),
                profit: 0.0,
                trade_count: 0,
                win_rate: 0.0,
            },
        };
        days.push(summary);
        cursor += Duration::days(1);
    }

    let month_profit = days.iter().map(|d| d.profit).sum::<f64>();
    let trade_count = days.iter().map(|d| d.trade_count).sum::<usize>();
    let trading_days = days.iter().filter(|d| d.trade_count > 0).count();

    let mut weeks = Vec::new();
    let mut week_start = grid_start;
    let mut week_index = 1;
    while week_start <= grid_end {
        let week_end = week_start + Duration::days(6);
        let week_days = days
            .iter()
            .filter(|day| {
                NaiveDate::parse_from_str(&day.date, "%Y-%m-%d")
                    .map(|date| date >= week_start && date <= week_end)
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();

        weeks.push(CalendarWeekSummary {
            week_index,
            week_start: week_start.format("%Y-%m-%d").to_string(),
            week_end: week_end.format("%Y-%m-%d").to_string(),
            profit: week_days.iter().map(|day| day.profit).sum::<f64>(),
            trade_count: week_days.iter().map(|day| day.trade_count).sum::<usize>(),
            trading_days: week_days.iter().filter(|day| day.trade_count > 0).count(),
        });

        week_index += 1;
        week_start += Duration::days(7);
    }

    Ok(CalendarAnalytics {
        year,
        month,
        month_profit,
        trade_count,
        trading_days,
        grid_start: grid_start.format("%Y-%m-%d").to_string(),
        grid_end: grid_end.format("%Y-%m-%d").to_string(),
        days,
        weeks,
    })
}

fn resolve_time_bounds(
    time_filter: &AnalyticsTimeFilter,
    now: DateTime<Utc>,
) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    // Allow trades closed any time up to 1 year in the future so
    // forward-dated journal entries (e.g. close_date logged ahead of time)
    // are always included in range filters.
    let far_future = now + Duration::days(366);

    match time_filter {
        AnalyticsTimeFilter::Last7Days => Ok((now - Duration::days(7), far_future)),
        AnalyticsTimeFilter::Last30Days => Ok((now - Duration::days(30), far_future)),
        AnalyticsTimeFilter::YearToDate => {
            let start = NaiveDate::from_ymd_opt(now.year(), 1, 1)
                .and_then(|date| date.and_hms_opt(0, 0, 0))
                .ok_or_else(|| anyhow!("Failed to compute year-to-date start"))?;
            Ok((
                DateTime::<Utc>::from_naive_utc_and_offset(start, Utc),
                far_future,
            ))
        }
        AnalyticsTimeFilter::Last1Year => Ok((now - Duration::days(365), far_future)),
        AnalyticsTimeFilter::Custom {
            start_date,
            end_date,
        } => {
            let start = parse_filter_datetime(start_date, FilterBound::Start)?;
            let end = parse_filter_datetime(end_date, FilterBound::End)?;
            ensure!(
                end >= start,
                "custom end_date must be on or after start_date"
            );
            Ok((start, end))
        }
    }
}

fn start_of_calendar_week(date: NaiveDate) -> NaiveDate {
    let days_from_sunday = i64::from(date.weekday().num_days_from_sunday());
    date - Duration::days(days_from_sunday)
}

fn end_of_calendar_week(date: NaiveDate) -> NaiveDate {
    let days_until_saturday = 6_i64 - i64::from(date.weekday().num_days_from_sunday());
    date + Duration::days(days_until_saturday)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterBound {
    Start,
    End,
}

fn parse_filter_datetime(value: &str, bound: FilterBound) -> Result<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Ok(parsed.with_timezone(&Utc));
    }

    for format in [
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(value, format) {
            return Ok(DateTime::<Utc>::from_naive_utc_and_offset(parsed, Utc));
        }
    }

    if let Ok(parsed) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let datetime = match bound {
            FilterBound::Start => parsed
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| anyhow!("Invalid start date provided"))?,
            FilterBound::End => parsed
                .and_hms_nano_opt(23, 59, 59, 999_999_999)
                .ok_or_else(|| anyhow!("Invalid end date provided"))?,
        };
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc));
    }

    Err(anyhow!(
        "Invalid filter datetime format '{}'. Use RFC3339, YYYY-MM-DD HH:MM[:SS], or YYYY-MM-DD",
        value
    ))
}
