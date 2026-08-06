use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;

use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::db::Db;
use crate::service::db::schema::tables::tags_table;
use crate::service::db::util::parse_flexible_datetime;

#[derive(Debug, Deserialize)]
struct DbQueryInput {
    entity: String,
    #[serde(default)]
    filters: QueryFilters,
    limit: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct QueryFilters {
    trade_ids: Option<Vec<String>>,
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
                            "trade_ids": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Exact internal trade IDs to fetch. Use for tagged trades and never repeat these IDs in the user-facing answer."
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
    workspace_id: &str,
    db: &Arc<Db>,
) -> Result<String> {
    let input: DbQueryInput = serde_json::from_str(arguments)?;
    let limit = input.limit.unwrap_or(20).min(50);

    match input.entity.as_str() {
        "trades" => {
            let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
                "SELECT id, symbol, symbol_name, trade_type, \
                 to_char(open_date AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS open_date, \
                 to_char(close_date AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS close_date, \
                 entry_price, exit_price, position_size, total_pl, net_roi, \
                 risk_reward, status, notes \
                 FROM journal_entries WHERE user_id = ",
            );
            qb.push_bind(user_id);
            qb.push(" AND workspace_id = ")
                .push_bind(workspace_id)
                .push(" AND deleted_at IS NULL");

            if let Some(ids) = &input.filters.trade_ids
                && !ids.is_empty()
            {
                qb.push(" AND id = ANY(").push_bind(ids).push(")");
            }
            if let Some(sym) = &input.filters.symbol {
                qb.push(" AND symbol = ").push_bind(sym);
            }
            if let Some(from) = &input.filters.date_from {
                qb.push(" AND open_date >= ")
                    .push_bind(parse_flexible_datetime(from)?);
            }
            if let Some(to) = &input.filters.date_to {
                qb.push(" AND close_date <= ")
                    .push_bind(parse_flexible_datetime(to)?);
            }
            if let Some(tt) = &input.filters.trade_type {
                qb.push(" AND trade_type = ").push_bind(tt);
            }

            qb.push(" ORDER BY open_date DESC, close_date DESC LIMIT ")
                .push_bind(limit as i64);

            let rows = qb.build().fetch_all(db.pool()).await?;
            let mut results = Vec::new();
            for row in &rows {
                results.push(json!({
                    "id": row.try_get::<String, _>(0).unwrap_or_default(),
                    "symbol": row.try_get::<String, _>(1).unwrap_or_default(),
                    "symbol_name": row.try_get::<String, _>(2).unwrap_or_default(),
                    "trade_type": row.try_get::<String, _>(3).unwrap_or_default(),
                    "open_date": row.try_get::<String, _>(4).unwrap_or_default(),
                    "close_date": row.try_get::<String, _>(5).unwrap_or_default(),
                    "entry_price": row.try_get::<f64, _>(6).unwrap_or_default(),
                    "exit_price": row.try_get::<f64, _>(7).unwrap_or_default(),
                    "position_size": row.try_get::<f64, _>(8).unwrap_or_default(),
                    "total_pl": row.try_get::<f64, _>(9).unwrap_or_default(),
                    "net_roi": row.try_get::<f64, _>(10).unwrap_or_default(),
                    "risk_reward": row.try_get::<f64, _>(11).unwrap_or_default(),
                    "status": row.try_get::<String, _>(12).unwrap_or_default(),
                    "notes": row.try_get::<Option<String>, _>(13).unwrap_or(None),
                }));
            }
            Ok(serde_json::to_string(&results)?)
        }
        "journal" => {
            let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
                "SELECT id, symbol, \
                 to_char(open_date AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS open_date, \
                 to_char(close_date AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS close_date, \
                 notes, mistakes, entry_tactics, edges_spotted \
                 FROM journal_entries WHERE user_id = ",
            );
            qb.push_bind(user_id);
            qb.push(" AND workspace_id = ")
                .push_bind(workspace_id)
                .push(" AND deleted_at IS NULL");

            if let Some(ids) = &input.filters.trade_ids
                && !ids.is_empty()
            {
                qb.push(" AND id = ANY(").push_bind(ids).push(")");
            }
            if let Some(sym) = &input.filters.symbol {
                qb.push(" AND symbol = ").push_bind(sym);
            }
            if let Some(from) = &input.filters.date_from {
                qb.push(" AND open_date >= ")
                    .push_bind(parse_flexible_datetime(from)?);
            }
            if let Some(to) = &input.filters.date_to {
                qb.push(" AND close_date <= ")
                    .push_bind(parse_flexible_datetime(to)?);
            }

            qb.push(" ORDER BY open_date DESC, close_date DESC LIMIT ")
                .push_bind(limit as i64);

            let rows = qb.build().fetch_all(db.pool()).await?;
            let mut results = Vec::new();
            let mut ids = Vec::new();
            for row in &rows {
                let id = row.try_get::<String, _>(0).unwrap_or_default();
                ids.push(id.clone());
                results.push(json!({
                    "id": id,
                    "symbol": row.try_get::<String, _>(1).unwrap_or_default(),
                    "open_date": row.try_get::<String, _>(2).unwrap_or_default(),
                    "close_date": row.try_get::<String, _>(3).unwrap_or_default(),
                    "notes": row.try_get::<Option<String>, _>(4).unwrap_or(None),
                    "mistakes": row.try_get::<String, _>(5).unwrap_or_default(),
                    "entry_tactics": row.try_get::<String, _>(6).unwrap_or_default(),
                    "edges_spotted": row.try_get::<String, _>(7).unwrap_or_default(),
                    "tags": [],
                }));
            }

            // Batch-attach each entry's tags (one query). Legacy freeform fields
            // above are preserved for old trades (dual-read coexistence).
            let trade_tags = tags_table::tags_for_trades(db.pool(), &ids).await?;
            for (entry, row) in results.iter_mut().zip(ids.iter()) {
                if let Some(tags) = trade_tags.get(row) {
                    let tag_json = tags
                        .iter()
                        .map(|t| {
                            json!({
                                "name": t.tag.name,
                                "category": t.category_name,
                            })
                        })
                        .collect::<Vec<_>>();
                    entry["tags"] = json!(tag_json);
                }
            }
            Ok(serde_json::to_string(&results)?)
        }
        other => Ok(format!("Unknown entity type: {}", other)),
    }
}
