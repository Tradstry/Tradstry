// Analytics API. The GraphQL query lives here in Rust; the frontend only calls
// `invoke("journal_analytics", { accountId, timeFilter })`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    profit_factor: f64,
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
