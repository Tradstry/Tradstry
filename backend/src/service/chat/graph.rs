use std::sync::Arc;

use langgraph::prelude::*;
use log::info;
use serde_json::{json, Value};

use crate::service::agents::client::AgentsClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;
use crate::service::chat::tools;
use crate::service::chat::types::*;
use crate::service::turso::TursoClient;

// ---------------------------------------------------------------------------
// Shared dependencies passed into every node closure
// ---------------------------------------------------------------------------
pub struct GraphDeps {
    pub agents: Arc<AgentsClient>,
    pub turso: Arc<TursoClient>,
    pub qdrant: Arc<VectorDatabaseClient>,
    pub tx: ChatEventBus,
    pub job_id: String,
    pub session_id: String,
    pub user_id: String,
    pub account_id: String,
    pub system_prompt: String,
}

// ---------------------------------------------------------------------------
// Graph builder
// ---------------------------------------------------------------------------
pub fn build_chat_graph(
    deps: Arc<GraphDeps>,
) -> Result<CompiledStateGraph, GraphError> {
    // --- Schema: 3 channels ---
    let schema = StateSchema::new()
        .with_topic("messages", true)?          // accumulate
        .with_last_value("current_tool_call")?  // last-value
        .with_last_value("iteration")?;         // last-value

    let mut graph = StateGraph::<Value>::new(schema);

    // --- Node: llm ---
    let llm_deps = Arc::clone(&deps);
    graph.add_node("llm", move |state: Value, _ctx: ExecutionContext<'_>| {
        let deps = Arc::clone(&llm_deps);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
        rt.block_on(async move {
            llm_node_async(&deps, &state).await
        }).map_err(|e| NodeExecutionError::fatal(e.to_string()))
    })?;

    // --- Node: db_query ---
    let db_deps = Arc::clone(&deps);
    graph.add_node("db_query", move |state: Value, _ctx: ExecutionContext<'_>| {
        let deps = Arc::clone(&db_deps);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
        rt.block_on(async move {
            tool_node_async(&deps, &state, "db_query").await
        }).map_err(|e| NodeExecutionError::fatal(e.to_string()))
    })?;

    // --- Node: semantic_search ---
    let ss_deps = Arc::clone(&deps);
    graph.add_node("semantic_search", move |state: Value, _ctx: ExecutionContext<'_>| {
        let deps = Arc::clone(&ss_deps);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
        rt.block_on(async move {
            tool_node_async(&deps, &state, "semantic_search").await
        }).map_err(|e| NodeExecutionError::fatal(e.to_string()))
    })?;

    // --- Node: analytics_calc ---
    let ac_deps = Arc::clone(&deps);
    graph.add_node("analytics_calc", move |state: Value, _ctx: ExecutionContext<'_>| {
        let deps = Arc::clone(&ac_deps);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
        rt.block_on(async move {
            tool_node_async(&deps, &state, "analytics_calc").await
        }).map_err(|e| NodeExecutionError::fatal(e.to_string()))
    })?;

    // --- Node: recall_memory ---
    let rm_deps = Arc::clone(&deps);
    graph.add_node("recall_memory", move |state: Value, _ctx: ExecutionContext<'_>| {
        let deps = Arc::clone(&rm_deps);
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| NodeExecutionError::fatal(format!("Failed to create runtime: {e}")))?;
        rt.block_on(async move {
            tool_node_async(&deps, &state, "recall_memory").await
        }).map_err(|e| NodeExecutionError::fatal(e.to_string()))
    })?;

    // --- Entry point ---
    graph.set_entry_point("llm")?;

    // --- Conditional edges: llm -> tool nodes or END ---
    graph.add_conditional_edges(
        "llm",
        |state: Value, _result: &NodeExecutionResult| {
            let tool_call = state.get("current_tool_call").cloned().unwrap_or(Value::Null);

            if tool_call.is_null() {
                // No tool call -- LLM produced a text response, finish.
                return Ok(vec![BranchTarget::End]);
            }

            let tool_name = tool_call
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            match tool_name {
                "db_query" => Ok(vec![BranchTarget::Node("db_query".to_owned())]),
                "semantic_search" => Ok(vec![BranchTarget::Node("semantic_search".to_owned())]),
                "analytics_calc" => Ok(vec![BranchTarget::Node("analytics_calc".to_owned())]),
                "recall_memory" => Ok(vec![BranchTarget::Node("recall_memory".to_owned())]),
                _ => {
                    // Unknown tool -- end the graph
                    Ok(vec![BranchTarget::End])
                }
            }
        },
        None,
    )?;

    // --- Tool nodes route back to llm ---
    graph.add_edge("db_query", "llm")?;
    graph.add_edge("semantic_search", "llm")?;
    graph.add_edge("analytics_calc", "llm")?;
    graph.add_edge("recall_memory", "llm")?;

    // --- Compile ---
    graph.compile()
}

// ---------------------------------------------------------------------------
// Run helper
// ---------------------------------------------------------------------------
pub fn run_chat_graph(
    compiled: &CompiledStateGraph,
    saver: &dyn CheckpointSaver,
    session_id: &str,
    user_message: Value,
) -> Result<LoopRunSummary, GraphError> {
    let config = LoopConfig::new(CheckpointConfig::new(session_id))
        .with_recursion_limit(12); // up to 5 tool calls + safety margin

    let input = json!({
        "messages": [user_message],
        "current_tool_call": Value::Null,
        "iteration": 0,
    });

    compiled.run_raw(Some(saver), config, input)
}

// ---------------------------------------------------------------------------
// Async node implementations
// ---------------------------------------------------------------------------

/// LLM node: builds Groq messages from state, calls stream_chat, and produces
/// channel writes for either a tool call or a final text response.
async fn llm_node_async(
    deps: &GraphDeps,
    state: &Value,
) -> Result<NodeExecutionResult, anyhow::Error> {
    // Reconstruct Groq messages from the "messages" channel (accumulated array).
    let messages_val = state.get("messages").cloned().unwrap_or(json!([]));
    let raw_messages: Vec<Value> = match messages_val {
        Value::Array(arr) => arr,
        _ => vec![messages_val],
    };

    let mut groq_messages: Vec<GroqMessage> = vec![GroqMessage {
        role: "system".to_owned(),
        content: Some(deps.system_prompt.clone()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];

    for msg_val in &raw_messages {
        if let Ok(msg) = serde_json::from_value::<GroqMessage>(msg_val.clone()) {
            groq_messages.push(msg);
        }
    }

    // Determine iteration count
    let iteration = state
        .get("iteration")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    // If we've hit 5 iterations, call without tools to force a final answer.
    let tool_defs = tools::tool_schemas();
    let tools_param = if iteration >= 5 {
        None
    } else {
        Some(tool_defs.as_slice())
    };

    let response = deps
        .agents
        .stream_chat(
            &groq_messages,
            tools_param,
            deps.tx.clone(),
            &deps.job_id,
            &deps.session_id,
        )
        .await?;

    match response {
        GroqChatResponse::ToolCall { id, name, arguments } => {
            info!(
                "LLM node: tool_call {} (iteration {})",
                name, iteration
            );

            // Write the assistant tool-call message into the messages channel
            let assistant_msg = serde_json::to_value(GroqMessage {
                role: "assistant".to_owned(),
                content: None,
                tool_calls: Some(vec![GroqToolCall {
                    id: id.clone(),
                    call_type: "function".to_owned(),
                    function: GroqFunctionCall {
                        name: name.clone(),
                        arguments: arguments.clone(),
                    },
                }]),
                tool_call_id: None,
                name: None,
            })?;

            let tool_call_info = json!({
                "id": id,
                "name": name,
                "arguments": arguments,
            });

            Ok(NodeExecutionResult::default()
                .with_write(ChannelWrite::new("messages", assistant_msg))
                .with_write(ChannelWrite::new("current_tool_call", tool_call_info))
                .with_write(ChannelWrite::new("iteration", json!(iteration + 1))))
        }

        GroqChatResponse::TextComplete { full_text } => {
            info!("LLM node: text complete (iteration {})", iteration);

            let assistant_msg = serde_json::to_value(GroqMessage {
                role: "assistant".to_owned(),
                content: Some(full_text.clone()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            })?;

            // Broadcast Done event (message lives in checkpoint, not SQLite)
            let msg_id = uuid::Uuid::new_v4().to_string();
            let _ = deps.tx.send(ChatStreamEnvelope {
                job_id: deps.job_id.clone(),
                session_id: deps.session_id.clone(),
                kind: ChatStreamKind::Done,
                content: None,
                tool_name: None,
                message_id: Some(msg_id),
            });

            // Clear current_tool_call to signal END via conditional routing
            Ok(NodeExecutionResult::default()
                .with_write(ChannelWrite::new("messages", assistant_msg))
                .with_write(ChannelWrite::new("current_tool_call", Value::Null)))
        }
    }
}

/// Tool node: reads current_tool_call from state, executes the tool,
/// broadcasts events, and writes the tool result back into messages.
async fn tool_node_async(
    deps: &GraphDeps,
    state: &Value,
    expected_tool: &str,
) -> Result<NodeExecutionResult, anyhow::Error> {
    let tool_call = state
        .get("current_tool_call")
        .cloned()
        .unwrap_or(Value::Null);

    let tool_id = tool_call
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    let tool_name = tool_call
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(expected_tool)
        .to_owned();
    let arguments = tool_call
        .get("arguments")
        .and_then(|v| v.as_str())
        .unwrap_or("{}")
        .to_owned();

    // Broadcast ToolStart
    let _ = deps.tx.send(ChatStreamEnvelope {
        job_id: deps.job_id.clone(),
        session_id: deps.session_id.clone(),
        kind: ChatStreamKind::ToolStart,
        content: None,
        tool_name: Some(tool_name.clone()),
        message_id: None,
    });

    // Execute
    let result = tools::execute_tool(
        &tool_name,
        &arguments,
        &deps.user_id,
        &deps.account_id,
        &deps.turso,
        &deps.qdrant,
    )
    .await
    .unwrap_or_else(|e| format!("Tool error: {e}"));

    // Broadcast ToolResult
    let _ = deps.tx.send(ChatStreamEnvelope {
        job_id: deps.job_id.clone(),
        session_id: deps.session_id.clone(),
        kind: ChatStreamKind::ToolResult,
        content: Some(result.clone()),
        tool_name: Some(tool_name.clone()),
        message_id: None,
    });

    // Write tool result message into messages channel
    let tool_msg = serde_json::to_value(GroqMessage {
        role: "tool".to_owned(),
        content: Some(result),
        tool_calls: None,
        tool_call_id: Some(tool_id),
        name: Some(tool_name),
    })?;

    // Clear current_tool_call so the next llm iteration starts fresh
    Ok(NodeExecutionResult::default()
        .with_write(ChannelWrite::new("messages", tool_msg))
        .with_write(ChannelWrite::new("current_tool_call", Value::Null)))
}
