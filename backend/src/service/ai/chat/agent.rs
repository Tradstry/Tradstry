use anyhow::Result;
use langgraph::prelude::{CheckpointSaver, Store};
use log::{error, info};
use serde_json::json;
use std::sync::Arc;

use crate::service::ai::chat::graph::{self, GraphDeps};
use crate::service::ai::chat::sessions::ChatSessionStore;
use crate::service::ai::chat::types::*;
use crate::service::ai::client::AgentsClient;
use crate::service::ai::vector_database::client::VectorDatabaseClient;
use crate::service::r2::R2Client;
use crate::service::turso::TursoClient;

/// Retry an LLM prompt up to `retries` extra times on failure, with a short backoff.
/// Per-model failover is handled inside `AgentsClient::prompt`; this loop covers the
/// case where the entire model strategy fails (e.g. all models cooled down).
async fn prompt_with_retry(agents: &AgentsClient, prompt: &str, retries: u32) -> Result<String> {
    let mut last_err = None;
    for attempt in 0..=retries {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
        }
        match agents.prompt(prompt).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                log::warn!("LLM prompt attempt {} failed: {e}", attempt + 1);
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("prompt_with_retry: no attempts made")))
}

const SYSTEM_PROMPT: &str = r#"You are a trading assistant for Tradstry. You help users analyze their trading performance, find patterns, and answer questions about their journal, playbooks, and statistics.

## Response framework

Follow this for every interaction:
1. Understand what the user actually needs - not just what they literally asked. "How am I doing?" means performance metrics, not a generic pep talk.
2. Scope using context - if the user pinned trades, set a date range, or selected a playbook, always use that to narrow your queries.
3. Pick the simplest tool - one focused tool call beats chaining three. If analytics_calc answers it, don't also run research.
4. Be specific with numbers - say "+$142.50 across 3 trades" not "you did well." Include win rate, P&L, and R-multiples when relevant.
5. Be honest about limited data - if there's only 1 trade or a short time window, say so. Don't overstate conclusions from thin data.

## When to use each tool

Use the simplest tool that answers the question. Don't chain tools when one will do.

<data_retrieval>
db_query - Fetch specific records by known criteria (a symbol, date, trade ID, journal entry). Use when the user asks about concrete data: "show my AAPL trades", "what did I journal last Monday?"
semantic_search - Find trades or notes by meaning, not exact match. Use when the user describes something loosely: "trades where I held too long", "my best setups this month"
analytics_calc - Compute metrics: win rate, P&L, profit factor, streaks, per-symbol breakdowns. Use when the user asks "how am I doing?" or "what's my win rate on TSLA?"
</data_retrieval>

<research_and_reporting>
research - Deep dive into a topic requiring multiple data pulls. Use for: "analyze my AAPL trades", "what patterns do you see in my losses?" Accepts query, optional symbol, optional date range.
report - Generate a structured performance report for a date range. Use for: "give me a weekly review", "how did I do in March?" Requires date_from and date_to.
comparison - Side-by-side trade comparison. Use for: "compare these two trades", "what's the difference between my wins and losses?" Accepts query and optional trade_ids.
</research_and_reporting>

<memory>
recall_memory - Search your memory of past conversations and the current session. Use when the user references something from before: "remember when I said...", "what's my name?", "what did we talk about last time?"
</memory>

<custom_agents>
create_agent - Start building a reusable automated workflow. Use when: "create an agent that..."
save_agent - Save a completed agent definition. Only call when all required fields are gathered.
run_agent - Run a saved agent by name. Use when: "run my [agent name]"
edit_agent - Modify an existing agent's name, goal, data sources, symbol, or output style.
</custom_agents>

## Output formatting

Never use markdown bold, italic, heading, or table syntax. Write in plain text.
Use short section titles on their own line, with content below.
Use numbered lists for steps, bullet lists (-) for breakdowns.
Write "Label: value" for metrics, one per line.
Keep responses concise and conversational.

<example>
User asks: "how did my recent trades go?"

Recent-Trade Performance Summary

Total P&L: +$25.00
Win rate: 100% (1 win, 0 losses)
Average R: 10.0 R
Profit Factor: N/A - only winning trades

Per-symbol breakdown
- AAPL - 1 trade, +$25.00, 100% win-rate

What the numbers mean
- Very high R-multiple indicates the trade far exceeded its target.
- Win-rate of 100% looks great, but based on a single trade.
</example>"#;

fn build_system_prompt(user_context: &Option<UserContext>) -> String {
    let mut prompt = SYSTEM_PROMPT.to_owned();

    if let Some(ctx) = user_context {
        prompt.push_str("\n\n## User Context\n");

        if let Some(trade_ids) = &ctx.trade_ids
            && !trade_ids.is_empty()
        {
            prompt.push_str(&format!("Pinned trade IDs: {}\n", trade_ids.join(", ")));
        }

        if let Some(date_range) = &ctx.date_range {
            prompt.push_str(&format!(
                "Date range: {} to {}\n",
                date_range.from, date_range.to
            ));
        }

        if let Some(playbook_ids) = &ctx.playbook_ids
            && !playbook_ids.is_empty()
        {
            prompt.push_str(&format!("Playbook IDs: {}\n", playbook_ids.join(", ")));
        }
    }

    prompt
}

#[allow(clippy::too_many_arguments)]
pub async fn run_chat_agent(
    session_id: String,
    job_id: String,
    user_message: String,
    user_context: Option<UserContext>,
    user_id: String,
    account_id: String,
    agents: Arc<AgentsClient>,
    turso: Arc<TursoClient>,
    qdrant: Arc<VectorDatabaseClient>,
    r2: Arc<R2Client>,
    tx: ChatStreamTx,
    checkpoint_saver: Arc<dyn CheckpointSaver>,
    memory_store: Option<Arc<dyn Store>>,
    session_store: Arc<ChatSessionStore>,
) -> Result<()> {
    // 1. Check if this is the first turn by looking at existing checkpoint
    let config = langgraph::prelude::CheckpointConfig::new(&session_id);
    let existing_checkpoint = checkpoint_saver.get(&config).ok().flatten();
    let is_first_turn = existing_checkpoint
        .as_ref()
        .and_then(|cp| cp.channel_values.get("messages"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
                .count()
                == 0
        })
        .unwrap_or(true);

    // 2. Build system prompt
    let system_prompt = build_system_prompt(&user_context);

    // 2b. Retrieve relevant memories and inject into system prompt
    let store_ref = memory_store.as_ref().map(|s| s.as_ref());
    let memories = crate::service::ai::chat::memory::retrieve_memories(
        &user_message,
        &user_id,
        &qdrant,
        store_ref,
        10,
    )
    .await;

    // 2c. If memories came from store fallback (Qdrant was empty), backfill Qdrant
    //     so recall_memory can find them. Run it in the background — the memories
    //     are already injected into the system prompt below, so the response need
    //     not wait on this (it only helps a same-turn recall_memory tool call,
    //     which is rare).
    if !memories.is_empty()
        && let Some(ref store) = memory_store
    {
        let store = Arc::clone(store);
        let qdrant_bg = Arc::clone(&qdrant);
        let user_id_bg = user_id.clone();
        tokio::spawn(async move {
            crate::service::ai::chat::memory::sync_store_to_qdrant(
                &user_id_bg,
                store.as_ref(),
                &qdrant_bg,
            )
            .await;
        });
    }

    let system_prompt = if memories.is_empty() {
        system_prompt
    } else {
        let memory_section = memories
            .iter()
            .map(|m| format!("- {m}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{system_prompt}\n\n## What I Remember About You\n{memory_section}")
    };

    // 3. Build GraphDeps
    //    Clone tx + job_id before moving into deps so we can broadcast Done
    //    ourselves after the graph run (and its checkpoint write) has fully
    //    landed — see step 8.
    let user_id_for_extraction = user_id.clone();
    let tx_for_done = tx.clone();
    let job_id_for_done = job_id.clone();
    let session_id_for_done = session_id.clone();
    let deps = Arc::new(GraphDeps {
        agents: Arc::clone(&agents),
        turso: Arc::clone(&turso),
        qdrant: Arc::clone(&qdrant),
        r2: Arc::clone(&r2),
        tx,
        job_id,
        session_id: session_id.clone(),
        user_id,
        account_id,
        system_prompt,
    });

    // 4. Compile the graph
    let compiled = graph::build_chat_graph(deps, Some(checkpoint_saver.clone()))
        .map_err(|e| anyhow::anyhow!("Failed to build chat graph: {e:?}"))?;

    // 5. Create the user message value
    let user_msg = json!({"role": "user", "content": user_message});

    // 6. Run the graph
    let summary =
        graph::run_chat_graph(&compiled, checkpoint_saver.as_ref(), &session_id, user_msg)
            .map_err(|e| anyhow::anyhow!("Graph execution error: {e:?}"))?;

    // 7. Log result
    info!(
        "Chat graph completed: status={:?}, steps={}, tasks={}",
        summary.status, summary.steps_executed, summary.tasks_executed,
    );

    // 8. Broadcast Done now that the graph has terminated and the checkpoint
    //    is durably persisted. Any chatMessages refetch the client fires in
    //    response to this event is guaranteed to read the new assistant
    //    message (previously this fired from inside the llm node, before the
    //    channel write was applied and the checkpoint committed).
    let _ = tx_for_done.send(ChatStreamEnvelope {
        job_id: job_id_for_done,
        session_id: session_id_for_done,
        kind: ChatStreamKind::Done,
        content: None,
        tool_name: None,
        message_id: Some(uuid::Uuid::new_v4().to_string()),
    });

    // 9. Touch session updated_at
    if let Err(e) = session_store.touch_session_updated_at(&session_id).await {
        error!("Failed to touch session updated_at: {e}");
    }

    // 9. Background tasks — title generation + memory extraction
    //    Run sequentially in one spawn to avoid concurrent LLM rate limits.
    {
        let do_title = is_first_turn;
        let agents_clone = Arc::clone(&agents);
        let session_store_clone = Arc::clone(&session_store);
        let qdrant_clone = Arc::clone(&qdrant);
        let session_id_clone = session_id.clone();
        let user_id_clone = user_id_for_extraction.clone();
        let user_message_clone = user_message.clone();
        let memory_store_clone = memory_store.clone();

        let messages_json = summary
            .checkpoint
            .channel_values
            .get("messages")
            .map(|v| serde_json::to_string(v).unwrap_or_default())
            .unwrap_or_default();

        tokio::spawn(async move {
            // Title generation first (fast, single LLM call)
            if do_title {
                let title_prompt = format!(
                    "Generate a short title (5 words max) for a trading assistant conversation \
                     that starts with this message: \"{}\". \
                     Respond with only the title, no quotes or punctuation.",
                    user_message_clone
                );

                let title = match prompt_with_retry(&agents_clone, &title_prompt, 2).await {
                    Ok(t) => {
                        let trimmed = t.trim().to_owned();
                        if trimmed.is_empty() {
                            user_message_clone.chars().take(50).collect::<String>()
                        } else {
                            trimmed
                        }
                    }
                    Err(e) => {
                        error!("Title generation failed after retries: {e}");
                        user_message_clone.chars().take(50).collect::<String>()
                    }
                };

                if let Err(e) = session_store_clone
                    .update_session_title(&session_id_clone, &title)
                    .await
                {
                    error!("Failed to update session title: {e}");
                }
            }

            // Memory extraction second (multiple LLM calls, heavier)
            #[allow(clippy::collapsible_if)]
            if let Some(store) = memory_store_clone
                && !messages_json.is_empty()
            {
                if let Err(e) = crate::service::ai::chat::memory::extract_and_store_memories(
                    &messages_json,
                    &user_id_clone,
                    &session_id_clone,
                    &agents_clone,
                    store.as_ref(),
                    &qdrant_clone,
                )
                .await
                {
                    error!("Memory extraction failed: {e}");
                }
            }
        });
    }

    Ok(())
}
