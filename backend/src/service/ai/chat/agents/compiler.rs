use std::sync::Arc;

use langgraph::core::constants::END;
use langgraph::prelude::*;
use serde_json::{Value, json};

use crate::service::ai::chat::tools;
use crate::service::ai::client::AgentsClient;
use crate::service::ai::vector_database::client::VectorDatabaseClient;
use crate::service::r2::R2Client;
use crate::service::turso::TursoClient;

use super::definition::AgentDefinition;

// ---------------------------------------------------------------------------
// Shared dependencies for all compiled agent nodes
// ---------------------------------------------------------------------------
struct AgentDeps {
    agents: Arc<AgentsClient>,
    turso: Arc<TursoClient>,
    qdrant: Arc<VectorDatabaseClient>,
    r2: Arc<R2Client>,
    user_id: String,
    account_id: String,
}

// ---------------------------------------------------------------------------
// compile_agent — turns an AgentDefinition into a runnable CompiledStateGraph
// ---------------------------------------------------------------------------
pub fn compile_agent(
    def: &AgentDefinition,
    agents: Arc<AgentsClient>,
    turso: Arc<TursoClient>,
    qdrant: Arc<VectorDatabaseClient>,
    r2: Arc<R2Client>,
) -> Result<CompiledStateGraph, GraphError> {
    let deps = Arc::new(AgentDeps {
        agents,
        turso,
        qdrant,
        r2,
        user_id: def.user_id.clone(),
        account_id: def.account_id.clone(),
    });

    // --- Build schema: one LastValue channel per unique output_channel ---
    let mut schema = StateSchema::new()
        .with_last_value("synthesis")?
        .with_last_value("tool_call_id")?;

    // Collect unique output channels from non-synthesize steps
    let mut seen_channels: std::collections::HashSet<String> = std::collections::HashSet::new();
    for step in &def.steps {
        if step.tool == "synthesize" {
            continue;
        }
        if let Some(ch) = &step.output_channel
            && seen_channels.insert(ch.clone())
        {
            schema = schema.with_last_value(ch)?;
        }
    }

    let mut graph = StateGraph::<Value>::new(schema);

    let goal = def.goal.clone();
    let output_style = def.output_style.clone();

    // Separate synthesize step from the rest
    let tool_steps: Vec<_> = def
        .steps
        .iter()
        .filter(|s| s.tool != "synthesize")
        .cloned()
        .collect();

    // --- Add a node for each tool step ---
    let mut node_names: Vec<String> = Vec::new();

    for (i, step) in tool_steps.iter().enumerate() {
        let node_name = format!("step_{i}");
        node_names.push(node_name.clone());

        let step_deps = Arc::clone(&deps);
        let tool_name = step.tool.clone();
        let tool_args = step.args.clone();
        let output_channel = step.output_channel.clone();

        graph.add_node(
            &node_name,
            move |_state: Value, _ctx: ExecutionContext<'_>| {
                let deps = Arc::clone(&step_deps);
                let tool_name = tool_name.clone();
                let tool_args = tool_args.clone();
                let output_channel = output_channel.clone();

                let handle = tokio::runtime::Handle::current();
                tokio::task::block_in_place(|| {
                    handle.block_on(async move {
                        let arguments =
                            serde_json::to_string(&tool_args).unwrap_or_else(|_| "{}".to_string());

                        let result = tools::execute_tool(
                            &tool_name,
                            &arguments,
                            &deps.user_id,
                            &deps.account_id,
                            &deps.turso,
                            &deps.qdrant,
                            &deps.r2,
                            Some(&deps.agents),
                            None,
                            None,
                        )
                        .await
                        .unwrap_or_else(|e| format!("{tool_name} error: {e}"));

                        let mut node_result = NodeExecutionResult::default();
                        if let Some(ch) = &output_channel {
                            node_result =
                                node_result.with_write(ChannelWrite::new(ch, json!(result)));
                        }
                        Ok::<NodeExecutionResult, anyhow::Error>(node_result)
                    })
                })
                .map_err(|e| NodeExecutionError::fatal(e.to_string()))
            },
        )?;
    }

    // --- Add synthesize node ---
    let synth_deps = Arc::clone(&deps);
    let output_channels: Vec<String> = seen_channels.into_iter().collect();
    let synth_goal = goal.clone();
    let synth_style = output_style.clone();

    graph.add_node(
        "synthesize",
        move |state: Value, _ctx: ExecutionContext<'_>| {
            let deps = Arc::clone(&synth_deps);
            let output_channels = output_channels.clone();
            let goal = synth_goal.clone();
            let output_style = synth_style.clone();

            let handle = tokio::runtime::Handle::current();
            tokio::task::block_in_place(|| {
                handle.block_on(async move {
                    // Gather all output channel values from state
                    let mut data_sections = String::new();
                    for ch in &output_channels {
                        if let Some(val) = state.get(ch) {
                            let owned;
                            let text = if let Some(s) = val.as_str() {
                                s
                            } else {
                                owned = val.to_string();
                                &owned
                            };
                            data_sections.push_str(&format!("\n\n## {ch}\n{text}"));
                        }
                    }

                    let synthesis_prompt = format!(
                        "You are a trading analyst. {output_style}\n\nGoal: {goal}{data_sections}\n\n\
                         Provide a focused synthesis that directly addresses the goal. \
                         Be specific with numbers. No markdown tables."
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
            })
            .map_err(|e| NodeExecutionError::fatal(e.to_string()))
        },
    )?;

    // --- Wire edges: step_0 → step_1 → ... → synthesize → END ---
    if node_names.is_empty() {
        graph.set_entry_point("synthesize")?;
    } else {
        graph.set_entry_point(&node_names[0])?;
        for window in node_names.windows(2) {
            graph.add_edge(&window[0], &window[1])?;
        }
        graph.add_edge(node_names.last().unwrap(), "synthesize")?;
    }
    graph.add_edge("synthesize", END)?;

    graph.compile()
}
