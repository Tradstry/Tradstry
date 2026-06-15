use anyhow::{Context, Result, anyhow};
use libsql::params;
use serde_json::Value;
use uuid::Uuid;

use crate::service::turso::client::TursoClient;

use super::types::{AiArtifactEnvelope, AiJobHandle, AiJobRecord, AiSourceDocument, AiTimeFilter};

/// Max times a job is leased before it's left as a permanent dead-letter. Caps the
/// damage from a "poison" job (bad data, or a corrupt local replica that fails every
/// attempt) so it can't be re-leased forever and spin the worker.
const MAX_JOB_ATTEMPTS: i64 = 5;

fn now_sql() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn value_to_string(row: &libsql::Row, index: i32) -> Option<String> {
    row.get::<libsql::Value>(index)
        .ok()
        .and_then(|value| match value {
            libsql::Value::Text(value) => Some(value),
            _ => None,
        })
}

#[allow(clippy::too_many_arguments)]
pub async fn enqueue_job(
    turso: &TursoClient,
    user_id: &str,
    account_id: &str,
    job_type: &str,
    artifact_type: Option<&str>,
    time_filter: &AiTimeFilter,
    payload: &Value,
    dedupe_key: Option<&str>,
) -> Result<AiJobHandle> {
    let conn = turso.get_connection()?;
    let time_filter_json =
        serde_json::to_string(time_filter).context("failed to serialize time filter")?;

    if let Some(dedupe_key) = dedupe_key {
        let mut rows = conn
            .query(
                "SELECT id, status FROM ai_jobs WHERE dedupe_key = ?1 AND status IN ('queued', 'running') ORDER BY created_at DESC LIMIT 1",
                params![dedupe_key],
            )
            .await
            .context("failed to query dedupe ai job")?;

        if let Some(row) = rows.next().await? {
            return Ok(AiJobHandle {
                job_id: row.get::<String>(0)?,
                status: row.get::<String>(1)?,
            });
        }
    }

    let id = new_id();
    conn.execute(
        "INSERT INTO ai_jobs (
            id, user_id, account_id, job_type, artifact_type, time_filter_json, payload_json, dedupe_key, status, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued', ?9, ?9)",
        params![
            id.as_str(),
            user_id,
            account_id,
            job_type,
            artifact_type,
            time_filter_json.as_str(),
            payload.to_string(),
            dedupe_key,
            now_sql().as_str(),
        ],
    )
    .await
    .context("failed to insert ai job")?;

    Ok(AiJobHandle {
        job_id: id,
        status: "queued".to_string(),
    })
}

pub async fn lease_due_job(
    turso: &TursoClient,
    lease_owner: &str,
    lease_seconds: i64,
) -> Result<Option<AiJobRecord>> {
    let conn = turso.get_connection()?;
    let now = chrono::Utc::now();
    let stale_before = (now - chrono::Duration::seconds(lease_seconds)).to_rfc3339();
    let now_rfc3339 = now.to_rfc3339();

    let mut rows = conn
        .query(
            "SELECT id, user_id, account_id, job_type, artifact_type, time_filter_json, payload_json, status, error_message, created_at, completed_at
             FROM ai_jobs
             WHERE attempt_count < ?2
               AND (status = 'queued' OR (status = 'running' AND leased_at < ?1))
             ORDER BY created_at ASC
             LIMIT 1",
            params![stale_before.as_str(), MAX_JOB_ATTEMPTS],
        )
        .await
        .context("failed to lease ai job")?;

    let Some(row) = rows.next().await? else {
        return Ok(None);
    };

    let id = row.get::<String>(0)?;
    conn.execute(
        "UPDATE ai_jobs
         SET status = 'running', lease_owner = ?1, leased_at = ?2, attempt_count = attempt_count + 1, updated_at = ?2
         WHERE id = ?3",
        params![lease_owner, now_rfc3339.as_str(), id.as_str()],
    )
    .await
    .context("failed to update leased ai job")?;

    Ok(Some(AiJobRecord {
        id,
        user_id: row.get::<String>(1)?,
        account_id: row.get::<String>(2)?,
        job_type: row.get::<String>(3)?,
        artifact_type: value_to_string(&row, 4),
        time_filter_json: row.get::<String>(5)?,
        payload_json: row.get::<String>(6)?,
        status: "running".to_string(),
        error_message: value_to_string(&row, 8),
        created_at: row.get::<String>(9)?,
        completed_at: value_to_string(&row, 10),
    }))
}

pub async fn complete_job(turso: &TursoClient, job_id: &str) -> Result<()> {
    let conn = turso.get_connection()?;
    let now = now_sql();
    conn.execute(
        "UPDATE ai_jobs SET status = 'completed', completed_at = ?1, updated_at = ?1 WHERE id = ?2",
        params![now.as_str(), job_id],
    )
    .await
    .context("failed to complete ai job")?;
    Ok(())
}

pub async fn fail_job(turso: &TursoClient, job_id: &str, error_message: &str) -> Result<()> {
    let conn = turso.get_connection()?;
    let now = now_sql();
    conn.execute(
        "UPDATE ai_jobs SET status = 'failed', error_message = ?1, completed_at = ?2, updated_at = ?2 WHERE id = ?3",
        params![error_message, now.as_str(), job_id],
    )
    .await
    .context("failed to fail ai job")?;
    Ok(())
}

pub async fn get_job_for_user(
    turso: &TursoClient,
    user_id: &str,
    job_id: &str,
) -> Result<Option<AiJobRecord>> {
    let conn = turso.get_connection()?;
    let mut rows = conn
        .query(
            "SELECT id, user_id, account_id, job_type, artifact_type, time_filter_json, payload_json, status, error_message, created_at, completed_at
             FROM ai_jobs WHERE id = ?1 AND user_id = ?2 LIMIT 1",
            params![job_id, user_id],
        )
        .await
        .context("failed to fetch ai job")?;

    if let Some(row) = rows.next().await? {
        Ok(Some(AiJobRecord {
            id: row.get::<String>(0)?,
            user_id: row.get::<String>(1)?,
            account_id: row.get::<String>(2)?,
            job_type: row.get::<String>(3)?,
            artifact_type: value_to_string(&row, 4),
            time_filter_json: row.get::<String>(5)?,
            payload_json: row.get::<String>(6)?,
            status: row.get::<String>(7)?,
            error_message: value_to_string(&row, 8),
            created_at: row.get::<String>(9)?,
            completed_at: value_to_string(&row, 10),
        }))
    } else {
        Ok(None)
    }
}

pub async fn replace_source_documents_for_account(
    turso: &TursoClient,
    user_id: &str,
    account_id: &str,
    docs: &[AiSourceDocument],
) -> Result<()> {
    let conn = turso.get_connection()?;
    conn.execute(
        "DELETE FROM ai_source_documents WHERE user_id = ?1 AND account_id = ?2",
        params![user_id, account_id],
    )
    .await
    .context("failed to delete ai source documents")?;

    for doc in docs {
        conn.execute(
            "INSERT INTO ai_source_documents (
                id, user_id, account_id, source_type, source_id, title, body_text, metadata_json, content_hash, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                doc.id.as_str(),
                doc.user_id.as_str(),
                doc.account_id.as_str(),
                doc.source_type.as_str(),
                doc.source_id.as_str(),
                doc.title.as_str(),
                doc.body_text.as_str(),
                doc.metadata_json.as_str(),
                doc.content_hash.as_str(),
                now_sql().as_str(),
            ],
        )
        .await
        .with_context(|| format!("failed to insert ai source document {}", doc.id))?;
    }

    Ok(())
}

pub async fn list_source_documents_for_account(
    turso: &TursoClient,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<AiSourceDocument>> {
    let conn = turso.get_connection()?;
    let mut rows = conn
        .query(
            "SELECT id, user_id, account_id, source_type, source_id, title, body_text, metadata_json, content_hash
             FROM ai_source_documents
             WHERE user_id = ?1 AND account_id = ?2
             ORDER BY updated_at DESC",
            params![user_id, account_id],
        )
        .await
        .context("failed to list ai source documents")?;

    let mut docs = Vec::new();
    while let Some(row) = rows.next().await? {
        docs.push(AiSourceDocument {
            id: row.get::<String>(0)?,
            user_id: row.get::<String>(1)?,
            account_id: row.get::<String>(2)?,
            source_type: row.get::<String>(3)?,
            source_id: row.get::<String>(4)?,
            title: row.get::<String>(5)?,
            body_text: row.get::<String>(6)?,
            metadata_json: row.get::<String>(7)?,
            content_hash: row.get::<String>(8)?,
        });
    }

    Ok(docs)
}

#[allow(clippy::too_many_arguments)]
pub async fn save_artifact(
    turso: &TursoClient,
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
    let conn = turso.get_connection()?;
    let time_filter_json =
        serde_json::to_string(time_filter).context("failed to serialize ai time filter")?;
    let payload_json =
        serde_json::to_string(artifact).context("failed to serialize ai artifact payload")?;
    let artifact_id = new_id();
    let now = now_sql();
    let range_bounds = resolve_range_bounds(time_filter);

    conn.execute(
        "DELETE FROM ai_artifacts WHERE user_id = ?1 AND account_id = ?2 AND artifact_type = ?3 AND time_filter_json = ?4",
        params![user_id, account_id, artifact_type, time_filter_json.as_str()],
    )
    .await
    .context("failed to replace existing ai artifact")?;

    conn.execute(
        "INSERT INTO ai_artifacts (
            id, user_id, account_id, artifact_type, time_filter_json, range_start, range_end, status,
            model, prompt_version, payload_json, generated_at, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, ?12)",
        params![
            artifact_id.as_str(),
            user_id,
            account_id,
            artifact_type,
            time_filter_json.as_str(),
            range_bounds.0.as_deref(),
            range_bounds.1.as_deref(),
            status,
            model,
            prompt_version,
            payload_json.as_str(),
            now.as_str(),
        ],
    )
    .await
    .context("failed to insert ai artifact")?;

    for doc in citations {
        let excerpt = doc.body_text.chars().take(240).collect::<String>();
        conn.execute(
            "INSERT INTO ai_artifact_sources (
                id, artifact_id, source_document_id, source_type, source_id, title, excerpt, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                new_id(),
                artifact_id.as_str(),
                doc.id.as_str(),
                doc.source_type.as_str(),
                doc.source_id.as_str(),
                doc.title.as_str(),
                excerpt.as_str(),
                now.as_str(),
            ],
        )
        .await
        .context("failed to insert ai artifact source")?;
    }

    Ok(())
}

pub async fn get_latest_artifact(
    turso: &TursoClient,
    user_id: &str,
    account_id: &str,
    artifact_type: &str,
    time_filter: &AiTimeFilter,
) -> Result<Option<AiArtifactEnvelope>> {
    let conn = turso.get_connection()?;
    let time_filter_json =
        serde_json::to_string(time_filter).context("failed to serialize ai time filter")?;
    let mut rows = conn
        .query(
            "SELECT payload_json FROM ai_artifacts
             WHERE user_id = ?1 AND account_id = ?2 AND artifact_type = ?3 AND time_filter_json = ?4
             ORDER BY generated_at DESC
             LIMIT 1",
            params![
                user_id,
                account_id,
                artifact_type,
                time_filter_json.as_str()
            ],
        )
        .await
        .context("failed to query latest ai artifact")?;

    let Some(row) = rows.next().await? else {
        return Ok(None);
    };

    let payload_json = row.get::<String>(0)?;
    let artifact =
        serde_json::from_str::<AiArtifactEnvelope>(&payload_json).with_context(|| {
            format!("failed to deserialize ai artifact payload for {artifact_type}")
        })?;

    Ok(Some(artifact))
}

pub async fn ensure_account_exists_for_user(
    turso: &TursoClient,
    user_id: &str,
    account_id: &str,
) -> Result<()> {
    let conn = turso.get_connection()?;
    let mut rows = conn
        .query(
            "SELECT id FROM accounts WHERE id = ?1 AND user_id = ?2 LIMIT 1",
            params![account_id, user_id],
        )
        .await
        .context("failed to validate account")?;

    if rows.next().await?.is_some() {
        Ok(())
    } else {
        Err(anyhow!("account not found"))
    }
}

pub async fn list_user_account_ids(turso: &TursoClient, user_id: &str) -> Result<Vec<String>> {
    let conn = turso.get_connection()?;
    let mut rows = conn
        .query(
            "SELECT id FROM accounts WHERE user_id = ?1 ORDER BY created_at ASC",
            params![user_id],
        )
        .await
        .context("failed to list user accounts")?;

    let mut account_ids = Vec::new();
    while let Some(row) = rows.next().await? {
        account_ids.push(row.get::<String>(0)?);
    }
    Ok(account_ids)
}

fn resolve_range_bounds(time_filter: &AiTimeFilter) -> (Option<String>, Option<String>) {
    match time_filter.range {
        super::types::AiRange::Custom => {
            (time_filter.start_date.clone(), time_filter.end_date.clone())
        }
        _ => (None, None),
    }
}
