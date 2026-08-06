use std::sync::Arc;

use async_graphql::{Context, InputObject, Object, Result, SimpleObject, Subscription};
use clerk_rs::validators::authorizer::ClerkJwt;
use futures_util::StreamExt;
use langgraph::prelude::{CheckpointConfig, CheckpointSaver, Store};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;

use crate::service::{
    ai::chat::{
        agent,
        sessions::{ChatSession, ChatSessionStore},
        types::{
            ChatJobRegistry, ChatStreamEnvelope, ChatStreamKind, ChatStreamTx, DateRange,
            UserContext,
        },
    },
    ai::client::AgentsClient,
    ai::vector_database::client::VectorDatabaseClient,
    countly::Countly,
    db::Db,
    r2::R2Client,
    read_service::users::ensure_user,
};

async fn resolve_user(ctx: &Context<'_>) -> Result<(Arc<Db>, String)> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let db = ctx.data::<Arc<Db>>()?;
    let pool = db.pool();
    let full_name = jwt
        .other
        .get("full_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let email = jwt
        .other
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let user = ensure_user(pool, &jwt.sub, full_name, email).await?;
    Ok((db.clone(), user.id))
}

// --- GraphQL output types ---

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct GqlChatSession {
    pub id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ChatSession> for GqlChatSession {
    fn from(s: ChatSession) -> Self {
        Self {
            id: s.id,
            title: s.title,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct GqlChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub context_json: Option<String>,
    pub tool_name: Option<String>,
    pub created_at: String,
}

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct GqlChatStreamEvent {
    pub job_id: String,
    pub session_id: String,
    pub kind: String,
    pub content: Option<String>,
    pub tool_name: Option<String>,
    pub message_id: Option<String>,
}

impl From<ChatStreamEnvelope> for GqlChatStreamEvent {
    fn from(e: ChatStreamEnvelope) -> Self {
        Self {
            job_id: e.job_id,
            session_id: e.session_id,
            kind: e.kind.as_str().to_string(),
            content: e.content,
            tool_name: e.tool_name,
            message_id: e.message_id,
        }
    }
}

// --- Input types ---

#[derive(InputObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct DateRangeInput {
    pub from: String,
    pub to: String,
}

#[derive(InputObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct ChatContextInput {
    pub trade_ids: Option<Vec<String>>,
    pub date_range: Option<DateRangeInput>,
    pub playbook_ids: Option<Vec<String>>,
}

impl From<ChatContextInput> for UserContext {
    fn from(c: ChatContextInput) -> Self {
        Self {
            trade_ids: c.trade_ids,
            date_range: c.date_range.map(|dr| DateRange {
                from: dr.from,
                to: dr.to,
            }),
            playbook_ids: c.playbook_ids,
        }
    }
}

// --- Query ---

#[derive(Default)]
pub struct ChatQuery;

#[Object]
impl ChatQuery {
    async fn chat_sessions(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        limit: Option<i32>,
    ) -> Result<Vec<GqlChatSession>> {
        let (_db, user_id) = resolve_user(ctx).await?;
        let store = ctx.data::<Arc<ChatSessionStore>>()?;
        let limit = limit.unwrap_or(50) as i64;
        let sessions = store.list_sessions(&user_id, &workspace_id, limit).await?;
        Ok(sessions.into_iter().map(Into::into).collect())
    }

    async fn chat_messages(
        &self,
        ctx: &Context<'_>,
        session_id: String,
        limit: Option<i32>,
        _before: Option<String>,
    ) -> Result<Vec<GqlChatMessage>> {
        let (_db, _user_id) = resolve_user(ctx).await?;
        let checkpoint_saver = ctx.data::<Arc<dyn CheckpointSaver>>()?;
        let config = CheckpointConfig::new(session_id.clone());
        let limit = limit.unwrap_or(50) as usize;

        let checkpoint = checkpoint_saver
            .get(&config)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        let msgs = match checkpoint {
            None => Vec::new(),
            Some(cp) => {
                let messages_value = cp.channel_values.get("messages");
                match messages_value.and_then(|v| v.as_array()) {
                    None => Vec::new(),
                    Some(arr) => arr
                        .iter()
                        .enumerate()
                        .filter(|(_, msg)| {
                            msg.get("role")
                                .and_then(|r| r.as_str())
                                .map(|r| r != "system")
                                .unwrap_or(true)
                        })
                        .take(limit)
                        .map(|(idx, msg)| GqlChatMessage {
                            id: format!("{}-{}", session_id, idx),
                            session_id: session_id.clone(),
                            role: msg
                                .get("role")
                                .and_then(|r| r.as_str())
                                .unwrap_or("")
                                .to_string(),
                            content: msg
                                .get("content")
                                .and_then(|c| c.as_str())
                                .unwrap_or("")
                                .to_string(),
                            context_json: None,
                            tool_name: msg
                                .get("name")
                                .and_then(|n| n.as_str())
                                .map(|s| s.to_string()),
                            created_at: String::new(),
                        })
                        .collect(),
                }
            }
        };

        Ok(msgs)
    }
}

// --- Mutation ---

#[derive(Default)]
pub struct ChatMutation;

#[Object]
impl ChatMutation {
    async fn create_chat_session(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<GqlChatSession> {
        let (_db, user_id) = resolve_user(ctx).await?;
        let store = ctx.data::<Arc<ChatSessionStore>>()?;
        let session = store.create_session(&user_id, &workspace_id).await?;
        Ok(session.into())
    }

    async fn update_chat_session(
        &self,
        ctx: &Context<'_>,
        session_id: String,
        title: String,
    ) -> Result<GqlChatSession> {
        let (_db, _user_id) = resolve_user(ctx).await?;
        let store = ctx.data::<Arc<ChatSessionStore>>()?;
        let session = store.update_session_title(&session_id, &title).await?;
        Ok(session.into())
    }

    async fn delete_chat_session(&self, ctx: &Context<'_>, session_id: String) -> Result<bool> {
        let (_db, user_id) = resolve_user(ctx).await?;
        let store = ctx.data::<Arc<ChatSessionStore>>()?;
        let deleted = store.delete_session(&session_id, &user_id).await?;
        Ok(deleted)
    }

    async fn send_chat_message(
        &self,
        ctx: &Context<'_>,
        session_id: String,
        content: String,
        context: Option<ChatContextInput>,
    ) -> Result<String> {
        let (db, user_id) = resolve_user(ctx).await?;
        let agents = ctx.data::<Arc<AgentsClient>>()?.clone();
        let qdrant = ctx.data::<Arc<VectorDatabaseClient>>()?.clone();
        let r2 = ctx.data::<Arc<R2Client>>()?.clone();
        let chat_jobs = ctx.data::<ChatJobRegistry>()?.clone();
        let checkpoint_saver: Arc<dyn CheckpointSaver> =
            ctx.data::<Arc<dyn CheckpointSaver>>()?.clone();
        let memory_store: Option<Arc<dyn Store>> = ctx.data::<Arc<dyn Store>>().ok().cloned();
        let session_store = ctx.data::<Arc<ChatSessionStore>>()?.clone();
        let countly = ctx.data::<Arc<Countly>>().ok().cloned();
        let clerk_id = ctx.data::<ClerkJwt>()?.sub.clone();

        // Resolve session to get workspace_id
        let session = session_store.get_session(&session_id).await?;
        let workspace_id = session.workspace_id.clone();

        let job_id = Uuid::new_v4().to_string();
        let user_context: Option<UserContext> = context.map(Into::into);

        // Per-job buffered channel. The receiver is parked in the registry until
        // the client's `chatStream` subscription attaches and drains it. Because
        // it's an unbounded mpsc, events the agent emits before the subscription
        // connects are buffered (not dropped) — so the agent runs immediately
        // with no artificial startup delay and no lost first tokens.
        let (tx, rx): (ChatStreamTx, _) = mpsc::unbounded_channel();
        chat_jobs.lock().unwrap().insert(job_id.clone(), rx);

        let job_id_clone = job_id.clone();
        let job_id_cleanup = job_id.clone();
        let tx_err = tx.clone();
        let job_id_err = job_id.clone();
        let session_id_err = session_id.clone();
        tokio::spawn(async move {
            match agent::run_chat_agent(
                session_id,
                job_id_clone,
                content,
                user_context,
                user_id,
                workspace_id,
                agents,
                db,
                qdrant,
                r2,
                tx,
                checkpoint_saver,
                memory_store,
                session_store,
            )
            .await
            {
                Ok(()) => {
                    if let Some(countly) = countly {
                        countly
                            .capture(&clerk_id, "ai_chat_completed", serde_json::json!({}))
                            .await;
                    }
                }
                Err(e) => {
                    log::error!("Chat agent error: {e}");
                    let _ = tx_err.send(ChatStreamEnvelope {
                        job_id: job_id_err,
                        session_id: session_id_err,
                        kind: ChatStreamKind::Error,
                        content: Some(format!("{e}")),
                        tool_name: None,
                        message_id: None,
                    });
                }
            }
            // Drop the last sender so the subscription stream closes promptly
            // once the client has drained the buffered events (otherwise this
            // clone would hold it open for the whole cleanup window below).
            drop(tx_err);
            // Reclaim the receiver if the client never subscribed (e.g. it
            // navigated away). Harmless if the subscription already took it.
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            chat_jobs.lock().unwrap().remove(&job_id_cleanup);
        });

        Ok(job_id)
    }
}

// --- Subscription ---

#[derive(Default)]
pub struct ChatSubscription;

#[Subscription]
impl ChatSubscription {
    async fn chat_stream(
        &self,
        ctx: &Context<'_>,
        job_id: String,
    ) -> Result<impl futures_util::Stream<Item = GqlChatStreamEvent>> {
        let chat_jobs = ctx.data_unchecked::<ChatJobRegistry>();
        // Take this job's receiver. If it's missing (already consumed by a prior
        // subscribe, or unknown id), hand back an immediately-empty stream by
        // dropping a throwaway sender.
        let rx = chat_jobs
            .lock()
            .unwrap()
            .remove(&job_id)
            .unwrap_or_else(|| {
                let (_tx, rx) = mpsc::unbounded_channel();
                rx
            });
        Ok(UnboundedReceiverStream::new(rx).map(GqlChatStreamEvent::from))
    }
}
