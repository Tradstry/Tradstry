use async_graphql::{ComplexObject, Context, Object, Result, SimpleObject};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::graphql::tags::TagGql;
use crate::service::db::schema::tables::journal_table::{
    self as journal_table, CreateJournalEntryInput, JournalDelta, JournalEntry,
    UpdateJournalEntryInput,
};
use crate::service::db::schema::tables::notebook::sync as notebook_sync;
use crate::service::read_service::journal as journal_service;
use crate::service::read_service::principle as principle_service;
use crate::service::read_service::tags as tags_service;
use crate::service::read_service::users::ensure_user;
use crate::service::{ai::jobs as ai_jobs, db::Db};

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct JournalEntryDeltaGql {
    pub id: String,
    pub open_date: String,
    pub close_date: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub position_size: f64,
    pub symbol: String,
    pub symbol_name: String,
    pub status: String,
    pub total_pl: f64,
    pub net_roi: f64,
    pub duration: i64,
    pub stop_loss: Option<f64>,
    pub risk_reward: Option<f64>,
    pub trade_type: String,
    pub mistakes: String,
    pub entry_tactics: String,
    pub edges_spotted: String,
    pub playbook_id: Option<String>,
    pub notes: Option<String>,
    pub broke_30min_rule: Option<bool>,
    pub pre_trade_conviction: Option<i32>,
    pub market_regime: Option<String>,
    pub is_planned_pre_market: Option<bool>,
    pub revenge_trade: Option<bool>,
    pub rule_adherence_score: Option<i32>,
    pub tag_ids: Vec<String>,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

impl From<JournalDelta> for JournalEntryDeltaGql {
    fn from(d: JournalDelta) -> Self {
        Self {
            id: d.id,
            open_date: d.open_date,
            close_date: d.close_date,
            entry_price: d.entry_price,
            exit_price: d.exit_price,
            position_size: d.position_size,
            symbol: d.symbol,
            symbol_name: d.symbol_name,
            status: d.status,
            total_pl: d.total_pl,
            net_roi: d.net_roi,
            duration: d.duration,
            stop_loss: d.stop_loss,
            risk_reward: d.risk_reward,
            trade_type: d.trade_type,
            mistakes: d.mistakes,
            entry_tactics: d.entry_tactics,
            edges_spotted: d.edges_spotted,
            playbook_id: d.playbook_id,
            notes: d.notes,
            broke_30min_rule: d.broke_30min_rule,
            pre_trade_conviction: d.pre_trade_conviction,
            market_regime: d.market_regime,
            is_planned_pre_market: d.is_planned_pre_market,
            revenge_trade: d.revenge_trade,
            rule_adherence_score: d.rule_adherence_score,
            tag_ids: d.tag_ids,
            hlc: d.hlc,
            deleted_at: d.deleted_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct JournalPullResult {
    pub cookie: String,
    pub last_mutation_id: i64,
    pub entries: Vec<JournalEntryDeltaGql>,
}

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

    async fn trade_violated_principle_ids(
        &self,
        ctx: &Context<'_>,
        journal_entry_id: String,
    ) -> Result<Vec<String>> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::principles_for_trade(&user_db, &journal_entry_id).await?)
    }

    /// Offline-first pull for the desktop. Account-scoped (journal entries
    /// belong to one account, unlike playbooks), with its own cursor.
    /// `lastMutationId` is the shared per-client watermark because journal
    /// mutations ride the same outbox/mutation log as the notebook.
    async fn pull_journal(
        &self,
        ctx: &Context<'_>,
        cookie: Option<String>,
        account_id: String,
        client_id: String,
    ) -> Result<JournalPullResult> {
        let user_db = get_user_db(ctx).await?;
        let pool = user_db.pool();
        let user_id = user_db.user_id();

        let deltas =
            journal_table::journal_entries_since(pool, user_id, &account_id, cookie.as_deref())
                .await?;

        let mut next = cookie.unwrap_or_default();
        for d in &deltas {
            if d.updated_at > next {
                next = d.updated_at.clone();
            }
        }

        let last_mutation_id =
            notebook_sync::last_mutation_id_for_client(pool, &client_id, user_id).await?;

        Ok(JournalPullResult {
            cookie: next,
            last_mutation_id,
            entries: deltas.into_iter().map(Into::into).collect(),
        })
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
        let violated_principle_ids = input.violated_principle_ids.clone();
        let entry = journal_service::create_journal_entry(&user_db, input).await?;
        tags_service::set_trade_tags(&user_db, &entry.id, &tag_ids).await?;
        principle_service::set_trade_principle_violations(
            &user_db,
            &entry.id,
            &violated_principle_ids,
        )
        .await?;
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
        let violated_principle_ids = input.violated_principle_ids.clone();
        let entry = journal_service::update_journal_entry(&user_db, &id, input).await?;
        if let Some(ids) = tag_ids {
            tags_service::set_trade_tags(&user_db, &entry.id, &ids).await?;
        }
        if let Some(ids) = violated_principle_ids {
            principle_service::set_trade_principle_violations(&user_db, &entry.id, &ids).await?;
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
