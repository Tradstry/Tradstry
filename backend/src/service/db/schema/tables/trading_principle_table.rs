use anyhow::{Context, Result, ensure};
use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Row};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::playbook_table;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct TradingPrinciple {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub playbook_id: Option<String>,
    pub evidence_note_id: Option<String>,
    pub title: String,
    pub the_rule: String,
    pub why: String,
    pub intervention: Option<String>,
    pub priority: i64,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct CreatePrincipleInput {
    pub account_id: String,
    pub title: String,
    pub the_rule: String,
    pub why: String,
    pub intervention: Option<String>,
    pub playbook_id: Option<String>,
    pub evidence_note_id: Option<String>,
}

/// `account_id` is deliberately absent: moving a principle between accounts
/// would invalidate its violation history and its evidence-note link.
#[derive(Debug, Clone, InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct UpdatePrincipleInput {
    pub title: Option<String>,
    pub the_rule: Option<String>,
    pub why: Option<String>,
    pub intervention: Option<String>,
    #[graphql(default)]
    pub clear_intervention: bool,
    pub playbook_id: Option<String>,
    #[graphql(default)]
    pub clear_playbook: bool,
    pub evidence_note_id: Option<String>,
    #[graphql(default)]
    pub clear_evidence_note: bool,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone)]
struct PreparedPrinciple {
    title: String,
    the_rule: String,
    why: String,
    intervention: Option<String>,
    playbook_id: Option<String>,
    evidence_note_id: Option<String>,
    is_active: bool,
}

const SELECT_COLS: &str = "id, user_id, account_id, playbook_id, evidence_note_id, title, the_rule, why, intervention, priority, is_active, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

fn normalize_required_text(value: &str, field: &str) -> Result<String> {
    let trimmed = value.trim();
    ensure!(!trimmed.is_empty(), "{field} cannot be empty");
    Ok(trimmed.to_string())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn validate_title_length(title: &str) -> Result<()> {
    ensure!(
        title.chars().count() <= 80,
        "principle title must be 80 characters or less"
    );
    Ok(())
}

fn row_to_principle(row: &sqlx::postgres::PgRow) -> Result<TradingPrinciple> {
    Ok(TradingPrinciple {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        account_id: row.try_get::<String, _>(2)?,
        playbook_id: row.try_get::<Option<String>, _>(3)?,
        evidence_note_id: row.try_get::<Option<String>, _>(4)?,
        title: row.try_get::<String, _>(5)?,
        the_rule: row.try_get::<String, _>(6)?,
        why: row.try_get::<String, _>(7)?,
        intervention: row.try_get::<Option<String>, _>(8)?,
        priority: row.try_get::<i64, _>(9)?,
        is_active: row.try_get::<bool, _>(10)?,
        created_at: row.try_get::<String, _>(11)?,
        updated_at: row.try_get::<String, _>(12)?,
    })
}

/// Invariant 2: a referenced playbook must belong to the same user. Playbooks
/// carry no `account_id`, so no account check is possible here.
async fn ensure_playbook_owned(
    pool: &PgPool,
    user_id: &str,
    playbook_id: Option<&str>,
) -> Result<()> {
    let Some(playbook_id) = playbook_id else {
        return Ok(());
    };
    playbook_table::find_playbook(pool, playbook_id, user_id)
        .await?
        .with_context(|| format!("playbook {playbook_id} not found"))?;
    Ok(())
}

/// Invariant 1: a referenced evidence note must belong to the same user AND the
/// same account as the principle. Postgres cannot express this without a
/// redundant column, so it is checked here.
async fn ensure_note_in_account(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    evidence_note_id: Option<&str>,
) -> Result<()> {
    let Some(note_id) = evidence_note_id else {
        return Ok(());
    };
    let found: Option<String> = sqlx::query_scalar(
        "SELECT id FROM notebook_notes WHERE id = $1 AND user_id = $2 AND account_id = $3",
    )
    .bind(note_id)
    .bind(user_id)
    .bind(account_id)
    .fetch_optional(pool)
    .await
    .context("Failed to verify evidence note")?;

    ensure!(
        found.is_some(),
        "evidence note {note_id} not found in account {account_id}"
    );
    Ok(())
}

fn prepare_new_principle(input: CreatePrincipleInput) -> Result<PreparedPrinciple> {
    let title = normalize_required_text(&input.title, "title")?;
    validate_title_length(&title)?;

    Ok(PreparedPrinciple {
        title,
        the_rule: normalize_required_text(&input.the_rule, "the_rule")?,
        why: normalize_required_text(&input.why, "why")?,
        intervention: normalize_optional_text(input.intervention),
        playbook_id: normalize_optional_text(input.playbook_id),
        evidence_note_id: normalize_optional_text(input.evidence_note_id),
        is_active: true,
    })
}

fn prepare_updated_principle(
    current: &TradingPrinciple,
    input: UpdatePrincipleInput,
) -> Result<PreparedPrinciple> {
    let title = if let Some(title) = input.title {
        normalize_required_text(&title, "title")?
    } else {
        current.title.clone()
    };
    validate_title_length(&title)?;

    let intervention = if input.clear_intervention {
        None
    } else if input.intervention.is_some() {
        normalize_optional_text(input.intervention)
    } else {
        current.intervention.clone()
    };

    let playbook_id = if input.clear_playbook {
        None
    } else if input.playbook_id.is_some() {
        normalize_optional_text(input.playbook_id)
    } else {
        current.playbook_id.clone()
    };

    let evidence_note_id = if input.clear_evidence_note {
        None
    } else if input.evidence_note_id.is_some() {
        normalize_optional_text(input.evidence_note_id)
    } else {
        current.evidence_note_id.clone()
    };

    Ok(PreparedPrinciple {
        title,
        the_rule: if let Some(v) = input.the_rule {
            normalize_required_text(&v, "the_rule")?
        } else {
            current.the_rule.clone()
        },
        why: if let Some(v) = input.why {
            normalize_required_text(&v, "why")?
        } else {
            current.why.clone()
        },
        intervention,
        playbook_id,
        evidence_note_id,
        is_active: input.is_active.unwrap_or(current.is_active),
    })
}

pub async fn list_principles(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<TradingPrinciple>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM trading_principles \
         WHERE user_id = $1 AND account_id = $2 AND deleted_at IS NULL \
         ORDER BY priority DESC, created_at ASC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .fetch_all(pool)
        .await
        .context("Failed to list principles")?;

    let mut principles = Vec::new();
    for row in &rows {
        principles.push(row_to_principle(row)?);
    }
    Ok(principles)
}

pub async fn find_principle(
    pool: &PgPool,
    id: &str,
    user_id: &str,
) -> Result<Option<TradingPrinciple>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM trading_principles WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("Failed to find principle")?;

    match row {
        Some(row) => Ok(Some(row_to_principle(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_principle(
    pool: &PgPool,
    user_id: &str,
    input: CreatePrincipleInput,
) -> Result<TradingPrinciple> {
    let account_id = normalize_required_text(&input.account_id, "account_id")?;
    let prepared = prepare_new_principle(input)?;

    ensure_playbook_owned(pool, user_id, prepared.playbook_id.as_deref()).await?;
    ensure_note_in_account(
        pool,
        user_id,
        &account_id,
        prepared.evidence_note_id.as_deref(),
    )
    .await?;

    let id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO trading_principles \
         (id, user_id, account_id, playbook_id, evidence_note_id, title, the_rule, why, intervention, is_active, hlc) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(account_id.as_str())
    .bind(prepared.playbook_id.as_deref())
    .bind(prepared.evidence_note_id.as_deref())
    .bind(prepared.title.as_str())
    .bind(prepared.the_rule.as_str())
    .bind(prepared.why.as_str())
    .bind(prepared.intervention.as_deref())
    .bind(prepared.is_active)
    .bind(crate::service::hlc::stamp())
    .execute(pool)
    .await
    .context("Failed to insert principle")?;

    find_principle(pool, &id, user_id)
        .await?
        .context("Principle not found after insert")
}

pub async fn update_principle(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    input: UpdatePrincipleInput,
) -> Result<TradingPrinciple> {
    let current = find_principle(pool, id, user_id)
        .await?
        .context("Principle not found")?;
    let prepared = prepare_updated_principle(&current, input)?;

    ensure_playbook_owned(pool, user_id, prepared.playbook_id.as_deref()).await?;
    ensure_note_in_account(
        pool,
        user_id,
        &current.account_id,
        prepared.evidence_note_id.as_deref(),
    )
    .await?;

    sqlx::query(
        "UPDATE trading_principles SET title = $1, the_rule = $2, why = $3, intervention = $4, \
         playbook_id = $5, evidence_note_id = $6, is_active = $7, hlc = $8, updated_at = now() \
         WHERE id = $9 AND user_id = $10",
    )
    .bind(prepared.title.as_str())
    .bind(prepared.the_rule.as_str())
    .bind(prepared.why.as_str())
    .bind(prepared.intervention.as_deref())
    .bind(prepared.playbook_id.as_deref())
    .bind(prepared.evidence_note_id.as_deref())
    .bind(prepared.is_active)
    .bind(crate::service::hlc::stamp())
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Failed to update principle")?;

    find_principle(pool, id, user_id)
        .await?
        .context("Principle not found after update")
}

pub async fn delete_principle(pool: &PgPool, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = {
        // Soft delete, not DELETE: a hard-deleted row vanishes from the sync delta entirely, so
        // the desktop never learns it is gone and keeps showing it forever. The tombstone is
        // what propagates.
        let existed = find_principle(pool, id, user_id).await?.is_some();
        if existed {
            let mut conn = pool.acquire().await?;
            soft_delete_principle_tx(&mut conn, user_id, id, &crate::service::hlc::stamp()).await?;
        }
        u64::from(existed)
    };

    Ok(rows_affected > 0)
}

/// Assign `priority` by descending position: the first id in `ordered_ids` gets
/// the highest priority, matching the `priority DESC` index and the display order.
pub async fn reorder_principles(
    pool: &PgPool,
    user_id: &str,
    ordered_ids: &[String],
) -> Result<()> {
    let mut tx = pool.begin().await?;
    let top = ordered_ids.len() as i64;

    for (index, id) in ordered_ids.iter().enumerate() {
        let priority = top - index as i64;
        let affected = sqlx::query(
            "UPDATE trading_principles SET priority = $1, hlc = $4, updated_at = now() \
             WHERE id = $2 AND user_id = $3",
        )
        .bind(priority)
        .bind(id.as_str())
        .bind(user_id)
        .bind(crate::service::hlc::stamp())
        .execute(&mut *tx)
        .await
        .context("Failed to reorder principle")?
        .rows_affected();

        ensure!(affected == 1, "principle {id} not found");
    }

    tx.commit().await?;
    Ok(())
}

/// Replace a trade's violated-principle links with exactly the given set.
///
/// Invariant 3: every principle must belong to the caller AND govern the same
/// account as the trade. A `user_id`-only check would let a principle from the
/// user's other account attach to this trade and corrupt its violation stats.
pub async fn set_trade_principle_violations(
    pool: &PgPool,
    user_id: &str,
    journal_entry_id: &str,
    principle_ids: &[String],
) -> Result<()> {
    let trade_account_id: String =
        sqlx::query_scalar(
            "SELECT account_id FROM journal_entries WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
        )
            .bind(journal_entry_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .context("Failed to load trade for violation linking")?
            .with_context(|| format!("journal entry {journal_entry_id} not found"))?;

    for principle_id in principle_ids {
        let principle = find_principle(pool, principle_id, user_id)
            .await?
            .with_context(|| format!("principle {principle_id} not found"))?;
        ensure!(
            principle.account_id == trade_account_id,
            "principle {principle_id} governs account {} but the trade is in account {trade_account_id}",
            principle.account_id
        );
    }

    let mut tx = pool.begin().await?;

    // Read before the rewrite so re-saving a trade with the same links notifies
    // nobody: only ids that were not already stored count as newly violated.
    let mut already_linked: HashSet<String> = sqlx::query_scalar(
        "SELECT principle_id FROM trade_principle_violations WHERE journal_entry_id = $1",
    )
    .bind(journal_entry_id)
    .fetch_all(&mut *tx)
    .await
    .context("Failed to load existing trade_principle_violations")?
    .into_iter()
    .collect();

    sqlx::query("DELETE FROM trade_principle_violations WHERE journal_entry_id = $1")
        .bind(journal_entry_id)
        .execute(&mut *tx)
        .await
        .context("Failed to clear existing trade_principle_violations")?;

    for principle_id in principle_ids {
        sqlx::query(
            "INSERT INTO trade_principle_violations (journal_entry_id, principle_id) VALUES ($1, $2) \
             ON CONFLICT (journal_entry_id, principle_id) DO NOTHING",
        )
        .bind(journal_entry_id)
        .bind(principle_id.as_str())
        .execute(&mut *tx)
        .await
        .context("Failed to insert trade_principle_violation")?;
    }

    let today = chrono::Utc::now()
        .with_timezone(&chrono_tz::US::Eastern)
        .date_naive();
    for principle_id in principle_ids
        .iter()
        .filter(|id| already_linked.insert((*id).clone()))
    {
        let event = crate::service::notifications::NotificationEvent::PrincipleViolated {
            account_id: trade_account_id.clone(),
            trade_id: journal_entry_id.to_string(),
            principle_id: principle_id.clone(),
        };
        crate::service::notifications::outbox::record(&mut *tx, user_id, &event, today).await?;
    }

    // `trade_principle_violations` carries no clock of its own: it reaches the desktop only inside the
    // journal delta, which is pulled on the entry's `updated_at` cursor. Without this bump
    // the link changes on the server and the desktop never hears about it.
    sqlx::query(
        "UPDATE journal_entries SET updated_at = now(), hlc = $1 WHERE id = $2 AND user_id = $3",
    )
    .bind(crate::service::hlc::stamp())
    .bind(journal_entry_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .context("Failed to bump the trade after changing its links")?;

    tx.commit().await?;
    Ok(())
}

/// The principle ids a trade violated. Scoped to `user_id` as belt-and-suspenders.
pub async fn principles_for_trade(
    pool: &PgPool,
    user_id: &str,
    journal_entry_id: &str,
) -> Result<Vec<String>> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT v.principle_id FROM trade_principle_violations v \
         JOIN trading_principles p ON p.id = v.principle_id \
         WHERE v.journal_entry_id = $1 AND p.user_id = $2 \
         ORDER BY p.priority DESC",
    )
    .bind(journal_entry_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to load violated principles for trade")?;

    Ok(ids)
}

/// Batch: the principles each trade violated, one query. Trades with no violations are
/// absent from the map. The per-trade variant in a loop would be an N+1 across a page of
/// trades, which is exactly what a tool paging 500 rows would do.
pub async fn principles_for_trades(
    pool: &PgPool,
    user_id: &str,
    journal_entry_ids: &[String],
) -> Result<HashMap<String, Vec<String>>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    if journal_entry_ids.is_empty() {
        return Ok(map);
    }

    let rows = sqlx::query(
        "SELECT v.journal_entry_id, v.principle_id FROM trade_principle_violations v \
         JOIN trading_principles p ON p.id = v.principle_id \
         WHERE v.journal_entry_id = ANY($1) AND p.user_id = $2 AND p.deleted_at IS NULL \
         ORDER BY p.priority DESC",
    )
    .bind(journal_entry_ids)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to load violated principles for trades")?;

    for row in &rows {
        let entry_id: String = row.try_get(0)?;
        let principle_id: String = row.try_get(1)?;
        map.entry(entry_id).or_default().push(principle_id);
    }
    Ok(map)
}

/// Batch per-trade principle-violation counts for the given journal entries,
/// one query. Trades with zero violations are simply absent from the map
/// (mirrors `tags_table::tags_for_trades`).
pub async fn violation_counts_for_trades(
    pool: &PgPool,
    journal_entry_ids: &[String],
) -> Result<HashMap<String, usize>> {
    let mut map: HashMap<String, usize> = HashMap::new();
    if journal_entry_ids.is_empty() {
        return Ok(map);
    }

    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT journal_entry_id, COUNT(*) FROM trade_principle_violations \
         WHERE journal_entry_id = ANY($1) GROUP BY journal_entry_id",
    )
    .bind(journal_entry_ids)
    .fetch_all(pool)
    .await
    .context("Failed to batch-load principle violation counts for trades")?;

    for (id, count) in rows {
        map.insert(id, count as usize);
    }
    Ok(map)
}

// ---- Offline-first sync (whole-row LWW + soft-delete) --------------------

/// The editable payload a `createPrinciple`/`updatePrinciple` mutation
/// carries. Whole-row LWW: the server writes every writable column plus the
/// client's `hlc`, and conflict resolution is entirely client-side (unlike
/// `create_principle`/`update_principle`, there is no server-side partial
/// merge or FK validation — the client already resolved the merge locally).
pub struct PrincipleWriteArgs {
    pub id: String,
    pub account_id: String,
    pub playbook_id: Option<String>,
    pub evidence_note_id: Option<String>,
    pub title: String,
    pub the_rule: String,
    pub why: String,
    pub intervention: Option<String>,
    pub is_active: bool,
    pub priority: i64,
}

/// Raw principle fields only — NOT the derived violation stats
/// (`violation_count`/`violated_cumulative_profit`/`violated_cumulative_roi`/
/// `violated_win_rate`), which the desktop computes on-device from local
/// trades, same as `journal_table::JournalDelta` excludes nothing derived but
/// mirrors the "delta carries source-of-truth columns" shape.
#[derive(Debug, Clone)]
pub struct PrincipleDelta {
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

const DELTA_COLS: &str = "id, account_id, playbook_id, evidence_note_id, title, the_rule, why, intervention, priority, is_active, hlc, \
    to_char(deleted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS deleted_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS updated_at";

pub async fn create_principle_tx(
    conn: &mut PgConnection,
    user_id: &str,
    args: &PrincipleWriteArgs,
    hlc: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO trading_principles \
         (id, user_id, account_id, playbook_id, evidence_note_id, title, the_rule, why, intervention, priority, is_active, hlc) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(&args.id)
    .bind(user_id)
    .bind(&args.account_id)
    .bind(args.playbook_id.as_deref())
    .bind(args.evidence_note_id.as_deref())
    .bind(args.title.as_str())
    .bind(args.the_rule.as_str())
    .bind(args.why.as_str())
    .bind(args.intervention.as_deref())
    .bind(args.priority)
    .bind(args.is_active)
    .bind(hlc)
    .execute(&mut *conn)
    .await
    .context("create_principle_tx")?;
    Ok(())
}

pub async fn update_principle_tx(
    conn: &mut PgConnection,
    user_id: &str,
    args: &PrincipleWriteArgs,
    hlc: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE trading_principles SET account_id = $1, playbook_id = $2, evidence_note_id = $3, \
         title = $4, the_rule = $5, why = $6, intervention = $7, priority = $8, is_active = $9, \
         hlc = $10, updated_at = now() \
         WHERE id = $11 AND user_id = $12",
    )
    .bind(&args.account_id)
    .bind(args.playbook_id.as_deref())
    .bind(args.evidence_note_id.as_deref())
    .bind(args.title.as_str())
    .bind(args.the_rule.as_str())
    .bind(args.why.as_str())
    .bind(args.intervention.as_deref())
    .bind(args.priority)
    .bind(args.is_active)
    .bind(hlc)
    .bind(&args.id)
    .bind(user_id)
    .execute(&mut *conn)
    .await
    .context("update_principle_tx")?;
    Ok(())
}

pub async fn soft_delete_principle_tx(
    conn: &mut PgConnection,
    user_id: &str,
    id: &str,
    hlc: &str,
) -> Result<()> {
    // A hard DELETE used to take the violation links with it via ON DELETE CASCADE. A soft
    // delete does not cascade, so drop them here — otherwise a trade keeps reporting a
    // violation of a principle that no longer exists.
    //
    // `trade_principle_violations` has no `hlc` or `updated_at` of its own: it reaches the
    // desktop only inside the journal delta (`violated_principle_ids`), which is pulled on
    // the entry's `updated_at` cursor. So the affected entries must be bumped, or the link
    // disappears on the server and lives on forever on the desktop.
    sqlx::query(
        "UPDATE journal_entries SET updated_at = now(), hlc = $1 \
         WHERE user_id = $2 AND id IN ( \
             SELECT journal_entry_id FROM trade_principle_violations WHERE principle_id = $3 \
         )",
    )
    .bind(hlc)
    .bind(user_id)
    .bind(id)
    .execute(&mut *conn)
    .await
    .context("soft_delete_principle_tx: bump violated entries")?;

    sqlx::query("DELETE FROM trade_principle_violations WHERE principle_id = $1")
        .bind(id)
        .execute(&mut *conn)
        .await
        .context("soft_delete_principle_tx: clear violations")?;

    sqlx::query(
        "UPDATE trading_principles SET deleted_at = now(), hlc = $1 \
         WHERE id = $2 AND user_id = $3 AND deleted_at IS NULL",
    )
    .bind(hlc)
    .bind(id)
    .bind(user_id)
    .execute(&mut *conn)
    .await
    .context("soft_delete_principle_tx")?;
    Ok(())
}

/// Assigns each id in `ordered_ids` an absolute priority equal to its index
/// (0-based, in list order). Unlike the web `reorder_principles` (which
/// inverts to `top - index` so `priority DESC` reads top-first), the offline
/// client is the sole source of truth for display order here and reads the
/// raw `priority` value back; direction is a client-side rendering choice.
pub async fn reorder_principles_tx(
    conn: &mut PgConnection,
    user_id: &str,
    ordered_ids: &[String],
    hlc: &str,
) -> Result<()> {
    // Match the web `reorder_principles` convention: first id = highest priority
    // (`top - index`), and both surfaces list DESC, so order agrees cross-device.
    let top = ordered_ids.len() as i64;
    for (index, id) in ordered_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE trading_principles SET priority = $1, hlc = $2, updated_at = now() \
             WHERE id = $3 AND user_id = $4",
        )
        .bind(top - index as i64)
        .bind(hlc)
        .bind(id.as_str())
        .bind(user_id)
        .execute(&mut *conn)
        .await
        .context("reorder_principles_tx")?;
    }
    Ok(())
}

/// Account-scoped pull deltas (principles are account-scoped, like journal
/// entries). Deliberately does NOT filter `deleted_at IS NULL`: a client that
/// never sees a tombstone can't distinguish "deleted" from "not yet pushed".
/// `>=` (not `>`) re-sends the cursor boundary row, which is harmless because
/// client apply is idempotent.
pub async fn principles_since(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    cookie: Option<&str>,
) -> Result<Vec<PrincipleDelta>> {
    // A first pull that saw no rows returns `""` as the cursor (unwrap_or_default),
    // and `''::timestamptz` throws. Treat an empty cookie as "no cursor".
    let cookie = cookie.filter(|c| !c.is_empty());
    let sql = format!(
        "SELECT {DELTA_COLS} FROM trading_principles \
         WHERE user_id = $1 AND account_id = $2 AND ($3::text IS NULL OR updated_at >= $3::timestamptz) \
         ORDER BY updated_at ASC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .bind(cookie)
        .fetch_all(pool)
        .await
        .context("Failed to read principle deltas")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(PrincipleDelta {
            id: row.try_get("id")?,
            account_id: row.try_get("account_id")?,
            playbook_id: row.try_get("playbook_id")?,
            evidence_note_id: row.try_get("evidence_note_id")?,
            title: row.try_get("title")?,
            the_rule: row.try_get("the_rule")?,
            why: row.try_get("why")?,
            intervention: row.try_get("intervention")?,
            priority: row.try_get("priority")?,
            is_active: row.try_get("is_active")?,
            hlc: row.try_get("hlc")?,
            deleted_at: row.try_get("deleted_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{normalize_optional_text, normalize_required_text, validate_title_length};

    #[test]
    fn blank_intervention_becomes_none() {
        assert_eq!(normalize_optional_text(Some("   ".to_string())), None);
        assert_eq!(
            normalize_optional_text(Some("  walk away  ".to_string())),
            Some("walk away".to_string())
        );
    }

    #[test]
    fn title_must_not_be_blank() {
        assert!(normalize_required_text("  ", "title").is_err());
        assert_eq!(
            normalize_required_text("  30-min rule ", "title").unwrap(),
            "30-min rule"
        );
    }

    #[test]
    fn title_over_80_chars_is_rejected() {
        let long = "x".repeat(81);
        assert!(validate_title_length(&long).is_err());
        assert!(validate_title_length(&"x".repeat(80)).is_ok());
    }
}
