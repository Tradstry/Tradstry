use anyhow::{Result, bail, ensure};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::db::Db;

#[derive(Debug, Deserialize)]
struct SaveAgentInput {
    name: String,
    goal: String,
    #[serde(default)]
    data_sources: Vec<String>,
    #[serde(default)]
    symbol: Option<String>,
    #[serde(default)]
    date_from: Option<String>,
    #[serde(default)]
    date_to: Option<String>,
    #[serde(default)]
    playbook_name: Option<String>,
    #[serde(default)]
    output_style: Option<String>,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "save_agent".to_string(),
            description:
                "Save a custom agent after the interview is complete. Only call this when you \
                 have all required fields."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Short, memorable name for the agent."
                    },
                    "goal": {
                        "type": "string",
                        "description": "One-sentence goal for what the agent accomplishes."
                    },
                    "data_sources": {
                        "type": "array",
                        "items": { "type": "string", "enum": ["db_query", "semantic_search", "analytics_calc", "get_playbook"] },
                        "description": "Validated tools the agent should use."
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Optional ticker symbol to focus on."
                    },
                    "date_from": {
                        "type": "string",
                        "description": "Optional default start date (ISO 8601)."
                    },
                    "date_to": {
                        "type": "string",
                        "description": "Optional default end date (ISO 8601)."
                    },
                    "playbook_name": {
                        "type": "string",
                        "description": "Optional playbook name to scope the agent to."
                    },
                    "output_style": {
                        "type": "string",
                        "description": "How results should be presented (e.g. 'concise bullet points')."
                    }
                },
                "required": ["name", "goal", "data_sources"]
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
    let input: SaveAgentInput = serde_json::from_str(arguments)?;
    ensure!(!input.name.trim().is_empty(), "agent name cannot be empty");
    ensure!(!input.goal.trim().is_empty(), "agent goal cannot be empty");
    ensure!(
        !input.data_sources.is_empty() || input.playbook_name.is_some(),
        "agent must have at least one data source"
    );

    // Build steps_json from data_sources
    let mut steps: Vec<serde_json::Value> = Vec::new();

    let mut data_sources = input.data_sources.clone();
    if input.playbook_name.is_some() && !data_sources.iter().any(|source| source == "get_playbook")
    {
        data_sources.push("get_playbook".to_owned());
    }

    for source in &data_sources {
        let channel = format!("{}_result", source);
        let mut args = json!({});

        match source.as_str() {
            "db_query" => {
                args["entity"] = json!("trades");
                let mut filters = json!({});
                if let Some(ref sym) = input.symbol {
                    filters["symbol"] = json!(sym);
                }
                if let Some(ref from) = input.date_from {
                    filters["date_from"] = json!(from);
                }
                if let Some(ref to) = input.date_to {
                    filters["date_to"] = json!(to);
                }
                args["filters"] = filters;
                args["limit"] = json!(50);
            }
            "semantic_search" => {
                args["query"] = json!(input.goal);
                if let Some(ref from) = input.date_from {
                    args["date_from"] = json!(from);
                }
                if let Some(ref to) = input.date_to {
                    args["date_to"] = json!(to);
                }
            }
            "analytics_calc" => {
                args["metrics"] = json!([
                    "win_rate",
                    "total_pnl",
                    "avg_r",
                    "profit_factor",
                    "per_symbol"
                ]);
                let mut filters = json!({});
                if let Some(ref sym) = input.symbol {
                    filters["symbol"] = json!(sym);
                }
                if let Some(ref from) = input.date_from {
                    filters["date_from"] = json!(from);
                }
                if let Some(ref to) = input.date_to {
                    filters["date_to"] = json!(to);
                }
                args["filters"] = filters;
            }
            "get_playbook" => {
                args["workspace_id"] = json!(workspace_id);
                if let Some(ref name) = input.playbook_name {
                    args["playbook_name"] = json!(name);
                }
            }
            unknown => bail!("unsupported agent data source: {unknown}"),
        }

        steps.push(json!({
            "tool": source,
            "args": args,
            "output_channel": channel,
        }));
    }

    // Add synthesize step
    steps.push(json!({
        "tool": "synthesize",
        "args": {},
        "output_channel": null,
    }));

    let steps_json = serde_json::to_string(&steps)?;
    let output_style = input
        .output_style
        .unwrap_or_else(|| "Concise and data-driven.".to_string());
    let config_json = json!({}).to_string();

    let pool = db.pool();

    // Check if an agent with this name already exists — update instead of creating duplicate
    let existing = crate::service::db::schema::tables::user_agents_table::find_user_agent_by_name(
        pool,
        &input.name,
        user_id,
        workspace_id,
    )
    .await?;

    let agent = if let Some(existing) = existing {
        crate::service::db::schema::tables::user_agents_table::update_user_agent(
            pool,
            &existing.id,
            user_id,
            Some(&input.name),
            Some(&input.goal),
            Some(&steps_json),
            Some(&output_style),
            Some(&config_json),
        )
        .await?
    } else {
        crate::service::db::schema::tables::user_agents_table::create_user_agent(
            pool,
            user_id,
            workspace_id,
            &input.name,
            &input.goal,
            &steps_json,
            &output_style,
            &config_json,
        )
        .await?
    };

    Ok(format!(
        "Agent '{}' saved successfully. You can run it anytime with: run my {}",
        agent.name, agent.name
    ))
}
