use anyhow::Result;
use langgraph::prelude::{NamespacePath, Store, StoreListQuery};
use log::{error, info};
use serde_json::Value;

use crate::service::agents::client::AgentsClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;

/// Strip tool messages and keep only the last N user/assistant exchanges.
fn compact_for_extraction(messages_json: &str, max_messages: usize) -> String {
    let messages: Vec<Value> = serde_json::from_str(messages_json).unwrap_or_default();

    // Keep all messages but skip assistant messages that are only tool calls (no text content)
    let filtered: Vec<&Value> = messages
        .iter()
        .filter(|m| {
            let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role == "assistant" {
                let has_content = m
                    .get("content")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                if !has_content {
                    return false;
                }
            }
            true
        })
        .collect();

    // Take only the last N messages
    let recent: Vec<&Value> = if filtered.len() > max_messages {
        filtered[filtered.len() - max_messages..].to_vec()
    } else {
        filtered
    };

    // Build compact representations — truncate long content, keep tool name for context
    let compact: Vec<Value> = recent
        .iter()
        .map(|m| {
            let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("unknown");
            let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let max_len = if role == "tool" { 200 } else { 500 };
            let truncated = if content.len() > max_len {
                let end = content.floor_char_boundary(max_len);
                format!("{}...", &content[..end])
            } else {
                content.to_string()
            };
            let mut obj = serde_json::json!({"role": role, "content": truncated});
            if role == "tool" {
                if let Some(name) = m.get("name") {
                    obj["name"] = name.clone();
                }
            }
            obj
        })
        .collect();

    serde_json::to_string(&compact).unwrap_or_default()
}

/// Extract memories from a conversation and store them in the key-value store and Qdrant.
pub async fn extract_and_store_memories(
    messages_json: &str,
    user_id: &str,
    _session_id: &str,
    agents: &AgentsClient,
    store: &dyn Store,
    qdrant: &VectorDatabaseClient,
) -> Result<()> {
    // 1. Compact the conversation: strip tool messages, keep last 10 exchanges
    let compact_messages = compact_for_extraction(messages_json, 10);

    // 2. Call the LLM to extract new memories from the conversation
    let extraction_prompt = format!(
        "Review this trading assistant conversation and extract any NEW facts, preferences, \
         patterns, or instructions the user revealed. Only extract information that would be \
         useful in future conversations.\n\n\
         Return a JSON array of strings. Each string is one distinct memory. If nothing new \
         was revealed, return an empty array [].\n\n\
         Conversation:\n{compact_messages}"
    );

    let extraction_response = agents.prompt(&extraction_prompt).await?;
    let candidates = parse_string_array(&extraction_response);

    if candidates.is_empty() {
        info!("Memory extraction: no new memories found for user {user_id}");
        return Ok(());
    }

    // 2. Deduplicate: find closest existing memory via embedding, then LLM decides
    let namespace = memory_namespace(user_id);
    let mut new_memories: Vec<String> = Vec::new();

    for candidate in &candidates {
        // Find the top 3 closest existing memories
        let matches = match qdrant.search_memories_scored(candidate, user_id, 3).await {
            Ok(results) => results,
            Err(e) => {
                error!("Memory dedup search failed: {e}");
                Vec::new()
            }
        };

        if matches.is_empty() {
            // No existing memories — keep
            new_memories.push(candidate.clone());
        } else {
            // Ask the LLM to compare against top 3 in one prompt
            let existing_list: String = matches
                .iter()
                .enumerate()
                .map(|(i, (text, _))| format!("{}. \"{}\"", i + 1, text))
                .collect::<Vec<_>>()
                .join("\n");

            let dedup_prompt = format!(
                "Does this new memory duplicate ANY of the existing ones below? \
                 Two memories are duplicates if they capture the same core fact, \
                 even if worded differently.\n\n\
                 New: \"{candidate}\"\n\n\
                 Existing:\n{existing_list}\n\n\
                 Reply with only \"duplicate\" or \"new\"."
            );
            match agents.prompt(&dedup_prompt).await {
                Ok(response) => {
                    let answer = response.trim().to_lowercase();
                    if answer.contains("duplicate") {
                        info!(
                            "Memory dedup: duplicate \"{}\"",
                            &candidate[..candidate.len().min(60)]
                        );
                    } else {
                        new_memories.push(candidate.clone());
                    }
                }
                Err(e) => {
                    error!("Memory dedup LLM failed: {e}");
                    new_memories.push(candidate.clone());
                }
            }
        }
    }

    if new_memories.is_empty() {
        info!("Memory extraction: all candidates were duplicates for user {user_id}");
        return Ok(());
    }

    // 4. Store each new memory in both the KV store and Qdrant
    let count = new_memories.len();
    for content in &new_memories {
        let key = uuid::Uuid::new_v4().to_string();
        let value = serde_json::json!({ "content": content });
        // Ignore store errors — Qdrant is the primary search index
        let _ = store.put(&namespace, &key, value);
        if let Err(e) = qdrant.upsert_memory(user_id, &key, content).await {
            error!("[memory] Failed to upsert memory to Qdrant: {:?}", e);
        }
    }

    info!("Memory extraction: stored {count} new memories for user {user_id}");
    Ok(())
}

/// Search for relevant memories from past conversations.
/// Tries Qdrant first; if empty, falls back to PostgresStore.
pub async fn retrieve_memories(
    query: &str,
    user_id: &str,
    qdrant: &VectorDatabaseClient,
    store: Option<&dyn Store>,
    top_k: u64,
) -> Vec<String> {
    // Try Qdrant first (semantic search)
    let results = qdrant
        .search_memories(query, user_id, top_k)
        .await
        .unwrap_or_default();

    if !results.is_empty() {
        return results;
    }

    // Fallback: load from PostgresStore (no semantic ranking, just all memories)
    if let Some(store) = store {
        let namespace = memory_namespace(user_id);
        if let Ok(items) = store.list(&StoreListQuery {
            namespace: Some(namespace),
            namespace_prefix: None,
            limit: Some(top_k as usize),
        }) {
            return items
                .iter()
                .filter_map(|item| {
                    item.value
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect();
        }
    }

    Vec::new()
}

/// Re-index all memories from PostgresStore into Qdrant.
/// Called on startup to backfill memories that failed to index previously.
pub async fn sync_store_to_qdrant(user_id: &str, store: &dyn Store, qdrant: &VectorDatabaseClient) {
    let namespace = memory_namespace(user_id);
    let items = match store.list(&StoreListQuery {
        namespace: Some(namespace),
        namespace_prefix: None,
        limit: Some(1000),
    }) {
        Ok(items) => items,
        Err(e) => {
            error!("[memory] Failed to list memories for backfill: {e}");
            return;
        }
    };

    if items.is_empty() {
        return;
    }

    let mut indexed = 0;
    for item in &items {
        let content = match item.value.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => continue,
        };

        if let Err(e) = qdrant.upsert_memory(user_id, &item.key, content).await {
            error!("[memory] Backfill upsert failed for key {}: {e}", item.key);
        } else {
            indexed += 1;
        }
    }

    if indexed > 0 {
        info!("[memory] Backfilled {indexed} memories to Qdrant for user {user_id}");
    }
}

/// Strip optional markdown code fences and parse a JSON array of strings.
fn parse_string_array(response: &str) -> Vec<String> {
    let trimmed = response.trim();

    // Strip ```json ... ``` or ``` ... ``` fences
    let json_str = if trimmed.starts_with("```") {
        let without_open = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_start();
        if let Some(close) = without_open.rfind("```") {
            without_open[..close].trim()
        } else {
            without_open.trim()
        }
    } else {
        trimmed
    };

    serde_json::from_str::<Vec<Value>>(json_str)
        .ok()
        .map(|arr| {
            arr.into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Returns the namespace path for a user's memories.
fn memory_namespace(user_id: &str) -> NamespacePath {
    vec!["memories".to_string(), user_id.to_string()]
}
