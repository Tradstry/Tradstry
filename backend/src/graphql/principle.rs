use async_graphql::{Context, Object, Result, SimpleObject};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::db::Db;
use crate::service::db::schema::tables::notebook::sync as notebook_sync;
use crate::service::db::schema::tables::trading_principle_table::{
    self, CreatePrincipleInput, PrincipleDelta, UpdatePrincipleInput,
};
use crate::service::read_service::principle as principle_service;
use crate::service::read_service::users::ensure_user;

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PrincipleDeltaGql {
    pub id: String,
    pub account_id: String,
    pub playbook_id: Option<String>,
    pub evidence_note_id: Option<String>,
    pub title: String,
    pub the_rule: String,
    pub why: String,
    pub intervention: Option<String>,
    pub priority: i64,
    pub is_active: bool,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

impl From<PrincipleDelta> for PrincipleDeltaGql {
    fn from(d: PrincipleDelta) -> Self {
        Self {
            id: d.id,
            account_id: d.account_id,
            playbook_id: d.playbook_id,
            evidence_note_id: d.evidence_note_id,
            title: d.title,
            the_rule: d.the_rule,
            why: d.why,
            intervention: d.intervention,
            priority: d.priority,
            is_active: d.is_active,
            hlc: d.hlc,
            deleted_at: d.deleted_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PrinciplePullResult {
    pub cookie: String,
    pub last_mutation_id: i64,
    pub principles: Vec<PrincipleDeltaGql>,
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
pub struct PrincipleQuery;

#[Object]
impl PrincipleQuery {
    async fn principles(
        &self,
        ctx: &Context<'_>,
        account_id: String,
    ) -> Result<Vec<principle_service::PrincipleWithStats>> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::list_principles(&user_db, &account_id).await?)
    }

    async fn principle(
        &self,
        ctx: &Context<'_>,
        id: String,
    ) -> Result<Option<principle_service::PrincipleWithStats>> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::get_principle(&user_db, &id).await?)
    }

    /// Offline-first pull for the desktop. Account-scoped (principles belong
    /// to one account), with its own cursor. Mirrors `journal::pull_journal`;
    /// `lastMutationId` is the shared per-client watermark because principle
    /// mutations ride the same outbox/mutation log as the notebook.
    async fn pull_principle(
        &self,
        ctx: &Context<'_>,
        cookie: Option<String>,
        account_id: String,
        client_id: String,
    ) -> Result<PrinciplePullResult> {
        let user_db = get_user_db(ctx).await?;
        let pool = user_db.pool();
        let user_id = user_db.user_id();

        let deltas = trading_principle_table::principles_since(
            pool,
            user_id,
            &account_id,
            cookie.as_deref(),
        )
        .await?;

        let mut next = cookie.unwrap_or_default();
        for d in &deltas {
            if d.updated_at > next {
                next = d.updated_at.clone();
            }
        }

        let last_mutation_id =
            notebook_sync::last_mutation_id_for_client(pool, &client_id, user_id).await?;

        Ok(PrinciplePullResult {
            cookie: next,
            last_mutation_id,
            principles: deltas.into_iter().map(Into::into).collect(),
        })
    }
}

#[derive(Default)]
pub struct PrincipleMutation;

// No `ai_jobs::enqueue_all_account_reindex` here: principles are not indexed
// into the vector store, so a reindex would be pure cost.
#[Object]
impl PrincipleMutation {
    async fn create_principle(
        &self,
        ctx: &Context<'_>,
        input: CreatePrincipleInput,
    ) -> Result<principle_service::PrincipleWithStats> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::create_principle(&user_db, input).await?)
    }

    async fn update_principle(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdatePrincipleInput,
    ) -> Result<principle_service::PrincipleWithStats> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::update_principle(&user_db, &id, input).await?)
    }

    async fn delete_principle(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::delete_principle(&user_db, &id).await?)
    }

    async fn reorder_principles(
        &self,
        ctx: &Context<'_>,
        ordered_ids: Vec<String>,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(principle_service::reorder_principles(&user_db, &ordered_ids).await?)
    }
}
