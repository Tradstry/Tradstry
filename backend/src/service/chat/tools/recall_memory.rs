use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::service::agents::vector_database::client::VectorDatabaseClient;
use crate::service::chat::types::{GroqFunctionDef, GroqToolDef};

#[derive(Debug, Deserialize)]
struct RecallMemoryInput {
    query: String,
}

pub fn schema() -> GroqToolDef {
    GroqToolDef {
        tool_type: "function".to_string(),
        function: GroqFunctionDef {
            name: "recall_memory".to_string(),
            description:
                "Search your memory of past conversations with this user. Use when the user \
                 references something from a previous chat or when you need context about their \
                 preferences, patterns, or history."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "What to search for in memory."
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
    qdrant: &Arc<VectorDatabaseClient>,
) -> Result<String> {
    let input: RecallMemoryInput = serde_json::from_str(arguments)?;

    let memories = qdrant
        .search_memories(&input.query, user_id, 5)
        .await
        .unwrap_or_default();

    let output: Vec<serde_json::Value> = memories
        .iter()
        .enumerate()
        .map(|(i, memory)| {
            json!({
                "index": i,
                "memory": memory,
            })
        })
        .collect();

    Ok(serde_json::to_string(&output)?)
}
