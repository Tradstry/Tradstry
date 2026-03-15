use async_graphql::{Context, Result, SimpleObject, Subscription};
use futures_util::{stream, Stream};
use std::pin::Pin;

use crate::service::ai::ai_chat;
use crate::service::turso::TursoClient;
use crate::service::agents::AgentsClient;
use clerk_rs::validators::authorizer::ClerkJwt;
use crate::graphql::ai_chat::AiChatInput;

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct AiChatStreamPayload {
    pub event: String,
    pub request_id: String,
    pub thread_id: String,
    pub text: String,
    pub promoted_memory_uris: Vec<String>,
    pub message: String,
}

#[derive(Default)]
pub struct AiChatSubscription;

fn to_payload(event: ai_chat::AiChatStreamEvent) -> AiChatStreamPayload {
    match event {
        ai_chat::AiChatStreamEvent::Delta {
            request_id,
            thread_id,
            text,
        } => AiChatStreamPayload {
            event: "delta".to_string(),
            request_id,
            thread_id,
            text,
            promoted_memory_uris: Vec::new(),
            message: String::new(),
        },
        ai_chat::AiChatStreamEvent::Completed {
            request_id,
            thread_id,
            text,
            promoted_memory_uris,
        } => AiChatStreamPayload {
            event: "completed".to_string(),
            request_id,
            thread_id,
            text,
            promoted_memory_uris,
            message: String::new(),
        },
        ai_chat::AiChatStreamEvent::Error {
            request_id,
            thread_id,
            message,
        } => AiChatStreamPayload {
            event: "error".to_string(),
            request_id,
            thread_id,
            text: String::new(),
            promoted_memory_uris: Vec::new(),
            message,
        },
    }
}

async fn get_user_id_from_ctx(ctx: &Context<'_>) -> Result<String> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let turso = ctx.data::<std::sync::Arc<TursoClient>>()?;
    let conn = turso.get_connection()?;

    let full_name = jwt
        .other
        .get("full_name")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let email = jwt
        .other
        .get("email")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    let user = crate::service::read_service::users::ensure_user(
        &conn,
        &jwt.sub,
        full_name,
        email,
    )
    .await?;
    Ok(user.id)
}

#[Subscription]
impl AiChatSubscription {
    async fn ai_chat_stream(
        &self,
        ctx: &Context<'_>,
        input: AiChatInput,
    ) -> Result<Pin<
        Box<dyn Stream<Item = AiChatStreamPayload> + Send>,
    >> {
        let user_id = get_user_id_from_ctx(ctx).await?;
        let agents_client = ctx.data::<Option<AgentsClient>>()?.clone();
        let turso = ctx.data::<std::sync::Arc<TursoClient>>()?.clone();

        let receiver = ai_chat::stream_chat_events(
            &agents_client,
            &turso,
            &user_id,
            input.thread_id,
            input.message,
        )
        .await?;

        let stream = stream::unfold(receiver, |mut receiver| async move {
            match receiver.recv().await {
                Some(event) => Some((to_payload(event), receiver)),
                None => None,
            }
        });

        Ok(Box::pin(stream))
    }
}
