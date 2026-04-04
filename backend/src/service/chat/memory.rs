use anyhow::Result;
use langgraph::prelude::{NamespacePath, Store, StoreListQuery};
use log::{error, info};
use serde_json::Value;

use crate::service::agents::client::AgentsClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;

/// Extract memories from a conversation and store them in the key-value store and Qdrant.
pub async fn extract_and_store_memories(
    messages_json: &str,
    user_id: &str,
    _session_id: &str,
    agents: &AgentsClient,
    store: &dyn Store,
    qdrant: &VectorDatabaseClient,
) -> Result<()> {
    // 1. Call the LLM to extract new memories from the conversation
    let extraction_prompt = format!(
        "Review this trading assistant conversation and extract any NEW facts, preferences, \
         patterns, or instructions the user revealed. Only extract information that would be \
         useful in future conversations.\n\n\
         Return a JSON array of strings. Each string is one distinct memory. If nothing new \
         was revealed, return an empty array [].\n\n\
         Conversation:\n{messages_json}"
    );

    let extraction_response = agents.prompt(&extraction_prompt).await?;
    let candidates = parse_string_array(&extraction_response);

    if candidates.is_empty() {
        info!("Memory extraction: no new memories found for user {user_id}");
        return Ok(());
    }

    // 2. Load existing memories from the store
    let namespace = memory_namespace(user_id);
    let existing_items = store
        .list(&StoreListQuery {
            namespace: Some(namespace.clone()),
            namespace_prefix: None,
            limit: None,
        })
        .unwrap_or_default();

    // 3. Deduplicate against existing memories (if any)
    let new_memories: Vec<String> = if existing_items.is_empty() {
        candidates
    } else {
        let existing_texts: Vec<String> = existing_items
            .iter()
            .filter_map(|item| item.value.get("content").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();

        let existing_json = serde_json::to_string(&existing_texts).unwrap_or_else(|_| "[]".to_string());
        let candidates_json = serde_json::to_string(&candidates).unwrap_or_else(|_| "[]".to_string());

        let dedup_prompt = format!(
            "Here are the user's existing memories:\n{existing_json}\n\n\
             Here are candidate new memories:\n{candidates_json}\n\n\
             Return a JSON array containing ONLY the candidates that are genuinely new \
             information not already covered by existing memories. If all are duplicates, \
             return []."
        );

        let dedup_response = agents.prompt(&dedup_prompt).await?;
        parse_string_array(&dedup_response)
    };

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
                .filter_map(|item| item.value.get("content").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect();
        }
    }

    Vec::new()
}

/// Re-index all memories from PostgresStore into Qdrant.
/// Called on startup to backfill memories that failed to index previously.
pub async fn sync_store_to_qdrant(
    user_id: &str,
    store: &dyn Store,
    qdrant: &VectorDatabaseClient,
) {
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
