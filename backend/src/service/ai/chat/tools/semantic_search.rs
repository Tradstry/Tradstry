use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::ai::vector_database::client::VectorDatabaseClient;

#[derive(Debug, Deserialize)]
struct SemanticSearchInput {
    query: String,
    date_from: Option<String>,
    date_to: Option<String>,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "semantic_search".to_string(),
            description:
                "Perform a hybrid semantic search across trading data including journal notes, \
                 mistakes, tactics, and playbook content."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural language search query."
                    },
                    "date_from": {
                        "type": "string",
                        "description": "Optional start date filter (ISO 8601)."
                    },
                    "date_to": {
                        "type": "string",
                        "description": "Optional end date filter (ISO 8601)."
                    }
                },
                "required": ["query"]
            }),
        },
    }
}

pub async fn execute(
    arguments: &str,
    user_id: &str,
    account_id: &str,
    qdrant: &Arc<VectorDatabaseClient>,
) -> Result<String> {
    let input: SemanticSearchInput = serde_json::from_str(arguments)?;

    let results = qdrant
        .hybrid_search(
            &input.query,
            user_id,
            account_id,
            input.date_from.as_deref(),
            input.date_to.as_deref(),
            5,
        )
        .await?;

    if results.is_empty() {
        return Ok("No results found".to_string());
    }

    let output: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            json!({
                "source_id": r.source_id,
                "source_type": r.source_type,
                "text": r.text,
                "score": r.score,
            })
        })
        .collect();

    Ok(serde_json::to_string(&output)?)
}
