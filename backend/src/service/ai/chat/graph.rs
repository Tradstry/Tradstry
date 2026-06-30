use std::sync::Arc;

use langgraph::prelude::*;
use log::info;
use serde_json::{Value, json};

use crate::service::ai::chat::tools;
use crate::service::ai::chat::types::*;
use crate::service::ai::client::AgentsClient;
use crate::service::ai::vector_database::client::VectorDatabaseClient;
use crate::service::db::Db;
use crate::service::r2::R2Client;

// ---------------------------------------------------------------------------
// Shared dependencies passed into every node closure
// ---------------------------------------------------------------------------
pub struct GraphDeps {
    pub agents: Arc<AgentsClient>,
    pub db: Arc<Db>,
    pub qdrant: Arc<VectorDatabaseClient>,
    pub r2: Arc<R2Client>,
    pub tx: ChatStreamTx,
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
    checkpoint_saver: Option<Arc<dyn CheckpointSaver>>,
) -> Result<CompiledStateGraph, GraphError> {
    // --- Schema: 3 channels ---
    let schema = StateSchema::new()
        .with_topic("messages", true)? // accumulate
        .with_last_value("current_tool_call")? // last-value
        .with_last_value("iteration")?; // last-value

    let mut graph = StateGraph::<Value>::new(schema);

    // --- Node: llm ---
    let llm_deps = Arc::clone(&deps);
    graph.add_node("llm", move |state: Value, _ctx: ExecutionContext| {
        let deps = Arc::clone(&llm_deps);
        async move {
            llm_node_async(&deps, &state)
                .await
                .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        }
    })?;

    // --- Node: db_query ---
    let db_deps = Arc::clone(&deps);
    graph.add_node("db_query", move |state: Value, _ctx: ExecutionContext| {
        let deps = Arc::clone(&db_deps);
        async move {
            tool_node_async(&deps, &state, "db_query")
                .await
                .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        }
    })?;

    // --- Node: semantic_search ---
    let ss_deps = Arc::clone(&deps);
    graph.add_node(
        "semantic_search",
        move |state: Value, _ctx: ExecutionContext| {
            let deps = Arc::clone(&ss_deps);
            async move {
                tool_node_async(&deps, &state, "semantic_search")
                    .await
                    .map_err(|e| NodeExecutionError::fatal(e.to_string()))
            }
        },
    )?;

    // --- Node: analytics_calc ---
    let ac_deps = Arc::clone(&deps);
    graph.add_node(
        "analytics_calc",
        move |state: Value, _ctx: ExecutionContext| {
            let deps = Arc::clone(&ac_deps);
            async move {
                tool_node_async(&deps, &state, "analytics_calc")
                    .await
                    .map_err(|e| NodeExecutionError::fatal(e.to_string()))
            }
        },
    )?;

    // --- Node: recall_memory ---
    let rm_deps = Arc::clone(&deps);
    graph.add_node(
        "recall_memory",
        move |state: Value, _ctx: ExecutionContext| {
            let deps = Arc::clone(&rm_deps);
            async move {
                tool_node_async(&deps, &state, "recall_memory")
                    .await
                    .map_err(|e| NodeExecutionError::fatal(e.to_string()))
            }
        },
    )?;

    // --- Node: create_agent ---
    let ca_deps = Arc::clone(&deps);
    graph.add_node(
        "create_agent",
        move |state: Value, _ctx: ExecutionContext| {
            let deps = Arc::clone(&ca_deps);
            async move {
                tool_node_async(&deps, &state, "create_agent")
                    .await
                    .map_err(|e| NodeExecutionError::fatal(e.to_string()))
            }
        },
    )?;

    // --- Node: save_agent ---
    let sa_deps = Arc::clone(&deps);
    graph.add_node("save_agent", move |state: Value, _ctx: ExecutionContext| {
        let deps = Arc::clone(&sa_deps);
        async move {
            tool_node_async(&deps, &state, "save_agent")
                .await
                .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        }
    })?;

    // --- Node: run_agent ---
    let ra_deps = Arc::clone(&deps);
    graph.add_node("run_agent", move |state: Value, _ctx: ExecutionContext| {
        let deps = Arc::clone(&ra_deps);
        async move {
            tool_node_async(&deps, &state, "run_agent")
                .await
                .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        }
    })?;

    // --- Node: edit_agent ---
    let ea_deps = Arc::clone(&deps);
    graph.add_node("edit_agent", move |state: Value, _ctx: ExecutionContext| {
        let deps = Arc::clone(&ea_deps);
        async move {
            tool_node_async(&deps, &state, "edit_agent")
                .await
                .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        }
    })?;

    // --- Node: stock_quote ---
    let sq_deps = Arc::clone(&deps);
    graph.add_node(
        "stock_quote",
        move |state: Value, _ctx: ExecutionContext| {
            let deps = Arc::clone(&sq_deps);
            async move {
                tool_node_async(&deps, &state, "stock_quote")
                    .await
                    .map_err(|e| NodeExecutionError::fatal(e.to_string()))
            }
        },
    )?;

    // --- Node: stock_news ---
    let sn_deps = Arc::clone(&deps);
    graph.add_node("stock_news", move |state: Value, _ctx: ExecutionContext| {
        let deps = Arc::clone(&sn_deps);
        async move {
            tool_node_async(&deps, &state, "stock_news")
                .await
                .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        }
    })?;

    // --- Node: financials ---
    let fi_deps = Arc::clone(&deps);
    graph.add_node("financials", move |state: Value, _ctx: ExecutionContext| {
        let deps = Arc::clone(&fi_deps);
        async move {
            tool_node_async(&deps, &state, "financials")
                .await
                .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        }
    })?;

    // --- Node: earnings ---
    let er_deps = Arc::clone(&deps);
    graph.add_node("earnings", move |state: Value, _ctx: ExecutionContext| {
        let deps = Arc::clone(&er_deps);
        async move {
            tool_node_async(&deps, &state, "earnings")
                .await
                .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        }
    })?;

    // --- Node: company_info ---
    let ci_deps = Arc::clone(&deps);
    graph.add_node(
        "company_info",
        move |state: Value, _ctx: ExecutionContext| {
            let deps = Arc::clone(&ci_deps);
            async move {
                tool_node_async(&deps, &state, "company_info")
                    .await
                    .map_err(|e| NodeExecutionError::fatal(e.to_string()))
            }
        },
    )?;

    // --- Node: get_notebook ---
    let gn_deps = Arc::clone(&deps);
    graph.add_node(
        "get_notebook",
        move |state: Value, _ctx: ExecutionContext| {
            let deps = Arc::clone(&gn_deps);
            async move {
                tool_node_async(&deps, &state, "get_notebook")
                    .await
                    .map_err(|e| NodeExecutionError::fatal(e.to_string()))
            }
        },
    )?;

    // --- Node: get_playbook ---
    let gp_deps = Arc::clone(&deps);
    graph.add_node(
        "get_playbook",
        move |state: Value, _ctx: ExecutionContext| {
            let deps = Arc::clone(&gp_deps);
            async move {
                tool_node_async(&deps, &state, "get_playbook")
                    .await
                    .map_err(|e| NodeExecutionError::fatal(e.to_string()))
            }
        },
    )?;

    // --- Node: view_media ---
    let vm_deps = Arc::clone(&deps);
    graph.add_node("view_media", move |state: Value, _ctx: ExecutionContext| {
        let deps = Arc::clone(&vm_deps);
        async move {
            tool_node_async(&deps, &state, "view_media")
                .await
                .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        }
    })?;

    // --- Subgraph: research ---
    {
        let research_deps = Arc::new(
            crate::service::ai::chat::subgraphs::research::ResearchDeps {
                agents: Arc::clone(&deps.agents),
                db: Arc::clone(&deps.db),
                qdrant: Arc::clone(&deps.qdrant),
                user_id: deps.user_id.clone(),
                account_id: deps.account_id.clone(),
            },
        );
        let research_child = crate::service::ai::chat::subgraphs::research::build(research_deps)
            .map_err(|e| {
                GraphError::validation(format!("Failed to build research subgraph: {e:?}"))
            })?;

        let research_config = SubgraphConfig {
            input_mapping: Arc::new(|parent_state: Value| {
                let tool_call = parent_state
                    .get("current_tool_call")
                    .cloned()
                    .unwrap_or(Value::Null);
                let tool_call_id = tool_call
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let args_str = tool_call
                    .get("arguments")
                    .and_then(|v| v.as_str())
                    .unwrap_or("{}");
                let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
                json!({
                    "query": args.get("query").cloned().unwrap_or(json!("")),
                    "symbol": args.get("symbol").cloned().unwrap_or(Value::Null),
                    "date_from": args.get("date_from").cloned().unwrap_or(Value::Null),
                    "date_to": args.get("date_to").cloned().unwrap_or(Value::Null),
                    "tool_call_id": tool_call_id,
                })
            }),
            output_mapping: Arc::new(|child_state: Value| {
                let synthesis = child_state
                    .get("synthesis")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let tool_call_id = child_state
                    .get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let tool_msg = json!({
                    "role": "tool",
                    "content": synthesis,
                    "tool_call_id": tool_call_id,
                    "name": "research"
                });
                vec![
                    ChannelWrite::new("messages", tool_msg),
                    ChannelWrite::new("current_tool_call", Value::Null),
                ]
            }),
            checkpoint_ns: Some("subgraph:research".to_string()),
            recursion_limit: None,
        };
        graph.add_subgraph(
            "research",
            research_child,
            research_config,
            checkpoint_saver.clone(),
        )?;
    }

    // --- Subgraph: report ---
    {
        let report_deps = Arc::new(crate::service::ai::chat::subgraphs::report::ReportDeps {
            agents: Arc::clone(&deps.agents),
            db: Arc::clone(&deps.db),
            qdrant: Arc::clone(&deps.qdrant),
            user_id: deps.user_id.clone(),
            account_id: deps.account_id.clone(),
        });
        let report_child = crate::service::ai::chat::subgraphs::report::build(report_deps)
            .map_err(|e| {
                GraphError::validation(format!("Failed to build report subgraph: {e:?}"))
            })?;

        let report_config = SubgraphConfig {
            input_mapping: Arc::new(|parent_state: Value| {
                let tool_call = parent_state
                    .get("current_tool_call")
                    .cloned()
                    .unwrap_or(Value::Null);
                let tool_call_id = tool_call
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let args_str = tool_call
                    .get("arguments")
                    .and_then(|v| v.as_str())
                    .unwrap_or("{}");
                let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
                json!({
                    "date_from": args.get("date_from").cloned().unwrap_or(json!("")),
                    "date_to": args.get("date_to").cloned().unwrap_or(json!("")),
                    "tool_call_id": tool_call_id,
                })
            }),
            output_mapping: Arc::new(|child_state: Value| {
                let report_json = child_state
                    .get("report_json")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let tool_call_id = child_state
                    .get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let tool_msg = json!({
                    "role": "tool",
                    "content": report_json,
                    "tool_call_id": tool_call_id,
                    "name": "report"
                });
                vec![
                    ChannelWrite::new("messages", tool_msg),
                    ChannelWrite::new("current_tool_call", Value::Null),
                ]
            }),
            checkpoint_ns: Some("subgraph:report".to_string()),
            recursion_limit: None,
        };
        graph.add_subgraph(
            "report",
            report_child,
            report_config,
            checkpoint_saver.clone(),
        )?;
    }

    // --- Subgraph: comparison ---
    {
        let comparison_deps = Arc::new(
            crate::service::ai::chat::subgraphs::comparison::ComparisonDeps {
                agents: Arc::clone(&deps.agents),
                db: Arc::clone(&deps.db),
                user_id: deps.user_id.clone(),
                account_id: deps.account_id.clone(),
            },
        );
        let comparison_child =
            crate::service::ai::chat::subgraphs::comparison::build(comparison_deps).map_err(
                |e| GraphError::validation(format!("Failed to build comparison subgraph: {e:?}")),
            )?;

        let comparison_config = SubgraphConfig {
            input_mapping: Arc::new(|parent_state: Value| {
                let tool_call = parent_state
                    .get("current_tool_call")
                    .cloned()
                    .unwrap_or(Value::Null);
                let tool_call_id = tool_call
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let args_str = tool_call
                    .get("arguments")
                    .and_then(|v| v.as_str())
                    .unwrap_or("{}");
                let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
                json!({
                    "query": args.get("query").cloned().unwrap_or(json!("")),
                    "trade_ids": args.get("trade_ids").cloned().unwrap_or(json!([])),
                    "tool_call_id": tool_call_id,
                })
            }),
            output_mapping: Arc::new(|child_state: Value| {
                let comparison_json = child_state
                    .get("comparison_json")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let tool_call_id = child_state
                    .get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let tool_msg = json!({
                    "role": "tool",
                    "content": comparison_json,
                    "tool_call_id": tool_call_id,
                    "name": "comparison"
                });
                vec![
                    ChannelWrite::new("messages", tool_msg),
                    ChannelWrite::new("current_tool_call", Value::Null),
                ]
            }),
            checkpoint_ns: Some("subgraph:comparison".to_string()),
            recursion_limit: None,
        };
        graph.add_subgraph(
            "comparison",
            comparison_child,
            comparison_config,
            checkpoint_saver,
        )?;
    }

    // --- Entry point ---
    graph.set_entry_point("llm")?;

    // --- Conditional edges: llm -> tool nodes or END ---
    graph.add_conditional_edges(
        "llm",
        |state: Value, _result: &NodeExecutionResult| {
            let tool_call = state
                .get("current_tool_call")
                .cloned()
                .unwrap_or(Value::Null);

            if tool_call.is_null() {
                // No tool call -- LLM produced a text response, finish.
                return Ok(vec![BranchTarget::End]);
            }

            let tool_name = tool_call.get("name").and_then(|v| v.as_str()).unwrap_or("");

            match tool_name {
                "db_query" => Ok(vec![BranchTarget::Node("db_query".to_owned())]),
                "semantic_search" => Ok(vec![BranchTarget::Node("semantic_search".to_owned())]),
                "analytics_calc" => Ok(vec![BranchTarget::Node("analytics_calc".to_owned())]),
                "recall_memory" => Ok(vec![BranchTarget::Node("recall_memory".to_owned())]),
                "research" => Ok(vec![BranchTarget::Node("research".to_owned())]),
                "report" => Ok(vec![BranchTarget::Node("report".to_owned())]),
                "comparison" => Ok(vec![BranchTarget::Node("comparison".to_owned())]),
                "create_agent" => Ok(vec![BranchTarget::Node("create_agent".to_owned())]),
                "save_agent" => Ok(vec![BranchTarget::Node("save_agent".to_owned())]),
                "run_agent" => Ok(vec![BranchTarget::Node("run_agent".to_owned())]),
                "edit_agent" => Ok(vec![BranchTarget::Node("edit_agent".to_owned())]),
                "stock_quote" => Ok(vec![BranchTarget::Node("stock_quote".to_owned())]),
                "stock_news" => Ok(vec![BranchTarget::Node("stock_news".to_owned())]),
                "financials" => Ok(vec![BranchTarget::Node("financials".to_owned())]),
                "earnings" => Ok(vec![BranchTarget::Node("earnings".to_owned())]),
                "company_info" => Ok(vec![BranchTarget::Node("company_info".to_owned())]),
                "get_notebook" => Ok(vec![BranchTarget::Node("get_notebook".to_owned())]),
                "get_playbook" => Ok(vec![BranchTarget::Node("get_playbook".to_owned())]),
                "view_media" => Ok(vec![BranchTarget::Node("view_media".to_owned())]),
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
    graph.add_edge("research", "llm")?;
    graph.add_edge("report", "llm")?;
    graph.add_edge("comparison", "llm")?;
    graph.add_edge("create_agent", "llm")?;
    graph.add_edge("save_agent", "llm")?;
    graph.add_edge("run_agent", "llm")?;
    graph.add_edge("edit_agent", "llm")?;
    graph.add_edge("stock_quote", "llm")?;
    graph.add_edge("stock_news", "llm")?;
    graph.add_edge("financials", "llm")?;
    graph.add_edge("earnings", "llm")?;
    graph.add_edge("company_info", "llm")?;
    graph.add_edge("get_notebook", "llm")?;
    graph.add_edge("get_playbook", "llm")?;
    graph.add_edge("view_media", "llm")?;

    // --- Compile ---
    graph.compile()
}

// ---------------------------------------------------------------------------
// Run helper
// ---------------------------------------------------------------------------
pub async fn run_chat_graph(
    compiled: &CompiledStateGraph,
    saver: &dyn CheckpointSaver,
    session_id: &str,
    user_message: Value,
) -> Result<LoopRunSummary, GraphError> {
    let config = LoopConfig::new(CheckpointConfig::new(session_id)).with_recursion_limit(12); // up to 5 tool calls + safety margin

    let input = json!({
        "messages": [user_message],
        "current_tool_call": Value::Null,
        "iteration": 0,
    });

    compiled.run_raw(Some(saver), config, input).await
}

// ---------------------------------------------------------------------------
// Async node implementations
// ---------------------------------------------------------------------------

/// LLM node: builds messages from state, calls stream_chat, and produces
/// channel writes for either a tool call or a final text response.
async fn llm_node_async(
    deps: &GraphDeps,
    state: &Value,
) -> Result<NodeExecutionResult, anyhow::Error> {
    // Reconstruct messages from the "messages" channel (accumulated array).
    let messages_val = state.get("messages").cloned().unwrap_or(json!([]));
    let raw_messages: Vec<Value> = match messages_val {
        Value::Array(arr) => arr,
        _ => vec![messages_val],
    };

    let mut groq_messages: Vec<LlmMessage> = vec![LlmMessage {
        role: "system".to_owned(),
        content: Some(deps.system_prompt.clone()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        media: None,
    }];

    // Auto-compact: if conversation is long, summarize older messages and keep recent ones full.
    // This preserves full context awareness without blowing up the token budget.
    const RECENT_KEEP: usize = 15; // keep the last 10 messages verbatim
    const COMPACT_THRESHOLD: usize = 20; // start compacting when we exceed this

    if raw_messages.len() > COMPACT_THRESHOLD {
        // Split into old messages (to summarize) and recent messages (to keep)
        let split_at = raw_messages.len() - RECENT_KEEP;
        let old_messages = &raw_messages[..split_at];
        let recent_messages = &raw_messages[split_at..];

        // Build a compact summary of old messages
        let mut summary_parts: Vec<String> = Vec::new();
        for msg_val in old_messages {
            let role = msg_val.get("role").and_then(|v| v.as_str()).unwrap_or("?");
            let content = msg_val
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if role == "tool" {
                let name = msg_val
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool");
                // Heavily truncate tool results in summary
                let short = if content.len() > 200 {
                    let end = content.floor_char_boundary(200);
                    &content[..end]
                } else {
                    content
                };
                summary_parts.push(format!("[Tool {name}: {short}...]"));
            } else if !content.is_empty() {
                let short = if content.len() > 300 {
                    let end = content.floor_char_boundary(300);
                    &content[..end]
                } else {
                    content
                };
                summary_parts.push(format!("{role}: {short}"));
            }
        }

        if !summary_parts.is_empty() {
            let summary = format!(
                "[Conversation summary ({} earlier messages)]\n{}",
                old_messages.len(),
                summary_parts.join("\n")
            );
            groq_messages.push(LlmMessage {
                role: "user".to_owned(),
                content: Some(summary),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                media: None,
            });
        }

        // Add recent messages verbatim (with tool result truncation)
        for msg_val in recent_messages {
            if let Ok(mut msg) = serde_json::from_value::<LlmMessage>(msg_val.clone()) {
                if (msg.role == "tool" || msg.role == "assistant")
                    && let Some(ref content) = msg.content
                    && content.len() > 3000
                {
                    let end = content.floor_char_boundary(3000);
                    msg.content = Some(format!("{}... [truncated]", &content[..end]));
                }
                groq_messages.push(msg);
            }
        }
    } else {
        // Short conversation — send everything, just truncate long tool results
        for msg_val in &raw_messages {
            if let Ok(mut msg) = serde_json::from_value::<LlmMessage>(msg_val.clone()) {
                if (msg.role == "tool" || msg.role == "assistant")
                    && let Some(ref content) = msg.content
                    && content.len() > 3000
                {
                    let end = content.floor_char_boundary(3000);
                    msg.content = Some(format!("{}... [truncated]", &content[..end]));
                }
                groq_messages.push(msg);
            }
        }
    }

    // Determine iteration count
    let iteration = state.get("iteration").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

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
        LlmChatResponse::ToolCall {
            id,
            name,
            arguments,
            thought_signature,
        } => {
            info!("LLM node: tool_call {} (iteration {})", name, iteration);

            // Write the assistant tool-call message into the messages channel
            let assistant_msg = serde_json::to_value(LlmMessage {
                role: "assistant".to_owned(),
                content: None,
                tool_calls: Some(vec![LlmToolCall {
                    id: id.clone(),
                    call_type: "function".to_owned(),
                    function: LlmFunctionCall {
                        name: name.clone(),
                        arguments: arguments.clone(),
                    },
                    thought_signature: thought_signature.clone(),
                }]),
                tool_call_id: None,
                name: None,
                media: None,
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

        LlmChatResponse::TextComplete { full_text } => {
            info!("LLM node: text complete (iteration {})", iteration);

            let assistant_msg = serde_json::to_value(LlmMessage {
                role: "assistant".to_owned(),
                content: Some(full_text.clone()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                media: None,
            })?;

            // NOTE: Done is broadcast from run_chat_agent after the graph run
            // completes and the checkpoint is persisted. Broadcasting it here
            // would race the checkpoint write — a client refetch in response
            // to Done could read stale chat history.

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
        content: Some(arguments.clone()),
        tool_name: Some(tool_name.clone()),
        message_id: None,
    });

    // Pass conversation messages so recall_memory can search current session
    let conversation_messages = state.get("messages");

    // Execute
    let result = tools::execute_tool(
        &tool_name,
        &arguments,
        &deps.user_id,
        &deps.account_id,
        &deps.db,
        &deps.qdrant,
        &deps.r2,
        Some(&deps.agents),
        None,
        conversation_messages,
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

    // For view_media, the tool returns a media envelope { text, media: [...] }.
    // Write the functionResponse tool message using only `text`, then inject the
    // media as a follow-up `user` turn so Gemini actually sees the image/video on
    // the next iteration. All other tools behave exactly as before.
    if tool_name == "view_media"
        && let Ok(envelope) = serde_json::from_str::<Value>(&result)
        && envelope.is_object()
    {
        let text = envelope
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or(&result)
            .to_owned();

        let media_parts: Vec<LlmMediaPart> = envelope
            .get("media")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| serde_json::from_value::<LlmMediaPart>(p.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        let tool_msg = serde_json::to_value(LlmMessage {
            role: "tool".to_owned(),
            content: Some(text),
            tool_calls: None,
            tool_call_id: Some(tool_id),
            name: Some(tool_name),
            media: None,
        })?;

        let mut node_result =
            NodeExecutionResult::default().with_write(ChannelWrite::new("messages", tool_msg));

        if !media_parts.is_empty() {
            let media_msg = serde_json::to_value(LlmMessage {
                role: "user".to_owned(),
                content: None,
                tool_calls: None,
                tool_call_id: None,
                name: None,
                media: Some(media_parts),
            })?;
            node_result = node_result.with_write(ChannelWrite::new("messages", media_msg));
        }

        return Ok(node_result.with_write(ChannelWrite::new("current_tool_call", Value::Null)));
    }

    // Write tool result message into messages channel
    let tool_msg = serde_json::to_value(LlmMessage {
        role: "tool".to_owned(),
        content: Some(result),
        tool_calls: None,
        tool_call_id: Some(tool_id),
        name: Some(tool_name),
        media: None,
    })?;

    // Clear current_tool_call so the next llm iteration starts fresh
    Ok(NodeExecutionResult::default()
        .with_write(ChannelWrite::new("messages", tool_msg))
        .with_write(ChannelWrite::new("current_tool_call", Value::Null)))
}
