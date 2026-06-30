use async_graphql::{ComplexObject, Context, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::graphql::tags::TagGql;
use crate::service::db::schema::tables::journal_table::{
    self as journal_table, CreateJournalEntryInput, JournalEntry, UpdateJournalEntryInput,
};
use crate::service::read_service::journal as journal_service;
use crate::service::read_service::tags as tags_service;
use crate::service::read_service::users::ensure_user;
use crate::service::{ai::jobs as ai_jobs, db::Db};

#[ComplexObject]
impl JournalEntry {
    /// Tags attached to this trade (replacement for the legacy freeform
    /// mistakes/entry_tactics/edges_spotted fields, which remain for legacy
    /// display).
    async fn tags(&self, ctx: &Context<'_>) -> Result<Vec<TagGql>> {
        let loader =
            ctx.data::<async_graphql::dataloader::DataLoader<crate::graphql::tags::TagLoader>>()?;
        Ok(loader.load_one(self.id.clone()).await?.unwrap_or_default())
    }
}

async fn get_user_db(ctx: &Context<'_>) -> Result<crate::service::db::client::UserDb> {
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

    Ok(db.get_user_db(&user.id))
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
            user_db.pool(),
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
        let db = ctx.data::<Arc<Db>>()?;
        ai_jobs::enqueue_account_reindex(db.as_ref(), user_db.user_id(), &entry.account_id).await?;
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
        let db = ctx.data::<Arc<Db>>()?;
        ai_jobs::enqueue_account_reindex(db.as_ref(), user_db.user_id(), &entry.account_id).await?;
        Ok(entry)
    }

    async fn delete_journal_entry(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        let existing = journal_service::get_journal_entry(&user_db, &id).await?;
        let deleted = journal_service::delete_journal_entry(&user_db, &id).await?;
        if deleted && let Some(entry) = existing {
            let db = ctx.data::<Arc<Db>>()?;
            ai_jobs::enqueue_account_reindex(db.as_ref(), user_db.user_id(), &entry.account_id)
                .await?;
        }
        Ok(deleted)
    }
}
