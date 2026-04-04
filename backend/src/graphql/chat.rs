use std::sync::Arc;

use async_graphql::{Context, InputObject, Object, Result, SimpleObject, Subscription};
use langgraph::prelude::{CheckpointConfig, CheckpointSaver, Store};
use clerk_rs::validators::authorizer::ClerkJwt;
use futures_util::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

use crate::service::{
    agents::client::AgentsClient,
    agents::vector_database::client::VectorDatabaseClient,
    chat::{
        agent,
        sessions,
        types::{ChatEventBus, ChatStreamEnvelope, ChatStreamKind, DateRange, UserContext},
    },
    read_service::users::ensure_user,
    turso::TursoClient,
};

async fn resolve_user(ctx: &Context<'_>) -> Result<(Arc<TursoClient>, String)> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let turso = ctx.data::<Arc<TursoClient>>()?;
    let conn = turso.get_connection()?;
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
    let user = ensure_user(&conn, &jwt.sub, full_name, email).await?;
    Ok((turso.clone(), user.id))
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

impl From<sessions::ChatSession> for GqlChatSession {
    fn from(s: sessions::ChatSession) -> Self {
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
        account_id: String,
        limit: Option<i32>,
    ) -> Result<Vec<GqlChatSession>> {
        let (turso, user_id) = resolve_user(ctx).await?;
        let conn = turso.get_connection()?;
        let limit = limit.unwrap_or(50) as i64;
        let sessions = sessions::list_sessions(&conn, &user_id, &account_id, limit).await?;
        Ok(sessions.into_iter().map(Into::into).collect())
    }

    async fn chat_messages(
        &self,
        ctx: &Context<'_>,
        session_id: String,
        limit: Option<i32>,
        _before: Option<String>,
    ) -> Result<Vec<GqlChatMessage>> {
        let (_turso, _user_id) = resolve_user(ctx).await?;
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
        account_id: String,
    ) -> Result<GqlChatSession> {
        let (turso, user_id) = resolve_user(ctx).await?;
        let conn = turso.get_connection()?;
        let session = sessions::create_session(&conn, &user_id, &account_id).await?;
        Ok(session.into())
    }

    async fn update_chat_session(
        &self,
        ctx: &Context<'_>,
        session_id: String,
        title: String,
    ) -> Result<GqlChatSession> {
        let (turso, _user_id) = resolve_user(ctx).await?;
        let conn = turso.get_connection()?;
        let session = sessions::update_session_title(&conn, &session_id, &title).await?;
        Ok(session.into())
    }

    async fn delete_chat_session(
        &self,
        ctx: &Context<'_>,
        session_id: String,
    ) -> Result<bool> {
        let (turso, user_id) = resolve_user(ctx).await?;
        let conn = turso.get_connection()?;
        let deleted = sessions::delete_session(&conn, &session_id, &user_id).await?;
        Ok(deleted)
    }

    async fn send_chat_message(
        &self,
        ctx: &Context<'_>,
        session_id: String,
        content: String,
        context: Option<ChatContextInput>,
    ) -> Result<String> {
        let (turso, user_id) = resolve_user(ctx).await?;
        let agents = ctx.data::<Arc<AgentsClient>>()?.clone();
        let qdrant = ctx.data::<Arc<VectorDatabaseClient>>()?.clone();
        let tx = ctx.data::<ChatEventBus>()?.clone();
        let checkpoint_saver: Arc<dyn CheckpointSaver> = ctx.data::<Arc<dyn CheckpointSaver>>()?.clone();
        let memory_store: Option<Arc<dyn Store>> =
            ctx.data::<Arc<dyn Store>>().ok().cloned();

        // Resolve session to get account_id
        let conn = turso.get_connection()?;
        let session = sessions::get_session(&conn, &session_id).await?;
        let account_id = session.account_id.clone();

        let job_id = Uuid::new_v4().to_string();
        let user_context: Option<UserContext> = context.map(Into::into);

        let job_id_clone = job_id.clone();
        let tx_err = tx.clone();
        let job_id_err = job_id.clone();
        tokio::spawn(async move {
            if let Err(e) = agent::run_chat_agent(
                session_id.clone(),
                job_id_clone,
                content,
                user_context,
                user_id,
                account_id,
                agents,
                turso,
                qdrant,
                tx,
                checkpoint_saver,
                memory_store,
            )
            .await
            {
                log::error!("Chat agent error: {e}");
                let _ = tx_err.send(ChatStreamEnvelope {
                    job_id: job_id_err,
                    session_id,
                    kind: ChatStreamKind::Error,
                    content: Some(format!("{e}")),
                    tool_name: None,
                    message_id: None,
                });
            }
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
        let event_bus = ctx.data_unchecked::<ChatEventBus>().clone();
        Ok(
            BroadcastStream::new(event_bus.subscribe()).filter_map(move |item| {
                let job_id = job_id.clone();
                async move {
                    match item.ok() {
                        Some(envelope) if envelope.job_id == job_id => {
                            Some(GqlChatStreamEvent::from(envelope))
                        }
                        _ => None,
                    }
                }
            }),
        )
    }
}
