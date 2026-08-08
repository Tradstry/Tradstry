use anyhow::{Result, bail, ensure};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::db::Db;
use crate::service::db::schema::tables::user_agents_table;

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
                        "items": {"type": "string", "enum": ["db_query", "get_playbook", "semantic_search", "analytics_calc"]},
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
    workspace_id: &str,
    db: &Arc<Db>,
) -> Result<String> {
    let input: EditAgentInput = serde_json::from_str(arguments)?;
    let pool = db.pool();

    let agent =
        user_agents_table::find_user_agent_by_name(pool, &input.agent_name, user_id, workspace_id)
            .await?;

    let agent = match agent {
        Some(a) => a,
        None => {
            let agents = user_agents_table::list_user_agents(pool, user_id, workspace_id).await?;
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
        ensure!(
            !sources.is_empty(),
            "agent must have at least one data source"
        );
        let goal = input.new_goal.as_deref().unwrap_or(&agent.goal);
        let symbol = input.new_symbol.as_deref();
        let mut steps = Vec::new();

        for source in sources {
            match source.as_str() {
                "trades" | "db_query" => {
                    let mut filters = json!({});
                    if let Some(sym) = symbol
                        && sym != "all"
                    {
                        filters["symbol"] = json!(sym);
                    }
                    steps.push(json!({"tool": "db_query", "args": {"entity": "trades", "filters": filters, "limit": 50}, "output_channel": "trades"}));
                }
                "playbooks" | "get_playbook" => {
                    steps.push(json!({"tool": "get_playbook", "args": {"workspace_id": workspace_id}, "output_channel": "playbook_rules"}));
                }
                "patterns" | "semantic_search" => {
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
                "metrics" | "analytics_calc" => {
                    let mut args = json!({"metrics": ["win_rate", "total_pnl", "avg_r", "profit_factor", "streak", "per_symbol"]});
                    if let Some(sym) = symbol
                        && sym != "all"
                    {
                        args["filters"] = json!({"symbol": sym});
                    }
                    steps.push(json!({"tool": "analytics_calc", "args": args, "output_channel": "metrics"}));
                }
                unknown => bail!("unsupported agent data source: {unknown}"),
            }
        }
        steps.push(json!({"tool": "synthesize", "args": {"goal": goal}}));
        Some(serde_json::to_string(&steps)?)
    } else if let Some(symbol) = input.new_symbol.as_deref() {
        let mut steps: Vec<serde_json::Value> = serde_json::from_str(&agent.steps_json)?;
        for step in &mut steps {
            let tool = step
                .get("tool")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned();
            let Some(args) = step
                .get_mut("args")
                .and_then(serde_json::Value::as_object_mut)
            else {
                continue;
            };
            match tool.as_str() {
                "db_query" | "analytics_calc" => {
                    let filters = args.entry("filters").or_insert_with(|| json!({}));
                    if let Some(filters) = filters.as_object_mut() {
                        if symbol == "all" {
                            filters.remove("symbol");
                        } else {
                            filters.insert("symbol".to_owned(), json!(symbol));
                        }
                    }
                }
                "stock_quote" | "stock_news" | "earnings" | "financials" | "company_info" => {
                    if symbol == "all" {
                        args.remove("symbol");
                    } else {
                        args.insert("symbol".to_owned(), json!(symbol));
                    }
                }
                _ => {}
            }
        }
        Some(serde_json::to_string(&steps)?)
    } else {
        None
    };

    let updated = user_agents_table::update_user_agent(
        pool,
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
    if input.new_symbol.is_some() {
        changes.push("symbol");
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
