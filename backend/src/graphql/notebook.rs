use async_graphql::{Context, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::read_service::notebook as notebook_service;
use crate::service::read_service::users::ensure_user;
use crate::service::turso::schema::tables::notebook_table::{
    CreateNotebookNoteInput, NotebookNote, UpdateNotebookNoteInput,
};
use crate::service::{ai::jobs as ai_jobs, turso::TursoClient};

async fn get_user_db(ctx: &Context<'_>) -> Result<crate::service::turso::client::UserDb> {
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

    Ok(turso.get_user_db(&user.id).await?)
}

#[derive(Default)]
pub struct NotebookQuery;

#[Object]
impl NotebookQuery {
    async fn notebook_notes(
        &self,
        ctx: &Context<'_>,
        account_id: Option<String>,
    ) -> Result<Vec<NotebookNote>> {
        let user_db = get_user_db(ctx).await?;
        Ok(notebook_service::list_notebook_notes(&user_db, account_id.as_deref()).await?)
    }

    async fn notebook_note(&self, ctx: &Context<'_>, id: String) -> Result<Option<NotebookNote>> {
        let user_db = get_user_db(ctx).await?;
        Ok(notebook_service::get_notebook_note(&user_db, &id).await?)
    }
}

#[derive(Default)]
pub struct NotebookMutation;

#[Object]
impl NotebookMutation {
    async fn create_notebook_note(
        &self,
        ctx: &Context<'_>,
        input: CreateNotebookNoteInput,
    ) -> Result<NotebookNote> {
        let user_db = get_user_db(ctx).await?;
        let note = notebook_service::create_notebook_note(&user_db, input).await?;
        let turso = ctx.data::<Arc<TursoClient>>()?;
        ai_jobs::enqueue_account_reindex(turso.as_ref(), user_db.user_id(), &note.account_id)
            .await?;
        Ok(note)
    }

    async fn update_notebook_note(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdateNotebookNoteInput,
    ) -> Result<NotebookNote> {
        let user_db = get_user_db(ctx).await?;
        let note = notebook_service::update_notebook_note(&user_db, &id, input).await?;
        let turso = ctx.data::<Arc<TursoClient>>()?;
        ai_jobs::enqueue_account_reindex(turso.as_ref(), user_db.user_id(), &note.account_id)
            .await?;
        Ok(note)
    }

    async fn delete_notebook_note(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        let existing = notebook_service::get_notebook_note(&user_db, &id).await?;
        let deleted = notebook_service::delete_notebook_note(&user_db, &id).await?;
        if deleted && let Some(note) = existing {
            let turso = ctx.data::<Arc<TursoClient>>()?;
            ai_jobs::enqueue_account_reindex(turso.as_ref(), user_db.user_id(), &note.account_id)
                .await?;
        }
        Ok(deleted)
    }
}
