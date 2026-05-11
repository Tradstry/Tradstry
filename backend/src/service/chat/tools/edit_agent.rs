use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::service::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::turso::TursoClient;
use crate::service::turso::schema::tables::user_agents_table;

#[derive(Debug, Deserialize)]
struct EditAgentInput {
    agent_name: String,
    #[serde(default)]
    new_name: Option<String>,
    #[serde(default)]
    new_goal: Option<String>,
    #[serde(default)]
    new_data_sources: Option<Vec<String>>,
    #[serde(default)]
    new_symbol: Option<String>,
    #[serde(default)]
    new_output_style: Option<String>,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "edit_agent".to_string(),
            description: "Edit an existing custom agent. Change its name, goal, data sources, symbol focus, or output style. Only include the fields you want to change.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "agent_name": {
                        "type": "string",
                        "description": "Current name of the agent to edit"
                    },
                    "new_name": {
                        "type": "string",
                        "description": "New name for the agent (optional)"
                    },
                    "new_goal": {
                        "type": "string",
                        "description": "New goal for the agent (optional)"
                    },
                    "new_data_sources": {
                        "type": "array",
                        "items": {"type": "string", "enum": ["trades", "playbooks", "patterns", "metrics"]},
                        "description": "New data sources (optional, replaces existing)"
                    },
                    "new_symbol": {
                        "type": "string",
                        "description": "New symbol filter (optional, use 'all' to remove filter)"
                    },
                    "new_output_style": {
                        "type": "string",
                        "enum": ["concise", "detailed", "json"],
                        "description": "New output style (optional)"
                    }
                },
                "required": ["agent_name"]
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
    let input: EditAgentInput = serde_json::from_str(arguments)?;
    let conn = turso.get_connection()?;

    let agent =
        user_agents_table::find_user_agent_by_name(&conn, &input.agent_name, user_id, account_id)
            .await?;

    let agent = match agent {
        Some(a) => a,
        None => {
            let agents = user_agents_table::list_user_agents(&conn, user_id, account_id).await?;
            let names: Vec<_> = agents.iter().map(|a| a.name.as_str()).collect();
            if names.is_empty() {
                return Ok(format!(
                    "Agent '{}' not found. You don't have any agents yet.",
                    input.agent_name
                ));
            }
            return Ok(format!(
                "Agent '{}' not found. Your agents: {}",
                input.agent_name,
                names.join(", ")
            ));
        }
    };

    // Rebuild steps_json if data sources changed
    let new_steps_json = if let Some(ref sources) = input.new_data_sources {
        let goal = input.new_goal.as_deref().unwrap_or(&agent.goal);
        let symbol = input.new_symbol.as_deref();
        let mut steps = Vec::new();

        for source in sources {
            match source.as_str() {
                "trades" => {
                    let mut filters = json!({});
                    if let Some(sym) = symbol
                        && sym != "all"
                    {
                        filters["symbol"] = json!(sym);
                    }
                    steps.push(json!({"tool": "db_query", "args": {"entity": "trades", "filters": filters, "limit": 50}, "output_channel": "trades"}));
                }
                "playbooks" => {
                    steps.push(json!({"tool": "db_query", "args": {"entity": "playbook", "limit": 20}, "output_channel": "playbook_rules"}));
                }
                "patterns" => {
                    let query = if let Some(sym) = symbol {
                        if sym != "all" {
                            format!("{} {}", goal, sym)
                        } else {
                            goal.to_string()
                        }
                    } else {
                        goal.to_string()
                    };
                    steps.push(json!({"tool": "semantic_search", "args": {"query": query}, "output_channel": "patterns"}));
                }
                "metrics" => {
                    let mut args = json!({"metrics": ["win_rate", "total_pnl", "avg_r", "profit_factor", "streak", "per_symbol"]});
                    if let Some(sym) = symbol
                        && sym != "all"
                    {
                        args["filters"] = json!({"symbol": sym});
                    }
                    steps.push(json!({"tool": "analytics_calc", "args": args, "output_channel": "metrics"}));
                }
                _ => {}
            }
        }
        steps.push(json!({"tool": "synthesize", "args": {"goal": goal}}));
        Some(serde_json::to_string(&steps)?)
    } else {
        None
    };

    let updated = user_agents_table::update_user_agent(
        &conn,
        &agent.id,
        user_id,
        input.new_name.as_deref(),
        input.new_goal.as_deref(),
        new_steps_json.as_deref(),
        input.new_output_style.as_deref(),
        None,
    )
    .await?;

    let mut changes = Vec::new();
    if input.new_name.is_some() {
        changes.push("name");
    }
    if input.new_goal.is_some() {
        changes.push("goal");
    }
    if input.new_data_sources.is_some() {
        changes.push("data sources");
    }
    if input.new_output_style.is_some() {
        changes.push("output style");
    }

    Ok(format!(
        "Agent '{}' updated. Changed: {}.",
        updated.name,
        if changes.is_empty() {
            "nothing".to_string()
        } else {
            changes.join(", ")
        }
    ))
}
