use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::service::db::client::Db;
use crate::service::db::util::parse_flexible_datetime;

use super::types::{AiArtifactEnvelope, AiJobHandle, AiJobRecord, AiSourceDocument, AiTimeFilter};

/// Max times a job is leased before it's left as a permanent dead-letter. Caps the
/// damage from a "poison" job (bad data, or a corrupt local replica that fails every
/// attempt) so it can't be re-leased forever and spin the worker.
const MAX_JOB_ATTEMPTS: i64 = 5;

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// `to_char` expressions that render TIMESTAMPTZ columns as RFC3339 UTC strings,
/// matching the `String`/`Option<String>` timestamp fields on `AiJobRecord`.
const JOB_SELECT_COLUMNS: &str = "id, user_id, account_id, job_type, artifact_type, time_filter_json, payload_json, status, error_message, \
     to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
     to_char(completed_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS completed_at";

#[allow(clippy::too_many_arguments)]
pub async fn enqueue_job(
    db: &Db,
    user_id: &str,
    account_id: &str,
    job_type: &str,
    artifact_type: Option<&str>,
    time_filter: &AiTimeFilter,
    payload: &Value,
    dedupe_key: Option<&str>,
) -> Result<AiJobHandle> {
    let pool = db.pool();
    let time_filter_json =
        serde_json::to_string(time_filter).context("failed to serialize time filter")?;

    if let Some(dedupe_key) = dedupe_key {
        let row = sqlx::query(
                "SELECT id, status FROM ai_jobs WHERE dedupe_key = $1 AND status IN ('queued', 'running') ORDER BY created_at DESC LIMIT 1",
            )
            .bind(dedupe_key)
            .fetch_optional(pool)
            .await
            .context("failed to query dedupe ai job")?;

        if let Some(row) = row {
            return Ok(AiJobHandle {
                job_id: row.try_get::<String, _>(0)?,
                status: row.try_get::<String, _>(1)?,
            });
        }
    }

    let id = new_id();
    sqlx::query(
        "INSERT INTO ai_jobs (
            id, user_id, account_id, job_type, artifact_type, time_filter_json, payload_json, dedupe_key, status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'queued')",
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(account_id)
    .bind(job_type)
    .bind(artifact_type)
    .bind(time_filter_json.as_str())
    .bind(payload.to_string())
    .bind(dedupe_key)
    .execute(pool)
    .await
    .context("failed to insert ai job")?;

    Ok(AiJobHandle {
        job_id: id,
        status: "queued".to_string(),
    })
}

pub async fn lease_due_job(
    db: &Db,
    lease_owner: &str,
    lease_seconds: i64,
) -> Result<Option<AiJobRecord>> {
    let pool = db.pool();
    let now = Utc::now();
    let stale_before = now - chrono::Duration::seconds(lease_seconds);

    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {JOB_SELECT_COLUMNS}
             FROM ai_jobs
             WHERE attempt_count < $2
               AND (status = 'queued' OR (status = 'running' AND leased_at < $1))
             ORDER BY created_at ASC
             LIMIT 1"
    )))
    .bind(stale_before)
    .bind(MAX_JOB_ATTEMPTS)
    .fetch_optional(pool)
    .await
    .context("failed to lease ai job")?;

    let Some(row) = row else {
        return Ok(None);
    };

    let id = row.try_get::<String, _>(0)?;
    sqlx::query(
        "UPDATE ai_jobs
         SET status = 'running', lease_owner = $1, leased_at = $2, attempt_count = attempt_count + 1, updated_at = $2
         WHERE id = $3",
    )
    .bind(lease_owner)
    .bind(now)
    .bind(id.as_str())
    .execute(pool)
    .await
    .context("failed to update leased ai job")?;

    Ok(Some(AiJobRecord {
        id,
        user_id: row.try_get::<String, _>(1)?,
        account_id: row.try_get::<String, _>(2)?,
        job_type: row.try_get::<String, _>(3)?,
        artifact_type: row.try_get::<Option<String>, _>(4)?,
        time_filter_json: row.try_get::<String, _>(5)?,
        payload_json: row.try_get::<String, _>(6)?,
        status: "running".to_string(),
        error_message: row.try_get::<Option<String>, _>(8)?,
        created_at: row.try_get::<String, _>(9)?,
        completed_at: row.try_get::<Option<String>, _>(10)?,
    }))
}

pub async fn complete_job(db: &Db, job_id: &str) -> Result<()> {
    let pool = db.pool();
    let now = Utc::now();
    sqlx::query(
        "UPDATE ai_jobs SET status = 'completed', completed_at = $1, updated_at = $1 WHERE id = $2",
    )
    .bind(now)
    .bind(job_id)
    .execute(pool)
    .await
    .context("failed to complete ai job")?;
    Ok(())
}

pub async fn fail_job(db: &Db, job_id: &str, error_message: &str) -> Result<()> {
    let pool = db.pool();
    let now = Utc::now();
    sqlx::query(
        "UPDATE ai_jobs SET status = 'failed', error_message = $1, completed_at = $2, updated_at = $2 WHERE id = $3",
    )
    .bind(error_message)
    .bind(now)
    .bind(job_id)
    .execute(pool)
    .await
    .context("failed to fail ai job")?;
    Ok(())
}

pub async fn get_job_for_user(db: &Db, user_id: &str, job_id: &str) -> Result<Option<AiJobRecord>> {
    let pool = db.pool();
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {JOB_SELECT_COLUMNS}
             FROM ai_jobs WHERE id = $1 AND user_id = $2 LIMIT 1"
    )))
    .bind(job_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("failed to fetch ai job")?;

    if let Some(row) = row {
        Ok(Some(AiJobRecord {
            id: row.try_get::<String, _>(0)?,
            user_id: row.try_get::<String, _>(1)?,
            account_id: row.try_get::<String, _>(2)?,
            job_type: row.try_get::<String, _>(3)?,
            artifact_type: row.try_get::<Option<String>, _>(4)?,
            time_filter_json: row.try_get::<String, _>(5)?,
            payload_json: row.try_get::<String, _>(6)?,
            status: row.try_get::<String, _>(7)?,
            error_message: row.try_get::<Option<String>, _>(8)?,
            created_at: row.try_get::<String, _>(9)?,
            completed_at: row.try_get::<Option<String>, _>(10)?,
        }))
    } else {
        Ok(None)
    }
}

pub async fn replace_source_documents_for_account(
    db: &Db,
    user_id: &str,
    account_id: &str,
    docs: &[AiSourceDocument],
) -> Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM ai_source_documents WHERE user_id = $1 AND account_id = $2")
        .bind(user_id)
        .bind(account_id)
        .execute(&mut *tx)
        .await
        .context("failed to delete ai source documents")?;

    for doc in docs {
        sqlx::query(
            "INSERT INTO ai_source_documents (
                id, user_id, account_id, source_type, source_id, title, body_text, metadata_json, content_hash
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(doc.id.as_str())
        .bind(doc.user_id.as_str())
        .bind(doc.account_id.as_str())
        .bind(doc.source_type.as_str())
        .bind(doc.source_id.as_str())
        .bind(doc.title.as_str())
        .bind(doc.body_text.as_str())
        .bind(doc.metadata_json.as_str())
        .bind(doc.content_hash.as_str())
        .execute(&mut *tx)
        .await
        .with_context(|| format!("failed to insert ai source document {}", doc.id))?;
    }

    tx.commit()
        .await
        .context("failed to commit ai source documents")?;
    Ok(())
}

pub async fn list_source_documents_for_account(
    db: &Db,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<AiSourceDocument>> {
    let pool = db.pool();
    let rows = sqlx::query(
            "SELECT id, user_id, account_id, source_type, source_id, title, body_text, metadata_json, content_hash
             FROM ai_source_documents
             WHERE user_id = $1 AND account_id = $2
             ORDER BY updated_at DESC",
        )
        .bind(user_id)
        .bind(account_id)
        .fetch_all(pool)
        .await
        .context("failed to list ai source documents")?;

    let mut docs = Vec::new();
    for row in &rows {
        docs.push(AiSourceDocument {
            id: row.try_get::<String, _>(0)?,
            user_id: row.try_get::<String, _>(1)?,
            account_id: row.try_get::<String, _>(2)?,
            source_type: row.try_get::<String, _>(3)?,
            source_id: row.try_get::<String, _>(4)?,
            title: row.try_get::<String, _>(5)?,
            body_text: row.try_get::<String, _>(6)?,
            metadata_json: row.try_get::<String, _>(7)?,
            content_hash: row.try_get::<String, _>(8)?,
        });
    }

    Ok(docs)
}

#[allow(clippy::too_many_arguments)]
pub async fn save_artifact(
    db: &Db,
    user_id: &str,
    account_id: &str,
    artifact_type: &str,
    time_filter: &AiTimeFilter,
    status: &str,
    model: &str,
    prompt_version: &str,
    artifact: &AiArtifactEnvelope,
    citations: &[AiSourceDocument],
) -> Result<()> {
    let time_filter_json =
        serde_json::to_string(time_filter).context("failed to serialize ai time filter")?;
    let payload_json =
        serde_json::to_string(artifact).context("failed to serialize ai artifact payload")?;
    let artifact_id = new_id();
    let now = Utc::now();
    let (range_start, range_end) = resolve_range_bounds(time_filter)?;

    let mut tx = db.begin().await?;

    sqlx::query(
        "DELETE FROM ai_artifacts WHERE user_id = $1 AND account_id = $2 AND artifact_type = $3 AND time_filter_json = $4",
    )
    .bind(user_id)
    .bind(account_id)
    .bind(artifact_type)
    .bind(time_filter_json.as_str())
    .execute(&mut *tx)
    .await
    .context("failed to replace existing ai artifact")?;

    sqlx::query(
        "INSERT INTO ai_artifacts (
            id, user_id, account_id, artifact_type, time_filter_json, range_start, range_end, status,
            model, prompt_version, payload_json, generated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
    )
    .bind(artifact_id.as_str())
    .bind(user_id)
    .bind(account_id)
    .bind(artifact_type)
    .bind(time_filter_json.as_str())
    .bind(range_start)
    .bind(range_end)
    .bind(status)
    .bind(model)
    .bind(prompt_version)
    .bind(payload_json.as_str())
    .bind(now)
    .execute(&mut *tx)
    .await
    .context("failed to insert ai artifact")?;

    for doc in citations {
        let excerpt = doc.body_text.chars().take(240).collect::<String>();
        sqlx::query(
            "INSERT INTO ai_artifact_sources (
                id, artifact_id, source_document_id, source_type, source_id, title, excerpt
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(new_id())
        .bind(artifact_id.as_str())
        .bind(doc.id.as_str())
        .bind(doc.source_type.as_str())
        .bind(doc.source_id.as_str())
        .bind(doc.title.as_str())
        .bind(excerpt.as_str())
        .execute(&mut *tx)
        .await
        .context("failed to insert ai artifact source")?;
    }

    tx.commit().await.context("failed to commit ai artifact")?;
    Ok(())
}

pub async fn get_latest_artifact(
    db: &Db,
    user_id: &str,
    account_id: &str,
    artifact_type: &str,
    time_filter: &AiTimeFilter,
) -> Result<Option<AiArtifactEnvelope>> {
    let pool = db.pool();
    let time_filter_json =
        serde_json::to_string(time_filter).context("failed to serialize ai time filter")?;
    let row = sqlx::query(
        "SELECT payload_json FROM ai_artifacts
             WHERE user_id = $1 AND account_id = $2 AND artifact_type = $3 AND time_filter_json = $4
             ORDER BY generated_at DESC
             LIMIT 1",
    )
    .bind(user_id)
    .bind(account_id)
    .bind(artifact_type)
    .bind(time_filter_json.as_str())
    .fetch_optional(pool)
    .await
    .context("failed to query latest ai artifact")?;

    let Some(row) = row else {
        return Ok(None);
    };

    let payload_json = row.try_get::<String, _>(0)?;
    let artifact =
        serde_json::from_str::<AiArtifactEnvelope>(&payload_json).with_context(|| {
            format!("failed to deserialize ai artifact payload for {artifact_type}")
        })?;

    Ok(Some(artifact))
}

pub async fn ensure_account_exists_for_user(
    db: &Db,
    user_id: &str,
    account_id: &str,
) -> Result<()> {
    let pool = db.pool();
    let row = sqlx::query("SELECT id FROM accounts WHERE id = $1 AND user_id = $2 LIMIT 1")
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("failed to validate account")?;

    if row.is_some() {
        Ok(())
    } else {
        Err(anyhow!("account not found"))
    }
}

pub async fn list_user_account_ids(db: &Db, user_id: &str) -> Result<Vec<String>> {
    let pool = db.pool();
    let rows = sqlx::query("SELECT id FROM accounts WHERE user_id = $1 ORDER BY created_at ASC")
        .bind(user_id)
        .fetch_all(pool)
        .await
        .context("failed to list user accounts")?;

    let mut account_ids = Vec::new();
    for row in &rows {
        account_ids.push(row.try_get::<String, _>(0)?);
    }
    Ok(account_ids)
}

/// Inclusive `[start, end]` window bounds, each optional (open-ended).
type RangeBounds = (Option<DateTime<Utc>>, Option<DateTime<Utc>>);

fn resolve_range_bounds(time_filter: &AiTimeFilter) -> Result<RangeBounds> {
    match time_filter.range {
        super::types::AiRange::Custom => {
            let start = time_filter
                .start_date
                .as_deref()
                .map(parse_flexible_datetime)
                .transpose()
                .context("failed to parse ai time filter start_date")?;
            let end = time_filter
                .end_date
                .as_deref()
                .map(parse_flexible_datetime)
                .transpose()
                .context("failed to parse ai time filter end_date")?;
            Ok((start, end))
        }
        _ => Ok((None, None)),
    }
}
