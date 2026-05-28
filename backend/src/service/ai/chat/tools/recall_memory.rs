use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::ai::vector_database::client::VectorDatabaseClient;

#[derive(Debug, Deserialize)]
struct RecallMemoryInput {
    query: String,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
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
    conversation_messages: Option<&Value>,
) -> Result<String> {
    let input: RecallMemoryInput = serde_json::from_str(arguments)?;
    let query_lower = input.query.to_lowercase();

    // 1. Search Qdrant for memories from previous sessions
    let qdrant_memories = qdrant
        .search_memories(&input.query, user_id, 5)
        .await
        .unwrap_or_default();

    // 2. Search current conversation history for relevant context
    let mut session_memories: Vec<String> = Vec::new();
    if let Some(Value::Array(messages)) = conversation_messages {
        // Skip the last user message — that's the current turn that triggered this tool call
        let last_user_idx = messages
            .iter()
            .rposition(|m| m.get("role").and_then(|v| v.as_str()) == Some("user"));

        for (idx, msg) in messages.iter().enumerate() {
            // Skip the current user message to avoid echoing it back
            if Some(idx) == last_user_idx {
                continue;
            }
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "user" && role != "assistant" {
                continue;
            }
            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if content.is_empty() || content.len() < 10 {
                continue;
            }
            // Check if any query keyword (3+ chars) appears in the message
            let content_lower = content.to_lowercase();
            let matches = query_lower
                .split_whitespace()
                .filter(|w| w.len() >= 3)
                .any(|word| content_lower.contains(word));
            if matches {
                let prefix = if role == "user" {
                    "User said"
                } else {
                    "Assistant said"
                };
                let snippet = if content.len() > 300 {
                    format!("{}...", &content[..300])
                } else {
                    content.to_string()
                };
                session_memories.push(format!("{prefix}: \"{snippet}\""));
            }
        }
        // Keep only the last 5 session matches to avoid flooding
        if session_memories.len() > 5 {
            let start = session_memories.len() - 5;
            session_memories = session_memories[start..].to_vec();
        }
    }

    // 3. Combine: Qdrant memories first, then session context
    let mut output: Vec<Value> = Vec::new();
    for (i, memory) in qdrant_memories.iter().enumerate() {
        output.push(json!({ "index": i, "source": "memory", "memory": memory }));
    }
    let offset = qdrant_memories.len();
    for (i, context) in session_memories.iter().enumerate() {
        output.push(json!({ "index": offset + i, "source": "current_session", "memory": context }));
    }

    Ok(serde_json::to_string(&output)?)
}
