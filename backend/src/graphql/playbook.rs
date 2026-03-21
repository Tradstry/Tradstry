use async_graphql::{Context, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::read_service::playbook as playbook_service;
use crate::service::read_service::users::ensure_user;
use crate::service::{ai::jobs as ai_jobs, turso::TursoClient};
use crate::service::turso::schema::tables::playbook_table::{
    CreatePlaybookInput, UpdatePlaybookInput,
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
pub struct PlaybookQuery;

#[Object]
impl PlaybookQuery {
    async fn playbooks(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<playbook_service::PlaybookWithStats>> {
        let user_db = get_user_db(ctx).await?;
        Ok(playbook_service::list_playbooks(&user_db).await?)
    }

    async fn playbook(
        &self,
        ctx: &Context<'_>,
        id: String,
    ) -> Result<Option<playbook_service::PlaybookWithStats>> {
        let user_db = get_user_db(ctx).await?;
        Ok(playbook_service::get_playbook(&user_db, &id).await?)
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
        let turso = ctx.data::<Arc<TursoClient>>()?;
        ai_jobs::enqueue_all_account_reindex(turso.as_ref(), user_db.user_id()).await?;
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
        let turso = ctx.data::<Arc<TursoClient>>()?;
        ai_jobs::enqueue_all_account_reindex(turso.as_ref(), user_db.user_id()).await?;
        Ok(playbook)
    }

    async fn delete_playbook(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        let deleted = playbook_service::delete_playbook(&user_db, &id).await?;
        if deleted {
            let turso = ctx.data::<Arc<TursoClient>>()?;
            ai_jobs::enqueue_all_account_reindex(turso.as_ref(), user_db.user_id()).await?;
        }
        Ok(deleted)
    }
}
