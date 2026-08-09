use crate::service::read_service::analytics as analytics_service;
use crate::service::read_service::analytics::{
    AnalyticsTimeFilter, CalendarAnalytics, CalendarDaySummary, CalendarWeekSummary,
    JournalAnalytics, TradeOutcome,
};
use crate::service::read_service::analytics_advanced::AdvancedAnalytics;
use async_graphql::{Context, Enum, InputObject, Object, Result, SimpleObject};
use std::sync::Arc;

use crate::service::redis::{RedisClient, analytics as analytics_cache};

async fn get_user_db(ctx: &Context<'_>) -> Result<crate::service::db::client::UserDb> {
    crate::graphql::auth::user_db(ctx).await
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum AnalyticsRange {
    Today,
    Last7Days,
    Last1Month,
    Last3Months,
    Last6Months,
    YearToDate,
    Last1Year,
    All,
    Custom,
}

#[derive(InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct AnalyticsTimeFilterInput {
    pub range: AnalyticsRange,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct TradeOutcomeGql {
    pub symbol: String,
    pub symbol_name: String,
    pub amount: f64,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct JournalAnalyticsGql {
    pub win_rate: f64,
    pub cumulative_profit: f64,
    pub average_risk_to_reward: f64,
    pub average_gain: f64,
    pub average_loss: f64,
    pub average_gain_pct: f64,
    pub average_loss_pct: f64,
    pub profit_factor: Option<f64>,
    pub biggest_win: Option<TradeOutcomeGql>,
    pub biggest_loss: Option<TradeOutcomeGql>,
    pub range_start: Option<String>,
    pub range_end: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct CalendarDaySummaryGql {
    pub date: String,
    pub profit: f64,
    pub trade_count: usize,
    pub win_rate: f64,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct CalendarWeekSummaryGql {
    pub week_index: usize,
    pub week_start: String,
    pub week_end: String,
    pub profit: f64,
    pub trade_count: usize,
    pub trading_days: usize,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct CalendarAnalyticsGql {
    pub year: i32,
    pub month: u32,
    pub month_profit: f64,
    pub trade_count: usize,
    pub trading_days: usize,
    pub grid_start: String,
    pub grid_end: String,
    pub days: Vec<CalendarDaySummaryGql>,
    pub weeks: Vec<CalendarWeekSummaryGql>,
}

impl From<TradeOutcome> for TradeOutcomeGql {
    fn from(value: TradeOutcome) -> Self {
        Self {
            symbol: value.symbol,
            symbol_name: value.symbol_name,
            amount: value.amount,
        }
    }
}

impl From<JournalAnalytics> for JournalAnalyticsGql {
    fn from(value: JournalAnalytics) -> Self {
        Self {
            win_rate: value.win_rate,
            cumulative_profit: value.cumulative_profit,
            average_risk_to_reward: value.average_risk_to_reward,
            average_gain: value.average_gain,
            average_loss: value.average_loss,
            average_gain_pct: value.average_gain_pct,
            average_loss_pct: value.average_loss_pct,
            profit_factor: value.profit_factor,
            biggest_win: value.biggest_win.map(Into::into),
            biggest_loss: value.biggest_loss.map(Into::into),
            range_start: value.range_start,
            range_end: value.range_end,
        }
    }
}

impl From<CalendarDaySummary> for CalendarDaySummaryGql {
    fn from(value: CalendarDaySummary) -> Self {
        Self {
            date: value.date,
            profit: value.profit,
            trade_count: value.trade_count,
            win_rate: value.win_rate,
        }
    }
}

impl From<CalendarWeekSummary> for CalendarWeekSummaryGql {
    fn from(value: CalendarWeekSummary) -> Self {
        Self {
            week_index: value.week_index,
            week_start: value.week_start,
            week_end: value.week_end,
            profit: value.profit,
            trade_count: value.trade_count,
            trading_days: value.trading_days,
        }
    }
}

impl From<CalendarAnalytics> for CalendarAnalyticsGql {
    fn from(value: CalendarAnalytics) -> Self {
        Self {
            year: value.year,
            month: value.month,
            month_profit: value.month_profit,
            trade_count: value.trade_count,
            trading_days: value.trading_days,
            grid_start: value.grid_start,
            grid_end: value.grid_end,
            days: value.days.into_iter().map(Into::into).collect(),
            weeks: value.weeks.into_iter().map(Into::into).collect(),
        }
    }
}

pub(crate) fn map_time_filter(input: AnalyticsTimeFilterInput) -> Result<AnalyticsTimeFilter> {
    match input.range {
        AnalyticsRange::Today => Ok(AnalyticsTimeFilter::Today),
        AnalyticsRange::Last7Days => Ok(AnalyticsTimeFilter::Last7Days),
        AnalyticsRange::Last1Month => Ok(AnalyticsTimeFilter::Last1Month),
        AnalyticsRange::Last3Months => Ok(AnalyticsTimeFilter::Last3Months),
        AnalyticsRange::Last6Months => Ok(AnalyticsTimeFilter::Last6Months),
        AnalyticsRange::YearToDate => Ok(AnalyticsTimeFilter::YearToDate),
        AnalyticsRange::Last1Year => Ok(AnalyticsTimeFilter::Last1Year),
        AnalyticsRange::All => Ok(AnalyticsTimeFilter::All),
        AnalyticsRange::Custom => {
            let start_date = input.start_date.ok_or_else(|| {
                async_graphql::Error::new("startDate is required for CUSTOM range")
            })?;
            let end_date = input
                .end_date
                .ok_or_else(|| async_graphql::Error::new("endDate is required for CUSTOM range"))?;
            Ok(AnalyticsTimeFilter::Custom {
                start_date,
                end_date,
            })
        }
    }
}

fn time_filter_cache_key(filter: &AnalyticsTimeFilter) -> String {
    match filter {
        AnalyticsTimeFilter::Today => "today".into(),
        AnalyticsTimeFilter::Last7Days => "last-7-days".into(),
        AnalyticsTimeFilter::Last1Month => "last-1-month".into(),
        AnalyticsTimeFilter::Last3Months => "last-3-months".into(),
        AnalyticsTimeFilter::Last6Months => "last-6-months".into(),
        AnalyticsTimeFilter::YearToDate => "year-to-date".into(),
        AnalyticsTimeFilter::Last1Year => "last-1-year".into(),
        AnalyticsTimeFilter::All => "all".into(),
        AnalyticsTimeFilter::Custom {
            start_date,
            end_date,
        } => format!("custom:{start_date}:{end_date}"),
    }
}

#[derive(Default)]
pub struct AnalyticsQuery;

#[Object]
impl AnalyticsQuery {
    async fn journal_analytics(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        time_filter: AnalyticsTimeFilterInput,
    ) -> Result<JournalAnalyticsGql> {
        let user_db = get_user_db(ctx).await?;
        let time_filter = map_time_filter(time_filter)?;
        let cache_key = time_filter_cache_key(&time_filter);
        let analytics = if let Ok(redis) = ctx.data::<Arc<RedisClient>>() {
            analytics_cache::get_or_load(
                redis,
                &user_db,
                &workspace_id,
                "journal",
                &cache_key,
                || analytics_service::get_journal_analytics(&user_db, &workspace_id, &time_filter),
            )
            .await?
        } else {
            analytics_service::get_journal_analytics(&user_db, &workspace_id, &time_filter).await?
        };

        Ok(analytics.into())
    }

    async fn calendar_analytics(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        year: i32,
        month: u32,
    ) -> Result<CalendarAnalyticsGql> {
        let user_db = get_user_db(ctx).await?;
        let cache_key = format!("{year}:{month}");
        let analytics = if let Ok(redis) = ctx.data::<Arc<RedisClient>>() {
            analytics_cache::get_or_load(
                redis,
                &user_db,
                &workspace_id,
                "calendar",
                &cache_key,
                || analytics_service::get_calendar_analytics(&user_db, &workspace_id, year, month),
            )
            .await?
        } else {
            analytics_service::get_calendar_analytics(&user_db, &workspace_id, year, month).await?
        };

        Ok(analytics.into())
    }

    async fn advanced_analytics(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        time_filter: AnalyticsTimeFilterInput,
    ) -> Result<AdvancedAnalytics> {
        let user_db = get_user_db(ctx).await?;
        let time_filter = map_time_filter(time_filter)?;
        let cache_key = time_filter_cache_key(&time_filter);
        let analytics = if let Ok(redis) = ctx.data::<Arc<RedisClient>>() {
            analytics_cache::get_or_load(
                redis,
                &user_db,
                &workspace_id,
                "advanced",
                &cache_key,
                || analytics_service::get_advanced_analytics(&user_db, &workspace_id, &time_filter),
            )
            .await?
        } else {
            analytics_service::get_advanced_analytics(&user_db, &workspace_id, &time_filter).await?
        };

        Ok(analytics)
    }
}
