use async_graphql::{ComplexObject, Context, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::graphql::tags::TagGql;
use crate::service::read_service::journal as journal_service;
use crate::service::read_service::tags as tags_service;
use crate::service::read_service::users::ensure_user;
use crate::service::turso::schema::tables::journal_table::{
    self as journal_table, CreateJournalEntryInput, JournalEntry, UpdateJournalEntryInput,
};
use crate::service::{ai::jobs as ai_jobs, turso::TursoClient};

#[ComplexObject]
impl JournalEntry {
    /// Tags attached to this trade (replacement for the legacy freeform
    /// mistakes/entry_tactics/edges_spotted fields, which remain for legacy
    /// display).
    async fn tags(&self, ctx: &Context<'_>) -> Result<Vec<TagGql>> {
        let user_db = get_user_db(ctx).await?;
        Ok(tags_service::tags_for_trade(&user_db, &self.id)
            .await?
            .into_iter()
            .map(TagGql::from)
            .collect())
    }
}

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

    async fn linked_brokerage_transaction_ids(
        &self,
        ctx: &Context<'_>,
        account_id: String,
    ) -> Result<Vec<String>> {
        let user_db = get_user_db(ctx).await?;
        Ok(journal_table::list_linked_brokerage_transaction_ids(
            user_db.conn(),
            user_db.user_id(),
            &account_id,
        )
        .await?)
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
        let tag_ids = input.tag_ids.clone();
        let entry = journal_service::create_journal_entry(&user_db, input).await?;
        tags_service::set_trade_tags(&user_db, &entry.id, &tag_ids).await?;
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
        let tag_ids = input.tag_ids.clone();
        let entry = journal_service::update_journal_entry(&user_db, &id, input).await?;
        if let Some(ids) = tag_ids {
            tags_service::set_trade_tags(&user_db, &entry.id, &ids).await?;
        }
        let turso = ctx.data::<Arc<TursoClient>>()?;
        ai_jobs::enqueue_account_reindex(turso.as_ref(), user_db.user_id(), &entry.account_id)
            .await?;
        Ok(entry)
    }

    async fn delete_journal_entry(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        let existing = journal_service::get_journal_entry(&user_db, &id).await?;
        let deleted = journal_service::delete_journal_entry(&user_db, &id).await?;
        if deleted && let Some(entry) = existing {
            let turso = ctx.data::<Arc<TursoClient>>()?;
            ai_jobs::enqueue_account_reindex(turso.as_ref(), user_db.user_id(), &entry.account_id)
                .await?;
        }
        Ok(deleted)
    }
}
