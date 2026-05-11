use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::service::agents::client::AgentsClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;
use crate::service::chat::agents::runner;
use crate::service::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::turso::TursoClient;

#[derive(Debug, Deserialize)]
struct RunAgentInput {
    agent_name: String,
    #[serde(default)]
    overrides: Option<serde_json::Value>,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "run_agent".to_string(),
            description:
                "Run a saved custom agent by name. Use when the user says 'run my [agent name]'."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "agent_name": {
                        "type": "string",
                        "description": "Name of the saved agent to run."
                    },
                    "overrides": {
                        "type": "object",
                        "description": "Optional runtime overrides: symbol, date_from, date_to, query.",
                        "properties": {
                            "symbol": { "type": "string" },
                            "date_from": { "type": "string" },
                            "date_to": { "type": "string" },
                            "query": { "type": "string" }
                        }
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
    agents: &Arc<AgentsClient>,
    turso: &Arc<TursoClient>,
    qdrant: &Arc<VectorDatabaseClient>,
) -> Result<String> {
    let input: RunAgentInput = serde_json::from_str(arguments)?;
    let overrides = input.overrides.unwrap_or(json!({}));

    match runner::run_user_agent_by_name(
        &input.agent_name,
        user_id,
        account_id,
        &overrides,
        agents,
        turso,
        qdrant,
    )
    .await
    {
        Ok(synthesis) => Ok(synthesis),
        Err(e) => {
            // On failure, list available agents to help the user
            let names = runner::list_agent_names(turso, user_id, account_id)
                .await
                .unwrap_or_default();

            if names.is_empty() {
                Ok(format!(
                    "Failed to run agent '{}': {}. You don't have any saved agents yet. \
                     Use create_agent to build one.",
                    input.agent_name, e
                ))
            } else {
                Ok(format!(
                    "Failed to run agent '{}': {}. Your available agents are: {}",
                    input.agent_name,
                    e,
                    names.join(", ")
                ))
            }
        }
    }
}
