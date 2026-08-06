use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::service::ai::chat::agents::runner;
use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::ai::client::AgentsClient;
use crate::service::ai::vector_database::client::VectorDatabaseClient;
use crate::service::db::Db;
use crate::service::r2::R2Client;

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
    workspace_id: &str,
    agents: &Arc<AgentsClient>,
    db: &Arc<Db>,
    qdrant: &Arc<VectorDatabaseClient>,
    r2: &Arc<R2Client>,
) -> Result<String> {
    let input: RunAgentInput = serde_json::from_str(arguments)?;
    let overrides = input.overrides.unwrap_or(json!({}));

    match runner::run_user_agent_by_name(
        &input.agent_name,
        user_id,
        workspace_id,
        &overrides,
        agents,
        db,
        qdrant,
        r2,
    )
    .await
    {
        Ok(synthesis) => Ok(synthesis),
        Err(e) => {
            // On failure, list available agents to help the user
            let names = runner::list_agent_names(db, user_id, workspace_id)
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
