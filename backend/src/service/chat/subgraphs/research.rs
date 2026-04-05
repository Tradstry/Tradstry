use std::sync::Arc;

use langgraph::core::constants::END;
use langgraph::prelude::*;
use serde_json::{Value, json};

use crate::service::agents::client::AgentsClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;
use crate::service::chat::tools;
use crate::service::chat::types::{GroqFunctionDef, GroqToolDef};
use crate::service::turso::TursoClient;

// ---------------------------------------------------------------------------
// Shared dependencies for all research subgraph nodes
// ---------------------------------------------------------------------------
pub struct ResearchDeps {
    pub agents: Arc<AgentsClient>,
    pub turso: Arc<TursoClient>,
    pub qdrant: Arc<VectorDatabaseClient>,
    pub user_id: String,
    pub account_id: String,
}

// ---------------------------------------------------------------------------
// Tool schema: describes the "research" tool to the LLM
// ---------------------------------------------------------------------------
pub fn tool_schema() -> GroqToolDef {
    GroqToolDef {
        tool_type: "function".to_string(),
        function: GroqFunctionDef {
            name: "research".to_string(),
            description: "Run a comprehensive research pipeline on trading data: fetch trades, \
                          compute performance metrics, search for semantic patterns, and synthesize \
                          a thorough analysis."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural language research query."
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Optional ticker symbol to restrict research to."
                    },
                    "date_from": {
                        "type": "string",
                        "description": "Optional start date filter (ISO 8601)."
                    },
                    "date_to": {
                        "type": "string",
                        "description": "Optional end date filter (ISO 8601)."
                    }
                },
                "required": ["query"]
            }),
        },
    }
}

// ---------------------------------------------------------------------------
// Graph builder
// ---------------------------------------------------------------------------
pub fn build(deps: Arc<ResearchDeps>) -> Result<CompiledStateGraph, GraphError> {
    // --- Schema: all channels are LastValue ---
    let schema = StateSchema::new()
        .with_last_value("query")?
        .with_last_value("symbol")?
        .with_last_value("date_from")?
        .with_last_value("date_to")?
        .with_last_value("trades")?
        .with_last_value("metrics")?
        .with_last_value("patterns")?
        .with_last_value("synthesis")?
        .with_last_value("tool_call_id")?;

    let mut graph = StateGraph::<Value>::new(schema);

    // --- Node: fetch_trades ---
    let ft_deps = Arc::clone(&deps);
    graph.add_node(
        "fetch_trades",
        move |state: Value, _ctx: ExecutionContext<'_>| {
            let deps = Arc::clone(&ft_deps);
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
            rt.block_on(async move {
                let query = state
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let symbol = state
                    .get("symbol")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned());
                let date_from = state
                    .get("date_from")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned());
                let date_to = state
                    .get("date_to")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned());

                let mut filters = json!({});
                if let Some(sym) = &symbol {
                    filters["symbol"] = json!(sym);
                }
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
                .unwrap_or_else(|e| format!("fetch_trades error: {e}"));

                Ok::<NodeExecutionResult, anyhow::Error>(
                    NodeExecutionResult::default()
                        .with_write(ChannelWrite::new("trades", json!(trades)))
                        .with_write(ChannelWrite::new("query", json!(query))),
                )
            })
            .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        },
    )?;

    // --- Node: compute_metrics ---
    let cm_deps = Arc::clone(&deps);
    graph.add_node("compute_metrics", move |state: Value, _ctx: ExecutionContext<'_>| {
        let deps = Arc::clone(&cm_deps);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
        rt.block_on(async move {
            let symbol = state.get("symbol").and_then(|v| v.as_str()).map(|s| s.to_owned());
            let date_from = state.get("date_from").and_then(|v| v.as_str()).map(|s| s.to_owned());
            let date_to = state.get("date_to").and_then(|v| v.as_str()).map(|s| s.to_owned());

            let mut filters = json!({});
            if let Some(sym) = &symbol {
                filters["symbol"] = json!(sym);
            }
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
            .unwrap_or_else(|e| format!("compute_metrics error: {e}"));

            Ok::<NodeExecutionResult, anyhow::Error>(
                NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("metrics", json!(metrics))),
            )
        })
        .map_err(|e| NodeExecutionError::fatal(e.to_string()))
    })?;

    // --- Node: search_patterns ---
    let sp_deps = Arc::clone(&deps);
    graph.add_node(
        "search_patterns",
        move |state: Value, _ctx: ExecutionContext<'_>| {
            let deps = Arc::clone(&sp_deps);
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
            rt.block_on(async move {
                let query = state
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let date_from = state
                    .get("date_from")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned());
                let date_to = state
                    .get("date_to")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned());

                let mut args_obj = json!({ "query": query });
                if let Some(from) = &date_from {
                    args_obj["date_from"] = json!(from);
                }
                if let Some(to) = &date_to {
                    args_obj["date_to"] = json!(to);
                }

                let arguments = serde_json::to_string(&args_obj)
                    .unwrap_or_else(|_| format!(r#"{{"query":"{}"}}"#, query));

                let patterns = tools::semantic_search::execute(
                    &arguments,
                    &deps.user_id,
                    &deps.account_id,
                    &deps.qdrant,
                )
                .await
                .unwrap_or_else(|e| format!("search_patterns error: {e}"));

                Ok::<NodeExecutionResult, anyhow::Error>(
                    NodeExecutionResult::default()
                        .with_write(ChannelWrite::new("patterns", json!(patterns))),
                )
            })
            .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        },
    )?;

    // --- Node: synthesize ---
    let sy_deps = Arc::clone(&deps);
    graph.add_node("synthesize", move |state: Value, _ctx: ExecutionContext<'_>| {
        let deps = Arc::clone(&sy_deps);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
        rt.block_on(async move {
            let query = state.get("query").and_then(|v| v.as_str()).unwrap_or("").to_owned();
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
            let patterns = state
                .get("patterns")
                .and_then(|v| v.as_str())
                .unwrap_or("[]")
                .to_owned();

            // Truncate inputs to avoid blowing up token budget
            let trades_trunc = &trades[..trades.len().min(1500)];
            let metrics_trunc = &metrics[..metrics.len().min(500)];
            let patterns_trunc = &patterns[..patterns.len().min(500)];

            let synthesis_prompt = format!(
                "You are a trading analyst. Write a CONCISE analysis (under 300 words) based on this data.\n\n\
                 Query: {query}\n\n\
                 Trades:\n{trades_trunc}\n\n\
                 Metrics:\n{metrics_trunc}\n\n\
                 Patterns:\n{patterns_trunc}\n\n\
                 Cover: what happened, performance summary, patterns worth noting, and 2-3 actionable takeaways. \
                 Be specific with numbers. No markdown tables. No bold/italic. Keep it short."
            );

            let synthesis = deps
                .agents
                .prompt(&synthesis_prompt)
                .await
                .unwrap_or_else(|e| format!("synthesize error: {e}"));

            Ok::<NodeExecutionResult, anyhow::Error>(
                NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("synthesis", json!(synthesis))),
            )
        })
        .map_err(|e| NodeExecutionError::fatal(e.to_string()))
    })?;

    // --- Entry point and edges ---
    graph.set_entry_point("fetch_trades")?;
    graph.add_edge("fetch_trades", "compute_metrics")?;
    graph.add_edge("compute_metrics", "search_patterns")?;
    graph.add_edge("search_patterns", "synthesize")?;
    graph.add_edge("synthesize", END)?;

    // --- Compile ---
    graph.compile()
}
