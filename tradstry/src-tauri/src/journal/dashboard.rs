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
