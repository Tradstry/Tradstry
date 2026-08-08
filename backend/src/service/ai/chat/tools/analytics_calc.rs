use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;

use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::db::Db;
use crate::service::db::util::{parse_flexible_datetime, parse_flexible_end_datetime};
use crate::service::read_service::analytics::{self, AnalyticsTimeFilter};

#[derive(Debug, Deserialize)]
struct AnalyticsInput {
    metrics: Vec<String>,
    #[serde(default)]
    filters: AnalyticsFilters,
}

#[derive(Debug, Default, Deserialize)]
struct AnalyticsFilters {
    trade_ids: Option<Vec<String>>,
    symbol: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
}

struct TradeRow {
    total_pl: f64,
    dollar_pl: f64,
    symbol: String,
    risk_reward: f64,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "analytics_calc".to_string(),
            description:
                "Compute trading performance metrics including win rate, total PnL, average R, \
                 profit factor, streak, and per-symbol breakdowns (basic), PLUS advanced analytics: \
                 expectancy ($ and R-multiple), profit factor, SQN (System Quality Number), \
                 max drawdown / recovery factor, equity curve, R-distribution, win/loss streaks, \
                 holding-time buckets, and dimensional breakdowns by symbol, day-of-week, session, \
                 holding duration, direction, and playbook. Also includes behavioral metrics: \
                 mistake cost."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "metrics": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["win_rate", "total_pnl", "avg_r", "profit_factor", "streak", "per_symbol"]
                        },
                        "description": "List of metrics to compute."
                    },
                    "filters": {
                        "type": "object",
                        "properties": {
                            "trade_ids": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Exact internal trade IDs to include. Use for tagged trades and never repeat these IDs in the user-facing answer."
                            },
                            "symbol": {
                                "type": "string",
                                "description": "Filter by ticker symbol."
                            },
                            "date_from": {
                                "type": "string",
                                "description": "Start date filter (ISO 8601)."
                            },
                            "date_to": {
                                "type": "string",
                                "description": "End date filter (ISO 8601)."
                            }
                        }
                    }
                },
                "required": ["metrics"]
            }),
        },
    }
}

pub async fn execute(
    arguments: &str,
    user_id: &str,
    workspace_id: &str,
    db: &Arc<Db>,
) -> Result<String> {
    let input: AnalyticsInput = serde_json::from_str(arguments)?;

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT total_pl, symbol, risk_reward, entry_price, position_size, contract_multiplier \
         FROM journal_entries WHERE user_id = ",
    );
    qb.push_bind(user_id);
    qb.push(" AND workspace_id = ")
        .push_bind(workspace_id)
        .push(" AND deleted_at IS NULL");

    let has_exact_trade_scope = input
        .filters
        .trade_ids
        .as_ref()
        .is_some_and(|ids| !ids.is_empty());
    if let Some(ids) = &input.filters.trade_ids
        && !ids.is_empty()
    {
        qb.push(" AND id = ANY(").push_bind(ids).push(")");
    }

    if let Some(sym) = &input.filters.symbol {
        qb.push(" AND symbol = UPPER(").push_bind(sym).push(")");
    }
    if let Some(from) = &input.filters.date_from {
        qb.push(" AND open_date >= ")
            .push_bind(parse_flexible_datetime(from)?);
    }
    if let Some(to) = &input.filters.date_to {
        qb.push(" AND close_date <= ")
            .push_bind(parse_flexible_end_datetime(to)?);
    }

    qb.push(" ORDER BY close_date ASC, open_date ASC");

    let rows = qb.build().fetch_all(db.pool()).await?;
    let mut trades: Vec<TradeRow> = Vec::new();
    for row in &rows {
        let total_pl = row.try_get::<f64, _>("total_pl").unwrap_or_default();
        let entry_price = row.try_get::<f64, _>("entry_price").unwrap_or_default();
        let position_size = row.try_get::<f64, _>("position_size").unwrap_or_default();
        let contract_multiplier = row.try_get::<f64, _>("contract_multiplier").unwrap_or(1.0);
        trades.push(TradeRow {
            total_pl,
            dollar_pl: calculate_dollar_pl(
                total_pl,
                entry_price,
                position_size,
                contract_multiplier,
            ),
            symbol: row.try_get::<String, _>("symbol").unwrap_or_default(),
            risk_reward: row.try_get::<f64, _>("risk_reward").unwrap_or_default(),
        });
    }

    let mut result: HashMap<String, Value> = HashMap::new();

    for metric in &input.metrics {
        match metric.as_str() {
            "win_rate" => {
                let total = trades.len();
                if total == 0 {
                    result.insert("win_rate".to_string(), json!(0.0));
                } else {
                    let wins = trades.iter().filter(|t| t.total_pl > 0.0).count();
                    result.insert(
                        "win_rate".to_string(),
                        json!((wins as f64 / total as f64) * 100.0),
                    );
                }
            }
            "total_pnl" => {
                let total: f64 = trades.iter().map(|t| t.dollar_pl).sum();
                result.insert("total_pnl".to_string(), json!(total));
            }
            "avg_r" => {
                if trades.is_empty() {
                    result.insert("avg_r".to_string(), json!(0.0));
                } else {
                    let avg: f64 =
                        trades.iter().map(|t| t.risk_reward).sum::<f64>() / trades.len() as f64;
                    result.insert("avg_r".to_string(), json!(avg));
                }
            }
            "profit_factor" => {
                let gross_profit: f64 = trades
                    .iter()
                    .filter(|t| t.total_pl > 0.0)
                    .map(|t| t.dollar_pl)
                    .sum();
                let gross_loss: f64 = trades
                    .iter()
                    .filter(|t| t.total_pl < 0.0)
                    .map(|t| t.dollar_pl.abs())
                    .sum();
                let pf = if gross_loss == 0.0 {
                    if gross_profit > 0.0 {
                        f64::INFINITY
                    } else {
                        0.0
                    }
                } else {
                    gross_profit / gross_loss
                };
                result.insert("profit_factor".to_string(), json!(pf));
            }
            "streak" => {
                let (max_wins, max_losses) = compute_streaks(&trades);
                result.insert(
                    "streak".to_string(),
                    json!({
                        "max_consecutive_wins": max_wins,
                        "max_consecutive_losses": max_losses,
                    }),
                );
            }
            "per_symbol" => {
                let mut by_symbol: HashMap<String, Vec<f64>> = HashMap::new();
                for t in &trades {
                    by_symbol
                        .entry(t.symbol.clone())
                        .or_default()
                        .push(t.dollar_pl);
                }
                let symbol_stats: Vec<Value> = by_symbol
                    .into_iter()
                    .map(|(sym, pls)| {
                        let count = pls.len();
                        let pnl: f64 = pls.iter().sum();
                        let wins = pls.iter().filter(|&&p| p > 0.0).count();
                        let win_rate = if count > 0 {
                            (wins as f64 / count as f64) * 100.0
                        } else {
                            0.0
                        };
                        json!({
                            "symbol": sym,
                            "pnl": pnl,
                            "trade_count": count,
                            "win_rate": win_rate,
                        })
                    })
                    .collect();
                result.insert("per_symbol".to_string(), json!(symbol_stats));
            }
            unknown => {
                result.insert(
                    unknown.to_string(),
                    json!(format!("Unknown metric: {}", unknown)),
                );
            }
        }
    }

    // Advanced analytics currently accepts a date range, but not exact trade IDs.
    // Do not mix workspace-wide statistics into a tagged-trade answer.
    if has_exact_trade_scope {
        result.insert(
            "scope".to_string(),
            json!({
                "selected_trades": true,
                "trade_count": trades.len(),
            }),
        );
    } else if let Some(time_filter) = advanced_time_filter(&input.filters) {
        let user_db = db.get_user_db(user_id);
        match analytics::get_advanced_analytics(&user_db, workspace_id, &time_filter).await {
            Ok(advanced) => {
                result.insert(
                    "advanced".to_string(),
                    serde_json::to_value(&advanced).unwrap_or(Value::Null),
                );
            }
            Err(e) => {
                result.insert(
                    "advanced_error".to_string(),
                    json!(format!("Advanced analytics unavailable: {e}")),
                );
            }
        }
    } else {
        result.insert(
            "advanced_scope".to_owned(),
            json!("Advanced breakdowns were omitted because they cannot enforce this exact filter; basic metrics above use the requested scope."),
        );
    }

    Ok(serde_json::to_string(&result)?)
}

fn advanced_time_filter(filters: &AnalyticsFilters) -> Option<AnalyticsTimeFilter> {
    if filters.symbol.is_some() {
        return None;
    }
    match (&filters.date_from, &filters.date_to) {
        (Some(from), Some(to)) => Some(AnalyticsTimeFilter::Custom {
            start_date: from.clone(),
            end_date: to.clone(),
        }),
        (None, None) => Some(AnalyticsTimeFilter::All),
        _ => None,
    }
}

fn calculate_dollar_pl(
    total_pl_percent: f64,
    entry_price: f64,
    position_size: f64,
    contract_multiplier: f64,
) -> f64 {
    position_size * entry_price * total_pl_percent / 100.0 * contract_multiplier
}

fn compute_streaks(trades: &[TradeRow]) -> (usize, usize) {
    let mut max_wins = 0usize;
    let mut max_losses = 0usize;
    let mut cur_wins = 0usize;
    let mut cur_losses = 0usize;

    for t in trades {
        if t.total_pl > 0.0 {
            cur_wins += 1;
            cur_losses = 0;
            if cur_wins > max_wins {
                max_wins = cur_wins;
            }
        } else if t.total_pl < 0.0 {
            cur_losses += 1;
            cur_wins = 0;
            if cur_losses > max_losses {
                max_losses = cur_losses;
            }
        } else {
            // break-even resets both
            cur_wins = 0;
            cur_losses = 0;
        }
    }

    (max_wins, max_losses)
}

#[cfg(test)]
mod tests {
    use super::{AnalyticsFilters, advanced_time_filter, calculate_dollar_pl};
    use crate::service::read_service::analytics::AnalyticsTimeFilter;

    #[test]
    fn converts_percentage_return_to_cash_pnl() {
        assert_eq!(calculate_dollar_pl(10.0, 50.0, 2.0, 1.0), 10.0);
        assert_eq!(calculate_dollar_pl(-5.0, 20.0, 10.0, 1.0), -10.0);
    }

    #[test]
    fn applies_derivative_contract_multiplier() {
        assert_eq!(calculate_dollar_pl(2.0, 5.0, 3.0, 100.0), 30.0);
    }

    #[test]
    fn unfiltered_basic_and_advanced_metrics_are_both_all_time() {
        assert!(matches!(
            advanced_time_filter(&AnalyticsFilters::default()),
            Some(AnalyticsTimeFilter::All)
        ));
    }

    #[test]
    fn advanced_metrics_are_omitted_when_they_cannot_enforce_symbol_scope() {
        let filters = AnalyticsFilters {
            symbol: Some("NVDA".to_owned()),
            ..Default::default()
        };
        assert!(advanced_time_filter(&filters).is_none());
    }
}
