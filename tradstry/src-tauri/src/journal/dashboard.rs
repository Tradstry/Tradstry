// Dashboard API for the Journal app. GraphQL queries live here in Rust; the
// frontend calls invoke("journal_analytics", …) and invoke("calendar_analytics", …).

use serde::{Deserialize, Serialize};
use serde_json::Value;

// --- Journal analytics (upper metric cards) -------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeOutcome {
    symbol: String,
    symbol_name: Option<String>,
    amount: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalAnalytics {
    win_rate: f64,
    cumulative_profit: f64,
    average_risk_to_reward: f64,
    average_gain: f64,
    average_loss: f64,
    average_gain_pct: f64,
    average_loss_pct: f64,
    // Null when the ratio is undefined (e.g. no losing trades / no trades).
    profit_factor: Option<f64>,
    biggest_win: Option<TradeOutcome>,
    biggest_loss: Option<TradeOutcome>,
    range_start: Option<String>,
    range_end: Option<String>,
}

const JOURNAL_ANALYTICS_QUERY: &str = r#"
query JournalAnalytics($accountId: String!, $timeFilter: AnalyticsTimeFilterInput!) {
  journalAnalytics(accountId: $accountId, timeFilter: $timeFilter) {
    winRate
    cumulativeProfit
    averageRiskToReward
    averageGain
    averageLoss
    averageGainPct
    averageLossPct
    profitFactor
    biggestWin { symbol symbolName amount }
    biggestLoss { symbol symbolName amount }
    rangeStart
    rangeEnd
  }
}
"#;

/// `timeFilter` is passed through as-is (shape: { range, startDate?, endDate? }).
#[tauri::command]
pub async fn journal_analytics(
    account_id: String,
    time_filter: Value,
) -> Result<JournalAnalytics, String> {
    let variables = serde_json::json!({
        "accountId": account_id,
        "timeFilter": time_filter,
    });
    let data = crate::api::graphql(JOURNAL_ANALYTICS_QUERY, variables).await?;
    let node = data
        .get("journalAnalytics")
        .cloned()
        .ok_or("missing journalAnalytics in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

// --- Calendar analytics (trading calendar) --------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDay {
    date: String,
    profit: f64,
    trade_count: i64,
    win_rate: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarWeek {
    week_index: i64,
    week_start: String,
    week_end: String,
    profit: f64,
    trade_count: i64,
    trading_days: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarAnalytics {
    year: i64,
    month: i64,
    month_profit: f64,
    trade_count: i64,
    trading_days: i64,
    grid_start: String,
    grid_end: String,
    days: Vec<CalendarDay>,
    weeks: Vec<CalendarWeek>,
}

const CALENDAR_ANALYTICS_QUERY: &str = r#"
query CalendarAnalytics($accountId: String!, $year: Int!, $month: Int!) {
  calendarAnalytics(accountId: $accountId, year: $year, month: $month) {
    year
    month
    monthProfit
    tradeCount
    tradingDays
    gridStart
    gridEnd
    days { date profit tradeCount winRate }
    weeks { weekIndex weekStart weekEnd profit tradeCount tradingDays }
  }
}
"#;

#[tauri::command]
pub async fn calendar_analytics(
    account_id: String,
    year: i64,
    month: i64,
) -> Result<CalendarAnalytics, String> {
    let variables = serde_json::json!({
        "accountId": account_id,
        "year": year,
        "month": month,
    });
    let data = crate::api::graphql(CALENDAR_ANALYTICS_QUERY, variables).await?;
    let node = data
        .get("calendarAnalytics")
        .cloned()
        .ok_or("missing calendarAnalytics in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

// --- Advanced analytics (the Analytics page) ------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMetrics {
    trade_count: i64,
    net_profit: f64,
    win_rate: f64,
    expectancy_dollars: f64,
    expectancy_r: Option<f64>,
    profit_factor: Option<f64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DimensionStat {
    key: String,
    metrics: GroupMetrics,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquityPoint {
    close_date: String,
    equity: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RBucket {
    label: String,
    count: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanFlawed {
    clean: GroupMetrics,
    flawed: GroupMetrics,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradesPerDay {
    avg: f64,
    max: i64,
    stdev: Option<f64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryBreakdown {
    category_name: String,
    role: Option<String>,
    tags: Vec<DimensionStat>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedAnalytics {
    trade_count: i64,
    net_profit: f64,
    win_rate: f64,
    expectancy_dollars: f64,
    expectancy_r: Option<f64>,
    r_trade_count: i64,
    profit_factor: Option<f64>,
    sqn: Option<f64>,
    average_gain: f64,
    average_loss: f64,
    average_gain_pct: f64,
    average_loss_pct: f64,
    max_drawdown_dollars: f64,
    max_drawdown_pct: f64,
    current_drawdown_dollars: f64,
    recovery_factor: Option<f64>,
    longest_drawdown_days: i64,
    equity_curve: Vec<EquityPoint>,
    starting_equity: Option<f64>,
    account_equity: Option<f64>,
    avg_planned_r: Option<f64>,
    avg_actual_r: Option<f64>,
    r_distribution: Vec<RBucket>,
    longest_win_streak: i64,
    longest_loss_streak: i64,
    current_streak: i64,
    avg_hold_winners_secs: Option<f64>,
    avg_hold_losers_secs: Option<f64>,
    monthly_win_rate_stdev: Option<f64>,
    trades_per_day: TradesPerDay,
    by_symbol: Vec<DimensionStat>,
    by_day_of_week: Vec<DimensionStat>,
    by_session: Vec<DimensionStat>,
    by_holding: Vec<DimensionStat>,
    by_direction: Vec<DimensionStat>,
    by_position_size: Vec<DimensionStat>,
    by_playbook: Vec<DimensionStat>,
    clean_vs_flawed: CleanFlawed,
    tag_breakdowns: Vec<CategoryBreakdown>,
    range_start: Option<String>,
    range_end: Option<String>,
}

const ADVANCED_ANALYTICS_QUERY: &str = r#"
query AdvancedAnalytics($accountId: String!, $timeFilter: AnalyticsTimeFilterInput!) {
  advancedAnalytics(accountId: $accountId, timeFilter: $timeFilter) {
    tradeCount netProfit winRate expectancyDollars expectancyR rTradeCount
    profitFactor sqn averageGain averageLoss averageGainPct averageLossPct
    maxDrawdownDollars maxDrawdownPct currentDrawdownDollars recoveryFactor
    longestDrawdownDays equityCurve { closeDate equity } startingEquity accountEquity
    avgPlannedR avgActualR rDistribution { label count }
    longestWinStreak longestLossStreak currentStreak
    avgHoldWinnersSecs avgHoldLosersSecs monthlyWinRateStdev
    tradesPerDay { avg max stdev }
    bySymbol { key metrics { ...GM } }
    byDayOfWeek { key metrics { ...GM } }
    bySession { key metrics { ...GM } }
    byHolding { key metrics { ...GM } }
    byDirection { key metrics { ...GM } }
    byPositionSize { key metrics { ...GM } }
    byPlaybook { key metrics { ...GM } }
    cleanVsFlawed { clean { ...GM } flawed { ...GM } }
    tagBreakdowns { categoryName role tags { key metrics { ...GM } } }
    rangeStart rangeEnd
  }
}
fragment GM on GroupMetrics {
  tradeCount netProfit winRate expectancyDollars expectancyR profitFactor
}
"#;

/// `timeFilter` is passed through as-is (shape: { range, startDate?, endDate? }).
#[tauri::command]
pub async fn advanced_analytics(
    account_id: String,
    time_filter: Value,
) -> Result<AdvancedAnalytics, String> {
    let variables = serde_json::json!({
        "accountId": account_id,
        "timeFilter": time_filter,
    });
    let data = crate::api::graphql(ADVANCED_ANALYTICS_QUERY, variables).await?;
    let node = data
        .get("advancedAnalytics")
        .cloned()
        .ok_or("missing advancedAnalytics in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}
