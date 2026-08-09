use async_graphql::{Context, Object, Result, SimpleObject};
use std::sync::Arc;

use crate::service::db::schema::tables::notebook::sync as notebook_sync;
use crate::service::db::schema::tables::playbook_table::{
    self, CreatePlaybookInput, PlaybookDelta, UpdatePlaybookInput,
};
use crate::service::read_service::playbook as playbook_service;
use crate::service::{ai::jobs as ai_jobs, db::Db};

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PlaybookDeltaGql {
    pub id: String,
    pub name: String,
    pub edge_name: String,
    pub entry_rules: String,
    pub exit_rules: String,
    pub position_sizing_rules: String,
    pub additional_rules: Option<String>,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

impl From<PlaybookDelta> for PlaybookDeltaGql {
    fn from(d: PlaybookDelta) -> Self {
        Self {
            id: d.id,
            name: d.name,
            edge_name: d.edge_name,
            entry_rules: d.entry_rules,
            exit_rules: d.exit_rules,
            position_sizing_rules: d.position_sizing_rules,
            additional_rules: d.additional_rules,
            hlc: d.hlc,
            deleted_at: d.deleted_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PlaybookPullResult {
    pub cookie: String,
    pub last_mutation_id: i64,
    pub playbooks: Vec<PlaybookDeltaGql>,
}

async fn get_user_db(ctx: &Context<'_>) -> Result<crate::service::db::client::UserDb> {
    crate::graphql::auth::user_db(ctx).await
}

#[derive(Default)]
pub struct PlaybookQuery;

#[Object]
impl PlaybookQuery {
    async fn playbooks(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<Vec<playbook_service::PlaybookWithStats>> {
        let user_db = get_user_db(ctx).await?;
        Ok(playbook_service::list_playbooks(&user_db, &workspace_id).await?)
    }

    async fn playbook(
        &self,
        ctx: &Context<'_>,
        id: String,
    ) -> Result<Option<playbook_service::PlaybookWithStats>> {
        let user_db = get_user_db(ctx).await?;
        Ok(playbook_service::get_playbook(&user_db, &id).await?)
    }

    /// Offline-first pull for one workspace, with its own cursor. It is separate
    /// from the notebook pull, while `lastMutationId` remains the shared per-client watermark because
    /// playbook mutations ride the same outbox/mutation log as the notebook.
    async fn pull_playbook(
        &self,
        ctx: &Context<'_>,
        cookie: Option<String>,
        client_id: String,
        workspace_id: String,
    ) -> Result<PlaybookPullResult> {
        let user_db = get_user_db(ctx).await?;
        let pool = user_db.pool();
        let user_id = user_db.user_id();

        let deltas =
            playbook_table::playbooks_since(pool, user_id, &workspace_id, cookie.as_deref())
                .await?;

        let mut next = cookie.unwrap_or_default();
        for d in &deltas {
            if d.updated_at > next {
                next = d.updated_at.clone();
            }
        }

        let last_mutation_id =
            notebook_sync::last_mutation_id_for_client(pool, &client_id, user_id).await?;

        Ok(PlaybookPullResult {
            cookie: next,
            last_mutation_id,
            playbooks: deltas.into_iter().map(Into::into).collect(),
        })
    }
}

#[derive(Default)]
pub struct PlaybookMutation;

#[Object]
impl PlaybookMutation {
    async fn create_playbook(
        &self,
        ctx: &Context<'_>,
        input: CreatePlaybookInput,
    ) -> Result<playbook_service::PlaybookWithStats> {
        let user_db = get_user_db(ctx).await?;
        let playbook = playbook_service::create_playbook(&user_db, input).await?;
        let db = ctx.data::<Arc<Db>>()?;
        ai_jobs::enqueue_source_reindex(
            db.as_ref(),
            user_db.user_id(),
            &playbook.workspace_id,
            "playbook",
            &playbook.id,
        )
        .await?;
        Ok(playbook)
    }

    async fn update_playbook(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdatePlaybookInput,
    ) -> Result<playbook_service::PlaybookWithStats> {
        let user_db = get_user_db(ctx).await?;
        let playbook = playbook_service::update_playbook(&user_db, &id, input).await?;
        let db = ctx.data::<Arc<Db>>()?;
        ai_jobs::enqueue_source_reindex(
            db.as_ref(),
            user_db.user_id(),
            &playbook.workspace_id,
            "playbook",
            &playbook.id,
        )
        .await?;
        Ok(playbook)
    }

    async fn delete_playbook(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        let existing = playbook_service::get_playbook(&user_db, &id).await?;
        let deleted = playbook_service::delete_playbook(&user_db, &id).await?;
        if deleted {
            let db = ctx.data::<Arc<Db>>()?;
            if let Some(playbook) = existing {
                ai_jobs::enqueue_source_reindex(
                    db.as_ref(),
                    user_db.user_id(),
                    &playbook.workspace_id,
                    "playbook",
                    &playbook.id,
                )
                .await?;
            }
        }
        Ok(deleted)
    }
}
