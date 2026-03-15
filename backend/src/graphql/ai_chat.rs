use async_graphql::{Context, InputObject, Object, Result, SimpleObject};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::ai::ai_chat;
use crate::service::read_service::users::ensure_user;
use crate::service::turso::schema::tables::ai_chat_table;
use crate::service::turso::TursoClient;

async fn get_user_id_from_ctx(ctx: &Context<'_>) -> Result<String> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let turso = ctx.data::<Arc<TursoClient>>()?;
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

    let user = ensure_user(&conn, &jwt.sub, full_name, email).await?;
    Ok(user.id)
}

#[derive(InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct AiChatInput {
    pub message: String,
    pub thread_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct AiChatResponse {
    pub request_id: String,
    pub thread_id: String,
    pub text: String,
    pub promoted_memory_uris: Vec<String>,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct AiChatThread {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct AiChatMessage {
    pub id: String,
    pub thread_id: String,
    pub request_id: Option<String>,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct DeleteAiChatThreadInput {
    pub thread_id: String,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct DeleteAiChatThreadResponse {
    pub success: bool,
}

#[derive(Default)]
pub struct AiChatQuery;

#[Object]
impl AiChatQuery {
    async fn ai_chat_threads(&self, ctx: &Context<'_>) -> Result<Vec<AiChatThread>> {
        let user_id = get_user_id_from_ctx(ctx).await?;
        let turso = ctx.data::<Arc<TursoClient>>()?;
        let conn = turso.get_connection()?;

        Ok(ai_chat_table::list_threads(&conn, &user_id)
            .await?
            .into_iter()
            .map(|thread| AiChatThread {
                id: thread.id,
                title: thread.title,
                created_at: thread.created_at,
                updated_at: thread.updated_at,
            })
            .collect())
    }

    async fn ai_chat_messages(
        &self,
        ctx: &Context<'_>,
        thread_id: String,
    ) -> Result<Vec<AiChatMessage>> {
        let user_id = get_user_id_from_ctx(ctx).await?;
        let turso = ctx.data::<Arc<TursoClient>>()?;
        let conn = turso.get_connection()?;

        Ok(ai_chat_table::list_thread_messages(&conn, &user_id, &thread_id)
            .await?
            .into_iter()
            .map(|message| AiChatMessage {
                id: message.id,
                thread_id: message.thread_id,
                request_id: message.request_id,
                role: message.role,
                content: message.content,
                created_at: message.created_at,
            })
            .collect())
    }
}

#[derive(Default)]
pub struct AiChatMutation;

#[Object]
impl AiChatMutation {
    async fn ai_chat(&self, ctx: &Context<'_>, input: AiChatInput) -> Result<AiChatResponse> {
        let user_id = get_user_id_from_ctx(ctx).await?;
        let agents_client = ctx
            .data::<Option<crate::service::agents::AgentsClient>>()?
            .clone();
        let turso = ctx.data::<Arc<TursoClient>>()?.clone();

        let response = ai_chat::send_chat_message(
            &agents_client,
            &turso,
            &user_id,
            input.thread_id,
            input.message,
        )
        .await?;

        Ok(AiChatResponse {
            request_id: response.request_id,
            thread_id: response.thread_id,
            text: response.text,
            promoted_memory_uris: response.promoted_memory_uris,
        })
    }

    async fn delete_ai_chat_thread(
        &self,
        ctx: &Context<'_>,
        input: DeleteAiChatThreadInput,
    ) -> Result<DeleteAiChatThreadResponse> {
        let user_id = get_user_id_from_ctx(ctx).await?;
        let turso = ctx.data::<Arc<TursoClient>>()?.clone();

        let success = ai_chat::delete_thread(&turso, &user_id, input.thread_id).await?;

        Ok(DeleteAiChatThreadResponse { success })
    }
}
