use std::sync::Arc;

use langgraph::core::constants::END;
use langgraph::prelude::*;
use serde_json::{Value, json};

use crate::service::ai::chat::tools;
use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::ai::client::AgentsClient;
use crate::service::ai::vector_database::client::VectorDatabaseClient;
use crate::service::turso::TursoClient;

// ---------------------------------------------------------------------------
// Shared dependencies for all report subgraph nodes
// ---------------------------------------------------------------------------
pub struct ReportDeps {
    pub agents: Arc<AgentsClient>,
    pub turso: Arc<TursoClient>,
    pub qdrant: Arc<VectorDatabaseClient>,
    pub user_id: String,
    pub account_id: String,
}

// ---------------------------------------------------------------------------
// Tool schema: describes the "report" tool to the LLM
// ---------------------------------------------------------------------------
pub fn tool_schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "report".to_string(),
            description: "Generate a comprehensive trading report for a date range: fetch all \
                          trades, compute performance metrics, identify recurring mistakes, and \
                          produce a structured JSON report with overview, per-symbol breakdown, \
                          mistakes, and next actions."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "date_from": {
                        "type": "string",
                        "description": "Report start date (ISO 8601)."
                    },
                    "date_to": {
                        "type": "string",
                        "description": "Report end date (ISO 8601)."
                    }
                },
                "required": ["date_from", "date_to"]
            }),
        },
    }
}

// ---------------------------------------------------------------------------
// Graph builder
// ---------------------------------------------------------------------------
pub fn build(deps: Arc<ReportDeps>) -> Result<CompiledStateGraph, GraphError> {
    // --- Schema: all channels are LastValue ---
    let schema = StateSchema::new()
        .with_last_value("date_from")?
        .with_last_value("date_to")?
        .with_last_value("trades")?
        .with_last_value("metrics")?
        .with_last_value("mistakes")?
        .with_last_value("report_json")?
        .with_last_value("tool_call_id")?;

    let mut graph = StateGraph::<Value>::new(schema);

    // --- Node: fetch_all_trades ---
    let fat_deps = Arc::clone(&deps);
    graph.add_node(
        "fetch_all_trades",
        move |state: Value, _ctx: ExecutionContext<'_>| {
            let deps = Arc::clone(&fat_deps);
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
            rt.block_on(async move {
                let date_from = state
                    .get("date_from")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned());
                let date_to = state
                    .get("date_to")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned());

                let mut filters = json!({});
                if let Some(from) = &date_from {
                    filters["date_from"] = json!(from);
                }
                if let Some(to) = &date_to {
                    filters["date_to"] = json!(to);
                }

                let arguments = serde_json::to_string(&json!({
                    "entity": "trades",
                    "filters": filters,
                    "limit": 50
                }))
                .unwrap_or_else(|_| r#"{"entity":"trades"}"#.to_string());

                let trades = tools::db_query::execute(
                    &arguments,
                    &deps.user_id,
                    &deps.account_id,
                    &deps.turso,
                )
                .await
                .unwrap_or_else(|e| format!("fetch_all_trades error: {e}"));

                Ok::<NodeExecutionResult, anyhow::Error>(
                    NodeExecutionResult::default()
                        .with_write(ChannelWrite::new("trades", json!(trades))),
                )
            })
            .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        },
    )?;

    // --- Node: compute_all_metrics ---
    let cam_deps = Arc::clone(&deps);
    graph.add_node("compute_all_metrics", move |state: Value, _ctx: ExecutionContext<'_>| {
        let deps = Arc::clone(&cam_deps);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
        rt.block_on(async move {
            let date_from = state.get("date_from").and_then(|v| v.as_str()).map(|s| s.to_owned());
            let date_to = state.get("date_to").and_then(|v| v.as_str()).map(|s| s.to_owned());

            let mut filters = json!({});
            if let Some(from) = &date_from {
                filters["date_from"] = json!(from);
            }
            if let Some(to) = &date_to {
                filters["date_to"] = json!(to);
            }

            let arguments = serde_json::to_string(&json!({
                "metrics": ["win_rate", "total_pnl", "avg_r", "profit_factor", "streak", "per_symbol"],
                "filters": filters
            }))
            .unwrap_or_else(|_| r#"{"metrics":["win_rate","total_pnl"]}"#.to_string());

            let metrics = tools::analytics_calc::execute(
                &arguments,
                &deps.user_id,
                &deps.account_id,
                &deps.turso,
            )
            .await
            .unwrap_or_else(|e| format!("compute_all_metrics error: {e}"));

            Ok::<NodeExecutionResult, anyhow::Error>(
                NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("metrics", json!(metrics))),
            )
        })
        .map_err(|e| NodeExecutionError::fatal(e.to_string()))
    })?;

    // --- Node: find_mistakes ---
    let fm_deps = Arc::clone(&deps);
    graph.add_node(
        "find_mistakes",
        move |state: Value, _ctx: ExecutionContext<'_>| {
            let deps = Arc::clone(&fm_deps);
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
            rt.block_on(async move {
                let date_from = state
                    .get("date_from")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned());
                let date_to = state
                    .get("date_to")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned());

                let mut args_obj = json!({ "query": "recurring mistakes and discipline issues" });
                if let Some(from) = &date_from {
                    args_obj["date_from"] = json!(from);
                }
                if let Some(to) = &date_to {
                    args_obj["date_to"] = json!(to);
                }

                let arguments = serde_json::to_string(&args_obj).unwrap_or_else(|_| {
                    r#"{"query":"recurring mistakes and discipline issues"}"#.to_string()
                });

                let mistakes = tools::semantic_search::execute(
                    &arguments,
                    &deps.user_id,
                    &deps.account_id,
                    &deps.qdrant,
                )
                .await
                .unwrap_or_else(|e| format!("find_mistakes error: {e}"));

                Ok::<NodeExecutionResult, anyhow::Error>(
                    NodeExecutionResult::default()
                        .with_write(ChannelWrite::new("mistakes", json!(mistakes))),
                )
            })
            .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        },
    )?;

    // --- Node: build_report ---
    let br_deps = Arc::clone(&deps);
    graph.add_node("build_report", move |state: Value, _ctx: ExecutionContext<'_>| {
        let deps = Arc::clone(&br_deps);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
        rt.block_on(async move {
            let date_from = state.get("date_from").and_then(|v| v.as_str()).unwrap_or("").to_owned();
            let date_to = state.get("date_to").and_then(|v| v.as_str()).unwrap_or("").to_owned();
            let trades = state
                .get("trades")
                .and_then(|v| v.as_str())
                .unwrap_or("[]")
                .to_owned();
            let metrics = state
                .get("metrics")
                .and_then(|v| v.as_str())
                .unwrap_or("{}")
                .to_owned();
            let mistakes = state
                .get("mistakes")
                .and_then(|v| v.as_str())
                .unwrap_or("[]")
                .to_owned();

            let trades_trunc = &trades[..trades.len().min(1500)];
            let metrics_trunc = &metrics[..metrics.len().min(500)];
            let mistakes_trunc = &mistakes[..mistakes.len().min(500)];

            let report_prompt = format!(
                "You are a trading analyst. Generate a CONCISE structured report (under 400 words) for {date_from} to {date_to}.\n\n\
                 Trades:\n{trades_trunc}\n\n\
                 Metrics:\n{metrics_trunc}\n\n\
                 Mistakes:\n{mistakes_trunc}\n\n\
                 Respond with ONLY a valid JSON object:\n\
                 {{\"overview\": \"2-3 sentences\", \"performance_metrics\": {{...}}, \"per_symbol_breakdown\": [{{...}}], \"mistakes_and_patterns\": [\"...\"], \"next_actions\": [\"...\"]}}\n\
                 Keep each section brief. No markdown."
            );

            let report_json = deps
                .agents
                .prompt(&report_prompt)
                .await
                .unwrap_or_else(|e| format!("build_report error: {e}"));

            Ok::<NodeExecutionResult, anyhow::Error>(
                NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("report_json", json!(report_json))),
            )
        })
        .map_err(|e| NodeExecutionError::fatal(e.to_string()))
    })?;

    // --- Entry point and edges ---
    graph.set_entry_point("fetch_all_trades")?;
    graph.add_edge("fetch_all_trades", "compute_all_metrics")?;
    graph.add_edge("compute_all_metrics", "find_mistakes")?;
    graph.add_edge("find_mistakes", "build_report")?;
    graph.add_edge("build_report", END)?;

    // --- Compile ---
    graph.compile()
}
