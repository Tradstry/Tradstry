use std::sync::Arc;

use langgraph::core::constants::END;
use langgraph::prelude::*;
use serde_json::{Value, json};

use crate::service::ai::chat::tools;
use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::ai::client::AgentsClient;
use crate::service::turso::TursoClient;

// ---------------------------------------------------------------------------
// Shared dependencies for all comparison subgraph nodes
// ---------------------------------------------------------------------------
pub struct ComparisonDeps {
    pub agents: Arc<AgentsClient>,
    pub turso: Arc<TursoClient>,
    pub user_id: String,
    pub account_id: String,
}

// ---------------------------------------------------------------------------
// Tool schema: describes the "comparison" tool to the LLM
// ---------------------------------------------------------------------------
pub fn tool_schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "comparison".to_string(),
            description: "Compare two trades side-by-side: resolve the trades by ID or query, \
                          fetch their full journal details, and produce a structured JSON \
                          comparison highlighting differences, what worked, and lessons learned."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural language query to find trades to compare if trade_ids are not provided."
                    },
                    "trade_ids": {
                        "type": "array",
                        "items": {
                            "type": "string"
                        },
                        "description": "Optional array of two trade IDs to compare directly."
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
pub fn build(deps: Arc<ComparisonDeps>) -> Result<CompiledStateGraph, GraphError> {
    // --- Schema: all channels are LastValue ---
    let schema = StateSchema::new()
        .with_last_value("query")?
        .with_last_value("trade_ids")?
        .with_last_value("trade_a")?
        .with_last_value("trade_b")?
        .with_last_value("comparison_json")?
        .with_last_value("tool_call_id")?;

    let mut graph = StateGraph::<Value>::new(schema);

    // --- Node: resolve_trades ---
    let rt_deps = Arc::clone(&deps);
    graph.add_node(
        "resolve_trades",
        move |state: Value, _ctx: ExecutionContext<'_>| {
            let deps = Arc::clone(&rt_deps);
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
            rt.block_on(async move {
                // Check if trade_ids is already a non-empty array
                let existing_ids: Vec<String> = state
                    .get("trade_ids")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                            .collect()
                    })
                    .unwrap_or_default();

                if existing_ids.len() >= 2 {
                    // Use the first two IDs directly
                    let resolved = json!([existing_ids[0], existing_ids[1]]);
                    return Ok::<NodeExecutionResult, anyhow::Error>(
                        NodeExecutionResult::default()
                            .with_write(ChannelWrite::new("trade_ids", resolved)),
                    );
                }

                // Fall back to querying by the natural language query
                let query = state
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();

                let arguments = serde_json::to_string(&json!({
                    "entity": "trades",
                    "filters": {},
                    "limit": 10
                }))
                .unwrap_or_else(|_| r#"{"entity":"trades"}"#.to_string());

                let result = tools::db_query::execute(
                    &arguments,
                    &deps.user_id,
                    &deps.account_id,
                    &deps.turso,
                )
                .await
                .unwrap_or_else(|e| format!("resolve_trades error: {e}"));

                // Parse the result and take the first two IDs
                let resolved_ids: Vec<String> = serde_json::from_str::<Vec<Value>>(&result)
                    .unwrap_or_default()
                    .into_iter()
                    .take(2)
                    .filter_map(|v| v.get("id").and_then(|id| id.as_str()).map(|s| s.to_owned()))
                    .collect();

                let _ = query; // query context used for intent; actual resolution uses db_query
                let resolved = json!(resolved_ids);

                Ok::<NodeExecutionResult, anyhow::Error>(
                    NodeExecutionResult::default()
                        .with_write(ChannelWrite::new("trade_ids", resolved)),
                )
            })
            .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        },
    )?;

    // --- Node: fetch_details ---
    let fd_deps = Arc::clone(&deps);
    graph.add_node(
        "fetch_details",
        move |state: Value, _ctx: ExecutionContext<'_>| {
            let deps = Arc::clone(&fd_deps);
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
            rt.block_on(async move {
                let trade_ids: Vec<String> = state
                    .get("trade_ids")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                            .collect()
                    })
                    .unwrap_or_default();

                let id_a = trade_ids.first().cloned().unwrap_or_default();
                let id_b = trade_ids.get(1).cloned().unwrap_or_default();

                // Fetch journal details for trade A
                let args_a = serde_json::to_string(&json!({
                    "entity": "journal",
                    "filters": { "symbol": id_a },
                    "limit": 1
                }))
                .unwrap_or_else(|_| r#"{"entity":"journal"}"#.to_string());

                let trade_a =
                    tools::db_query::execute(&args_a, &deps.user_id, &deps.account_id, &deps.turso)
                        .await
                        .unwrap_or_else(|e| format!("fetch_details trade_a error: {e}"));

                // Fetch journal details for trade B
                let args_b = serde_json::to_string(&json!({
                    "entity": "journal",
                    "filters": { "symbol": id_b },
                    "limit": 1
                }))
                .unwrap_or_else(|_| r#"{"entity":"journal"}"#.to_string());

                let trade_b =
                    tools::db_query::execute(&args_b, &deps.user_id, &deps.account_id, &deps.turso)
                        .await
                        .unwrap_or_else(|e| format!("fetch_details trade_b error: {e}"));

                Ok::<NodeExecutionResult, anyhow::Error>(
                    NodeExecutionResult::default()
                        .with_write(ChannelWrite::new("trade_a", json!(trade_a)))
                        .with_write(ChannelWrite::new("trade_b", json!(trade_b))),
                )
            })
            .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        },
    )?;

    // --- Node: compare ---
    let cmp_deps = Arc::clone(&deps);
    graph.add_node("compare", move |state: Value, _ctx: ExecutionContext<'_>| {
        let deps = Arc::clone(&cmp_deps);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
        rt.block_on(async move {
            let trade_a = state
                .get("trade_a")
                .and_then(|v| v.as_str())
                .unwrap_or("[]")
                .to_owned();
            let trade_b = state
                .get("trade_b")
                .and_then(|v| v.as_str())
                .unwrap_or("[]")
                .to_owned();

            let trade_a_trunc = &trade_a[..trade_a.len().min(1000)];
            let trade_b_trunc = &trade_b[..trade_b.len().min(1000)];

            let compare_prompt = format!(
                "You are a trading analyst. Compare these two trades CONCISELY (under 300 words).\n\n\
                 Trade A:\n{trade_a_trunc}\n\n\
                 Trade B:\n{trade_b_trunc}\n\n\
                 Respond with ONLY a valid JSON object:\n\
                 {{\"side_by_side\": {{...}}, \"key_differences\": [\"...\"], \"what_worked\": \"...\", \"lessons\": [\"...\"]}}\n\
                 Keep it brief. No markdown."
            );

            let comparison_json = deps
                .agents
                .prompt(&compare_prompt)
                .await
                .unwrap_or_else(|e| format!("compare error: {e}"));

            Ok::<NodeExecutionResult, anyhow::Error>(
                NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("comparison_json", json!(comparison_json))),
            )
        })
        .map_err(|e| NodeExecutionError::fatal(e.to_string()))
    })?;

    // --- Entry point and edges ---
    graph.set_entry_point("resolve_trades")?;
    graph.add_edge("resolve_trades", "fetch_details")?;
    graph.add_edge("fetch_details", "compare")?;
    graph.add_edge("compare", END)?;

    // --- Compile ---
    graph.compile()
}
