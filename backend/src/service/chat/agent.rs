use anyhow::Result;
use langgraph::prelude::CheckpointSaver;
use log::{error, info};
use serde_json::json;
use std::sync::Arc;

use crate::service::agents::client::AgentsClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;
use crate::service::chat::sessions;
use crate::service::chat::graph::{self, GraphDeps};
use crate::service::chat::types::*;
use crate::service::turso::TursoClient;

const SYSTEM_PROMPT: &str = r#"You are a trading assistant for Tradstry. You help users analyze their trading performance, find patterns in their trades, and answer questions about their trading journal, playbooks, and statistics.

You have access to three tools:
- db_query: Query specific trades, journal entries, or playbook rules from the database
- semantic_search: Search across all trading data using natural language (good for finding patterns, themes, similar trades)
- analytics_calc: Calculate performance metrics like win rate, P&L, profit factor, streaks, and per-symbol breakdowns

When the user provides context (pinned trades, date ranges, playbooks), use that to scope your queries. Be specific and data-driven in your responses. Format numbers clearly.

Formatting rules:
- Never use markdown bold (**text**), italic (*text*), or heading (###) syntax.
- Never use markdown tables (|---|). Instead, list each metric on its own line as "Label: value".
- Never use em dashes or en dashes. Use hyphens (-) instead.
- Use short section titles on their own line, followed by the content below.
- Use numbered lists for steps and bullet lists (-) for breakdowns.
- Keep responses concise and conversational.

Example format for metrics:
Recent-Trade Performance Summary

Total P&L: +$25.00
Win rate: 100% (1 win, 0 losses)
Average R: 10.0 R
Profit Factor: N/A - only winning trades

Per-symbol breakdown
- AAPL - 1 trade, +$25.00, 100% win-rate

What the numbers mean
- Very high R-multiple indicates the trade far exceeded its target.
- Win-rate of 100% looks great, but based on a single trade."#;

fn build_system_prompt(user_context: &Option<UserContext>) -> String {
    let mut prompt = SYSTEM_PROMPT.to_owned();

    if let Some(ctx) = user_context {
        prompt.push_str("\n\n## User Context\n");

        if let Some(trade_ids) = &ctx.trade_ids {
            if !trade_ids.is_empty() {
                prompt.push_str(&format!("Pinned trade IDs: {}\n", trade_ids.join(", ")));
            }
        }

        if let Some(date_range) = &ctx.date_range {
            prompt.push_str(&format!(
                "Date range: {} to {}\n",
                date_range.from, date_range.to
            ));
        }

        if let Some(playbook_ids) = &ctx.playbook_ids {
            if !playbook_ids.is_empty() {
                prompt.push_str(&format!(
                    "Playbook IDs: {}\n",
                    playbook_ids.join(", ")
                ));
            }
        }
    }

    prompt
}

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
    tx: ChatEventBus,
    checkpoint_saver: Arc<dyn CheckpointSaver>,
) -> Result<()> {
    // 1. Check if this is the first turn by looking at existing checkpoint
    let config = langgraph::prelude::CheckpointConfig::new(&session_id);
    let existing_checkpoint = checkpoint_saver.get(&config).ok().flatten();
    let is_first_turn = existing_checkpoint
        .as_ref()
        .and_then(|cp| cp.channel_values.get("messages"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("user")).count() == 0)
        .unwrap_or(true);

    // 2. Build system prompt
    let system_prompt = build_system_prompt(&user_context);

    // 3. Build GraphDeps
    let deps = Arc::new(GraphDeps {
        agents: Arc::clone(&agents),
        turso: Arc::clone(&turso),
        qdrant: Arc::clone(&qdrant),
        tx,
        job_id,
        session_id: session_id.clone(),
        user_id,
        account_id,
        system_prompt,
    });

    // 4. Compile the graph
    let compiled = graph::build_chat_graph(deps)
        .map_err(|e| anyhow::anyhow!("Failed to build chat graph: {e:?}"))?;

    // 5. Create the user message value
    let user_msg = json!({"role": "user", "content": user_message});

    // 6. Run the graph
    let summary = graph::run_chat_graph(
        &compiled,
        checkpoint_saver.as_ref(),
        &session_id,
        user_msg,
    )
    .map_err(|e| anyhow::anyhow!("Graph execution error: {e:?}"))?;

    // 7. Log result
    info!(
        "Chat graph completed: status={:?}, steps={}, tasks={}",
        summary.status,
        summary.steps_executed,
        summary.tasks_executed,
    );

    // 8. Touch session updated_at
    let conn = turso.get_connection()?;
    if let Err(e) = sessions::touch_session_updated_at(&conn, &session_id).await {
        error!("Failed to touch session updated_at: {e}");
    }

    // 9. Title generation — fire and forget on first turn
    if is_first_turn {
        let agents_clone = Arc::clone(&agents);
        let turso_clone = Arc::clone(&turso);
        let session_id_clone = session_id.clone();
        let user_message_clone = user_message.clone();

        tokio::spawn(async move {
            let title_prompt = format!(
                "Generate a short title (5 words max) for a trading assistant conversation that starts with this message: \"{}\". \
                 Respond with only the title, no quotes or punctuation.",
                user_message_clone
            );

            let title = match agents_clone.prompt(&title_prompt).await {
                Ok(t) => {
                    let trimmed = t.trim().to_owned();
                    if trimmed.is_empty() {
                        user_message_clone.chars().take(50).collect::<String>()
                    } else {
                        trimmed
                    }
                }
                Err(e) => {
                    error!("Title generation failed: {e}");
                    user_message_clone.chars().take(50).collect::<String>()
                }
            };

            match turso_clone.get_connection() {
                Ok(conn) => {
                    if let Err(e) =
                        sessions::update_session_title(&conn, &session_id_clone, &title).await
                    {
                        error!("Failed to update session title: {e}");
                    }
                }
                Err(e) => {
                    error!("Failed to get connection for title update: {e}");
                }
            }
        });
    }

    Ok(())
}
