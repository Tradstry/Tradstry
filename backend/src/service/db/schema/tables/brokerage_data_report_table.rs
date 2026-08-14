use anyhow::{Context, Result, bail};
use serde_json::Value;
use sqlx::{PgPool, Row};

#[derive(Debug, Clone)]
pub struct BrokerageDataReport {
    pub id: String,
    pub diagnostic_id: String,
    pub created_at: String,
}

pub struct CreateBrokerageDataReport<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub workspace_id: &'a str,
    pub snaptrade_account_id: &'a str,
    pub diagnostic_id: &'a str,
    pub category: &'a str,
    pub note: Option<&'a str>,
    pub diagnostic_snapshot: &'a Value,
}

/// Persists one user-confirmed report. Ownership is checked again in SQL so a
/// future caller cannot accidentally attach a report to another user's
/// workspace. Five reports per workspace per hour is enough for distinct issues
/// while preventing an accidental retry loop from flooding the review queue.
pub async fn create(
    pool: &PgPool,
    input: CreateBrokerageDataReport<'_>,
) -> Result<BrokerageDataReport> {
    let recent_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM brokerage_data_reports \
         WHERE user_id=$1 AND workspace_id=$2 AND created_at >= now() - interval '1 hour'",
    )
    .bind(input.user_id)
    .bind(input.workspace_id)
    .fetch_one(pool)
    .await
    .context("Failed to check brokerage report limit")?;
    if recent_count >= 5 {
        bail!("Too many reports were submitted for this workspace. Try again in an hour.");
    }

    let row = sqlx::query(
        "INSERT INTO brokerage_data_reports (
             id, user_id, workspace_id, snaptrade_account_id, diagnostic_id,
             category, note, diagnostic_snapshot
         )
         SELECT $1, $2, w.id, bc.snaptrade_account_id, $4, $5, $6, $7
         FROM workspaces w
         JOIN brokerage_connections bc ON bc.workspace_id = w.id AND bc.user_id = w.user_id
         WHERE w.id = $3 AND w.user_id = $2 AND bc.snaptrade_account_id = $8
         RETURNING id, diagnostic_id,
             to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .bind(input.id)
    .bind(input.user_id)
    .bind(input.workspace_id)
    .bind(input.diagnostic_id)
    .bind(input.category)
    .bind(input.note)
    .bind(input.diagnostic_snapshot)
    .bind(input.snaptrade_account_id)
    .fetch_optional(pool)
    .await
    .context("Failed to create brokerage data report")?
    .ok_or_else(|| anyhow::anyhow!("Brokerage account not found for this workspace"))?;

    Ok(BrokerageDataReport {
        id: row.try_get(0)?,
        diagnostic_id: row.try_get(1)?,
        created_at: row.try_get(2)?,
    })
}
