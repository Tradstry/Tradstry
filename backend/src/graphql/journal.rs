use async_graphql::{Context, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::read_service::journal as journal_service;
use crate::service::read_service::users::ensure_user;
use crate::service::{ai::jobs as ai_jobs, turso::TursoClient};
use crate::service::turso::schema::tables::journal_table::{
    CreateJournalEntryInput, JournalEntry, UpdateJournalEntryInput,
};

async fn get_user_db(ctx: &Context<'_>) -> Result<crate::service::turso::client::UserDb> {
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

    Ok(turso.get_user_db(&user.id).await?)
}

#[derive(Default)]
pub struct JournalQuery;

#[Object]
impl JournalQuery {
    async fn journal_entries(&self, ctx: &Context<'_>) -> Result<Vec<JournalEntry>> {
        let user_db = get_user_db(ctx).await?;
        Ok(journal_service::list_journal_entries(&user_db).await?)
    }

    async fn journal_entry(&self, ctx: &Context<'_>, id: String) -> Result<Option<JournalEntry>> {
        let user_db = get_user_db(ctx).await?;
        Ok(journal_service::get_journal_entry(&user_db, &id).await?)
    }
}

#[derive(Default)]
pub struct JournalMutation;

#[Object]
impl JournalMutation {
    async fn create_journal_entry(
        &self,
        ctx: &Context<'_>,
        input: CreateJournalEntryInput,
    ) -> Result<JournalEntry> {
        let user_db = get_user_db(ctx).await?;
        let entry = journal_service::create_journal_entry(&user_db, input).await?;
        let turso = ctx.data::<Arc<TursoClient>>()?;
        ai_jobs::enqueue_account_reindex(turso.as_ref(), user_db.user_id(), &entry.account_id)
            .await?;
        Ok(entry)
    }

    async fn update_journal_entry(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdateJournalEntryInput,
    ) -> Result<JournalEntry> {
        let user_db = get_user_db(ctx).await?;
        let entry = journal_service::update_journal_entry(&user_db, &id, input).await?;
        let turso = ctx.data::<Arc<TursoClient>>()?;
        ai_jobs::enqueue_account_reindex(turso.as_ref(), user_db.user_id(), &entry.account_id)
            .await?;
        Ok(entry)
    }

    async fn delete_journal_entry(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        let existing = journal_service::get_journal_entry(&user_db, &id).await?;
        let deleted = journal_service::delete_journal_entry(&user_db, &id).await?;
        if deleted {
            if let Some(entry) = existing {
                let turso = ctx.data::<Arc<TursoClient>>()?;
                ai_jobs::enqueue_account_reindex(
                    turso.as_ref(),
                    user_db.user_id(),
                    &entry.account_id,
                )
                .await?;
            }
        }
        Ok(deleted)
    }
}
