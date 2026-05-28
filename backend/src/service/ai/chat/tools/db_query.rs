use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::turso::TursoClient;

#[derive(Debug, Deserialize)]
struct DbQueryInput {
    entity: String,
    #[serde(default)]
    filters: QueryFilters,
    limit: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct QueryFilters {
    symbol: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    trade_type: Option<String>,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "db_query".to_string(),
            description: "Query trades or journal entries from the database. \
                          For playbooks, use the get_playbook tool instead."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "entity": {
                        "type": "string",
                        "enum": ["trades", "journal"],
                        "description": "The type of data to query."
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
                            },
                            "trade_type": {
                                "type": "string",
                                "enum": ["long", "short"],
                                "description": "Filter by trade direction."
                            }
                        }
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of records to return (default 20, max 50)."
                    }
                },
                "required": ["entity"]
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
    let input: DbQueryInput = serde_json::from_str(arguments)?;
    let limit = input.limit.unwrap_or(20).min(50);
    let conn = turso.get_connection()?;

    match input.entity.as_str() {
        "trades" => {
            let mut sql = "SELECT id, symbol, symbol_name, trade_type, open_date, close_date, \
                           entry_price, exit_price, position_size, total_pl, net_roi, \
                           risk_reward, status, notes \
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
            if let Some(tt) = &input.filters.trade_type {
                sql.push_str(" AND trade_type = ?");
                params.push(libsql::Value::Text(tt.clone()));
            }

            sql.push_str(&format!(" LIMIT {}", limit));

            let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
            let mut results = Vec::new();
            while let Some(row) = rows.next().await? {
                results.push(json!({
                    "id": row.get::<String>(0).unwrap_or_default(),
                    "symbol": row.get::<String>(1).unwrap_or_default(),
                    "symbol_name": row.get::<String>(2).unwrap_or_default(),
                    "trade_type": row.get::<String>(3).unwrap_or_default(),
                    "open_date": row.get::<String>(4).unwrap_or_default(),
                    "close_date": row.get::<String>(5).unwrap_or_default(),
                    "entry_price": row.get::<f64>(6).unwrap_or_default(),
                    "exit_price": row.get::<f64>(7).unwrap_or_default(),
                    "position_size": row.get::<f64>(8).unwrap_or_default(),
                    "total_pl": row.get::<f64>(9).unwrap_or_default(),
                    "net_roi": row.get::<f64>(10).unwrap_or_default(),
                    "risk_reward": row.get::<f64>(11).unwrap_or_default(),
                    "status": row.get::<String>(12).unwrap_or_default(),
                    "notes": row.get::<Option<String>>(13).unwrap_or(None),
                }));
            }
            Ok(serde_json::to_string(&results)?)
        }
        "journal" => {
            let mut sql = "SELECT id, symbol, open_date, close_date, notes, mistakes, \
                           entry_tactics, edges_spotted \
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

            sql.push_str(&format!(" LIMIT {}", limit));

            let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
            let mut results = Vec::new();
            while let Some(row) = rows.next().await? {
                results.push(json!({
                    "id": row.get::<String>(0).unwrap_or_default(),
                    "symbol": row.get::<String>(1).unwrap_or_default(),
                    "open_date": row.get::<String>(2).unwrap_or_default(),
                    "close_date": row.get::<String>(3).unwrap_or_default(),
                    "notes": row.get::<Option<String>>(4).unwrap_or(None),
                    "mistakes": row.get::<String>(5).unwrap_or_default(),
                    "entry_tactics": row.get::<String>(6).unwrap_or_default(),
                    "edges_spotted": row.get::<String>(7).unwrap_or_default(),
                }));
            }
            Ok(serde_json::to_string(&results)?)
        }
        other => Ok(format!("Unknown entity type: {}", other)),
    }
}
