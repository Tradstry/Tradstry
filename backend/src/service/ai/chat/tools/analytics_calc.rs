use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::read_service::analytics::{self, AnalyticsTimeFilter};
use crate::service::turso::TursoClient;

#[derive(Debug, Deserialize)]
struct AnalyticsInput {
    metrics: Vec<String>,
    #[serde(default)]
    filters: AnalyticsFilters,
}

#[derive(Debug, Default, Deserialize)]
struct AnalyticsFilters {
    symbol: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
}

struct TradeRow {
    total_pl: f64,
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
    account_id: &str,
    turso: &Arc<TursoClient>,
) -> Result<String> {
    let input: AnalyticsInput = serde_json::from_str(arguments)?;
    let conn = turso.get_connection()?;

    let mut sql = "SELECT total_pl, symbol, risk_reward \
                   FROM journal_entries WHERE user_id = ? AND account_id = ?"
        .to_string();
    let mut params: Vec<libsql::Value> = vec![
        libsql::Value::Text(user_id.to_string()),
        libsql::Value::Text(account_id.to_string()),
    ];

    if let Some(sym) = &input.filters.symbol {
        sql.push_str(" AND symbol = ?");
        params.push(libsql::Value::Text(sym.clone()));
    }
    if let Some(from) = &input.filters.date_from {
        sql.push_str(" AND open_date >= ?");
        params.push(libsql::Value::Text(from.clone()));
    }
    if let Some(to) = &input.filters.date_to {
        sql.push_str(" AND close_date <= ?");
        params.push(libsql::Value::Text(to.clone()));
    }

    let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
    let mut trades: Vec<TradeRow> = Vec::new();
    while let Some(row) = rows.next().await? {
        trades.push(TradeRow {
            total_pl: row.get::<f64>(0).unwrap_or_default(),
            symbol: row.get::<String>(1).unwrap_or_default(),
            risk_reward: row.get::<f64>(2).unwrap_or_default(),
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
                let total: f64 = trades.iter().map(|t| t.total_pl).sum();
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
                    .map(|t| t.total_pl)
                    .sum();
                let gross_loss: f64 = trades
                    .iter()
                    .filter(|t| t.total_pl < 0.0)
                    .map(|t| t.total_pl.abs())
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
                        .push(t.total_pl);
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

    // ---- Advanced analytics (additive — merged under "advanced" key) ----
    let time_filter = match (&input.filters.date_from, &input.filters.date_to) {
        (Some(from), Some(to)) => AnalyticsTimeFilter::Custom {
            start_date: from.clone(),
            end_date: to.clone(),
        },
        _ => AnalyticsTimeFilter::Last1Month,
    };

    let user_db = turso.get_user_db(user_id).await?;
    match analytics::get_advanced_analytics(&user_db, account_id, &time_filter).await {
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

    Ok(serde_json::to_string(&result)?)
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
