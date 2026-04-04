use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::service::chat::types::{GroqFunctionDef, GroqToolDef};
use crate::service::turso::TursoClient;

#[derive(Debug, Deserialize)]
struct SaveAgentInput {
    name: String,
    goal: String,
    #[serde(default)]
    data_sources: Vec<String>,
    #[serde(default)]
    symbol: Option<String>,
    #[serde(default)]
    playbook_name: Option<String>,
    #[serde(default)]
    output_style: Option<String>,
}

pub fn schema() -> GroqToolDef {
    GroqToolDef {
        tool_type: "function".to_string(),
        function: GroqFunctionDef {
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
                        "items": { "type": "string" },
                        "description": "Tools the agent should use: db_query, semantic_search, analytics_calc."
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Optional ticker symbol to focus on."
                    },
                    "date_range": {
                        "type": "string",
                        "description": "Default date range description (e.g. 'last 7 days')."
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
    account_id: &str,
    turso: &Arc<TursoClient>,
) -> Result<String> {
    let input: SaveAgentInput = serde_json::from_str(arguments)?;

    // Build steps_json from data_sources
    let mut steps: Vec<serde_json::Value> = Vec::new();

    for source in input.data_sources.iter() {
        let channel = format!("{}_result", source);
        let mut args = json!({});

        match source.as_str() {
            "db_query" => {
                args["entity"] = json!("trades");
                let mut filters = json!({});
                if let Some(ref sym) = input.symbol {
                    filters["symbol"] = json!(sym);
                }
                if let Some(ref pb) = input.playbook_name {
                    filters["playbook"] = json!(pb);
                }
                args["filters"] = filters;
                args["limit"] = json!(50);
            }
            "semantic_search" => {
                args["query"] = json!(input.goal);
            }
            "analytics_calc" => {
                args["metrics"] = json!(["win_rate", "total_pnl", "avg_r", "profit_factor", "per_symbol"]);
                let mut filters = json!({});
                if let Some(ref sym) = input.symbol {
                    filters["symbol"] = json!(sym);
                }
                args["filters"] = filters;
            }
            _ => {
                // Unknown data source; include it as-is
                args["query"] = json!(input.goal);
            }
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

    let conn = turso.get_connection()?;

    // Check if an agent with this name already exists — update instead of creating duplicate
    let existing = crate::service::turso::schema::tables::user_agents_table::find_user_agent_by_name(
        &conn, &input.name, user_id, account_id,
    ).await?;

    let agent = if let Some(existing) = existing {
        crate::service::turso::schema::tables::user_agents_table::update_user_agent(
            &conn,
            &existing.id,
            user_id,
            Some(&input.name),
            Some(&input.goal),
            Some(&steps_json),
            Some(&output_style),
            Some(&config_json),
        ).await?
    } else {
        crate::service::turso::schema::tables::user_agents_table::create_user_agent(
            &conn,
            user_id,
            account_id,
            &input.name,
            &input.goal,
            &steps_json,
            &output_style,
            &config_json,
        ).await?
    };

    Ok(format!(
        "Agent '{}' saved successfully (id: {}). You can run it anytime with: run my {}",
        agent.name, agent.id, agent.name
    ))
}
