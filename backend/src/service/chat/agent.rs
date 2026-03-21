use anyhow::Result;
use log::{error, info};
use std::sync::Arc;

use crate::service::agents::client::AgentsClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;
use crate::service::chat::messages;
use crate::service::chat::sessions;
use crate::service::chat::tools;
use crate::service::chat::types::*;
use crate::service::turso::TursoClient;

const MAX_ITERATIONS: u32 = 5;
const MAX_USER_TURNS: i64 = 5;

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
) -> Result<()> {
    // 1. Get DB connection
    let conn = turso.get_connection()?;

    // 2. Persist user message
    messages::insert_message(&conn, &session_id, "user", &user_message, None, None).await?;

    // 3. Load conversation history
    let history = messages::load_recent_turns(&conn, &session_id, MAX_USER_TURNS).await?;

    // 4. Build Groq messages array
    let system_prompt = build_system_prompt(&user_context);
    let mut groq_messages: Vec<GroqMessage> = vec![GroqMessage {
        role: "system".to_owned(),
        content: Some(system_prompt),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];

    // Convert history messages to GroqMessage format
    for msg in &history {
        groq_messages.push(GroqMessage {
            role: msg.role.clone(),
            content: Some(msg.content.clone()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
    }

    // 5. ReAct loop
    let tool_defs = tools::tool_schemas();
    let mut iteration_count: u32 = 0;
    let mut retry_count: u32 = 0;

    loop {
        // If we've hit max iterations, call without tools to force a final answer
        let tools_param = if iteration_count >= MAX_ITERATIONS {
            None
        } else {
            Some(tool_defs.as_slice())
        };

        let response = agents
            .stream_chat(&groq_messages, tools_param, tx.clone(), &job_id, &session_id)
            .await;

        match response {
            Ok(GroqChatResponse::ToolCall { id, name, arguments }) => {
                info!(
                    "Agent tool call: {} (iteration {}/{})",
                    name, iteration_count, MAX_ITERATIONS
                );

                // Execute the tool
                let tool_result = tools::execute_tool(
                    &name,
                    &arguments,
                    &user_id,
                    &account_id,
                    &turso,
                    &qdrant,
                )
                .await
                .unwrap_or_else(|e| format!("Tool error: {e}"));

                // Broadcast tool result to frontend
                let _ = tx.send(ChatStreamEnvelope {
                    job_id: job_id.clone(),
                    session_id: session_id.clone(),
                    kind: ChatStreamKind::ToolResult,
                    content: Some(tool_result.clone()),
                    tool_name: Some(name.clone()),
                    message_id: None,
                });

                // Push assistant tool_call message
                groq_messages.push(GroqMessage {
                    role: "assistant".to_owned(),
                    content: None,
                    tool_calls: Some(vec![GroqToolCall {
                        id: id.clone(),
                        call_type: "function".to_owned(),
                        function: GroqFunctionCall {
                            name: name.clone(),
                            arguments: arguments.clone(),
                        },
                    }]),
                    tool_call_id: None,
                    name: None,
                });

                // Push tool result message
                groq_messages.push(GroqMessage {
                    role: "tool".to_owned(),
                    content: Some(tool_result),
                    tool_calls: None,
                    tool_call_id: Some(id),
                    name: Some(name),
                });

                iteration_count += 1;
                retry_count = 0;
            }

            Ok(GroqChatResponse::TextComplete { full_text }) => {
                info!("Agent completed after {} tool calls", iteration_count);

                // Persist assistant message
                let msg = messages::insert_message(
                    &conn,
                    &session_id,
                    "assistant",
                    &full_text,
                    None,
                    None,
                )
                .await?;

                // Send Done event
                let _ = tx.send(ChatStreamEnvelope {
                    job_id: job_id.clone(),
                    session_id: session_id.clone(),
                    kind: ChatStreamKind::Done,
                    content: None,
                    tool_name: None,
                    message_id: Some(msg.id),
                });

                break;
            }

            Err(e) => {
                error!("Agent stream error: {e}");

                if retry_count == 0 {
                    // Retry once after 1s
                    retry_count += 1;
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }

                // Retry failed — send error event and break
                let _ = tx.send(ChatStreamEnvelope {
                    job_id: job_id.clone(),
                    session_id: session_id.clone(),
                    kind: ChatStreamKind::Error,
                    content: Some(format!("Agent error: {e}")),
                    tool_name: None,
                    message_id: None,
                });

                break;
            }
        }
    }

    // 6. Title generation — fire and forget on first message
    if history.len() <= 1 {
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
