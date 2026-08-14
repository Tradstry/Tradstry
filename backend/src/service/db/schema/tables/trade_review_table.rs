use std::collections::{BTreeSet, HashSet};

use anyhow::{Context, Result, anyhow, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::service::db::schema::tables::{
    brokerage_table, manual_execution_claim_table, position_calculator_plans_table,
};
use crate::service::trade_review::types::{
    EpisodeDirection, ExecutionFill, ExecutionInstrument, ExecutionSide, FillAllocation, FillRole,
    PlanSnapshot, PlanTranche, TradeEpisodeDraft,
};
use crate::service::trade_review::{
    build_episodes, calculate_review, reconcile_tranches, suggest_plan_match,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeReviewInboxItem {
    pub episode_id: String,
    pub instrument_key: String,
    pub direction: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub current_quantity: String,
    pub status: String,
    pub block_reason: Option<String>,
    pub match_status: Option<String>,
    pub confirmed_match_id: Option<String>,
    pub confirmed_plan_id: Option<String>,
    pub suggestions_json: String,
    pub latest_review_json: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoredEpisode {
    pub id: String,
    pub workspace_id: String,
    pub grouping_source: String,
    pub draft: TradeEpisodeDraft,
}

#[derive(Debug, Clone)]
pub struct PublishEpisodeReviewInput {
    pub episode_id: String,
    pub plan_id: Option<String>,
    pub stop_loss: Option<f64>,
    pub playbook_id: Option<String>,
    pub notes: Option<String>,
    pub plan_adherence: Option<String>,
    pub lesson: Option<String>,
    pub tag_ids: Vec<String>,
    pub violated_principle_ids: Vec<String>,
}

pub async fn rebuild_workspace(pool: &PgPool, user_id: &str, workspace_id: &str) -> Result<usize> {
    let transactions = brokerage_table::list_all_for_lifecycle(pool, user_id, workspace_id).await?;
    let manually_grouped_transaction_ids: HashSet<String> = sqlx::query_scalar(
        "SELECT DISTINCT f.brokerage_transaction_id
         FROM trade_episode_fills f
         JOIN trade_episodes e ON e.id=f.episode_id
         WHERE e.user_id=$1 AND e.workspace_id=$2 AND e.grouping_source='manual'",
    )
    .bind(user_id)
    .bind(workspace_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();
    let mut fills = Vec::new();
    for transaction in &transactions {
        if manually_grouped_transaction_ids.contains(&transaction.id) {
            continue;
        }
        if let Some(fill) = transaction_to_execution_fill(transaction) {
            fills.push(fill);
        }
    }

    let episodes = build_episodes(fills).context("failed to build deterministic trade episodes")?;
    let mut tx = pool.begin().await?;
    for episode in &episodes {
        let id = Uuid::new_v4().to_string();
        let instrument_json = serde_json::to_value(&episode.instrument)?;
        let direction = direction_str(episode.direction);
        let row = sqlx::query(
            "INSERT INTO trade_episodes
             (id,user_id,workspace_id,fingerprint,instrument_key,instrument_json,direction,opened_at,closed_at,current_quantity)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
             ON CONFLICT (user_id,workspace_id,fingerprint) DO UPDATE SET
               closed_at=EXCLUDED.closed_at,current_quantity=EXCLUDED.current_quantity,updated_at=now()
             RETURNING id",
        )
        .bind(&id)
        .bind(user_id)
        .bind(workspace_id)
        .bind(&episode.fingerprint)
        .bind(episode.instrument.key())
        .bind(instrument_json)
        .bind(direction)
        .bind(episode.opened_at)
        .bind(episode.closed_at)
        .bind(episode.current_quantity)
        .fetch_one(&mut *tx)
        .await?;
        let episode_id: String = row.try_get(0)?;
        for allocation in &episode.allocations {
            let fill_id = format!(
                "{}:{}:{}",
                episode_id,
                allocation.transaction_id,
                role_str(allocation.role)
            );
            sqlx::query(
                "INSERT INTO trade_episode_fills
                 (id,episode_id,brokerage_transaction_id,role,quantity,price,fee,executed_at)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
                 ON CONFLICT (episode_id,brokerage_transaction_id,role) DO UPDATE SET
                   quantity=EXCLUDED.quantity,price=EXCLUDED.price,fee=EXCLUDED.fee,executed_at=EXCLUDED.executed_at",
            )
            .bind(fill_id)
            .bind(&episode_id)
            .bind(&allocation.transaction_id)
            .bind(role_str(allocation.role))
            .bind(allocation.quantity)
            .bind(allocation.price)
            .bind(allocation.fee)
            .bind(allocation.executed_at)
            .execute(&mut *tx)
            .await?;
        }
    }
    let fingerprints: Vec<String> = episodes
        .iter()
        .map(|episode| episode.fingerprint.clone())
        .collect();
    sqlx::query(
        "DELETE FROM trade_episodes e
         WHERE e.user_id=$1 AND e.workspace_id=$2
           AND e.grouping_source='automatic'
           AND NOT (e.fingerprint = ANY($3::text[]))
           AND NOT EXISTS (SELECT 1 FROM trade_episode_matches m WHERE m.episode_id=e.id AND m.status='confirmed')
           AND NOT EXISTS (SELECT 1 FROM brokerage_episode_publications p WHERE p.episode_id=e.id)",
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(&fingerprints)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    refresh_suggestions(pool, user_id, workspace_id).await?;
    manual_execution_claim_table::reconcile_confirmed_matches_for_workspace(
        pool,
        user_id,
        workspace_id,
    )
    .await?;
    Ok(episodes.len())
}

fn transaction_to_execution_fill(
    transaction: &brokerage_table::BrokerageTransaction,
) -> Option<ExecutionFill> {
    let upper = transaction.transaction_type.to_ascii_uppercase();
    let side = if upper.starts_with("BUY") {
        ExecutionSide::Buy
    } else if upper.starts_with("SELL") {
        ExecutionSide::Sell
    } else {
        return None;
    };
    let symbol = transaction
        .symbol
        .clone()
        .filter(|symbol| !symbol.trim().is_empty())?;
    let executed_at = transaction.trade_date.as_deref().and_then(parse_datetime)?;
    let price = Decimal::from_f64_retain(transaction.price)?;
    let quantity = Decimal::from_f64_retain(transaction.units.abs())?;
    let fee = Decimal::from_f64_retain(transaction.fee).unwrap_or(Decimal::ZERO);
    let instrument = if let (Some(underlying), Some(kind), Some(strike), Some(expiration)) = (
        transaction.underlying_symbol.clone(),
        transaction.option_kind.clone(),
        transaction.strike_price,
        transaction.option_expiration.as_deref(),
    ) {
        let expiration = NaiveDate::parse_from_str(expiration, "%Y-%m-%d").ok()?;
        let strike = Decimal::from_f64_retain(strike)?;
        ExecutionInstrument::Option {
            underlying,
            expiration,
            strike,
            option_kind: kind,
            multiplier: Decimal::from_f64_retain(transaction.contract_multiplier)
                .filter(|value| *value > Decimal::ZERO)
                .unwrap_or(Decimal::new(100, 0)),
        }
    } else {
        ExecutionInstrument::Equity { symbol }
    };
    Some(ExecutionFill {
        transaction_id: transaction.id.clone(),
        instrument,
        side,
        price,
        quantity,
        fee,
        executed_at,
    })
}

pub async fn regroup_episode(
    pool: &PgPool,
    user_id: &str,
    episode_id: &str,
    transaction_ids: Vec<String>,
) -> Result<String> {
    let requested_ids: BTreeSet<String> = transaction_ids
        .into_iter()
        .filter(|id| !id.trim().is_empty())
        .collect();
    ensure!(!requested_ids.is_empty(), "select at least one broker fill");

    let original =
        sqlx::query("SELECT workspace_id FROM trade_episodes WHERE id=$1 AND user_id=$2")
            .bind(episode_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| anyhow!("trade grouping not found"))?;
    let workspace_id: String = original.try_get(0)?;
    let requested: Vec<String> = requested_ids.into_iter().collect();
    let transactions = brokerage_table::get_transactions_by_ids(pool, user_id, &requested).await?;
    ensure!(
        transactions.len() == requested.len(),
        "one or more selected fills were not found"
    );
    ensure!(
        transactions
            .iter()
            .all(|transaction| transaction.workspace_id == workspace_id),
        "all selected fills must belong to this brokerage account"
    );

    let fills: Vec<ExecutionFill> = transactions
        .iter()
        .map(|transaction| {
            transaction_to_execution_fill(transaction)
                .ok_or_else(|| anyhow!("only executed buy and sell fills can be grouped"))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut episodes = build_episodes(fills).context("failed to validate the selected fills")?;
    ensure!(
        episodes.len() == 1,
        "selected fills must describe exactly one trade and one instrument"
    );
    let draft = episodes.pop().expect("one episode checked above");
    ensure!(
        draft.closed_at.is_some() && draft.current_quantity == Decimal::ZERO,
        "selected fills do not close the trade"
    );

    let mut tx = pool.begin().await?;
    sqlx::query(
        "SELECT id FROM brokerage_transactions
         WHERE id=ANY($1) AND user_id=$2 AND workspace_id=$3 FOR UPDATE",
    )
    .bind(&requested)
    .bind(user_id)
    .bind(&workspace_id)
    .fetch_all(&mut *tx)
    .await?;
    let linked_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_brokerage_links
         WHERE brokerage_transaction_id=ANY($1) AND user_id=$2",
    )
    .bind(&requested)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        linked_count == 0,
        "one or more selected fills are already journaled"
    );
    let other_manual_group_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT e.id)
         FROM trade_episodes e
         JOIN trade_episode_fills f ON f.episode_id=e.id
         WHERE e.user_id=$1 AND e.workspace_id=$2 AND e.grouping_source='manual'
           AND e.id<>$3 AND f.brokerage_transaction_id=ANY($4)",
    )
    .bind(user_id)
    .bind(&workspace_id)
    .bind(episode_id)
    .bind(&requested)
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        other_manual_group_count == 0,
        "one or more selected fills belong to another manual grouping"
    );
    let published: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM brokerage_episode_publications
         WHERE episode_id=$1 AND user_id=$2)",
    )
    .bind(episode_id)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    ensure!(!published, "a published trade grouping cannot be changed");

    sqlx::query(
        "DELETE FROM trade_episodes e
         WHERE e.user_id=$1 AND e.workspace_id=$2
           AND e.id<>$3 AND e.grouping_source='automatic'
           AND EXISTS (
             SELECT 1 FROM trade_episode_fills f
             WHERE f.episode_id=e.id AND f.brokerage_transaction_id=ANY($4)
           )
           AND NOT EXISTS (
             SELECT 1 FROM brokerage_episode_publications p WHERE p.episode_id=e.id
           )",
    )
    .bind(user_id)
    .bind(&workspace_id)
    .bind(episode_id)
    .bind(&requested)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE manual_execution_claims SET status='pending',reconciled_match_id=NULL,updated_at=now()
         WHERE user_id=$1 AND reconciled_match_id IN (
           SELECT id FROM trade_episode_matches WHERE episode_id=$2 AND user_id=$1
         )",
    )
    .bind(user_id)
    .bind(episode_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM trade_episode_matches WHERE episode_id=$1 AND user_id=$2")
        .bind(episode_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM trade_episode_fills WHERE episode_id=$1")
        .bind(episode_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE trade_episodes SET
           fingerprint=$1,instrument_key=$2,instrument_json=$3,direction=$4,
           opened_at=$5,closed_at=$6,current_quantity=$7,grouping_source='manual',
           status='ready',block_reason=NULL,updated_at=now()
         WHERE id=$8 AND user_id=$9",
    )
    .bind(format!("manual:{episode_id}"))
    .bind(draft.instrument.key())
    .bind(serde_json::to_value(&draft.instrument)?)
    .bind(direction_str(draft.direction))
    .bind(draft.opened_at)
    .bind(draft.closed_at)
    .bind(draft.current_quantity)
    .bind(episode_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    for allocation in &draft.allocations {
        sqlx::query(
            "INSERT INTO trade_episode_fills
             (id,episode_id,brokerage_transaction_id,role,quantity,price,fee,executed_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(format!(
            "{}:{}:{}",
            episode_id,
            allocation.transaction_id,
            role_str(allocation.role)
        ))
        .bind(episode_id)
        .bind(&allocation.transaction_id)
        .bind(role_str(allocation.role))
        .bind(allocation.quantity)
        .bind(allocation.price)
        .bind(allocation.fee)
        .bind(allocation.executed_at)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    rebuild_workspace(pool, user_id, &workspace_id).await?;
    Ok(episode_id.to_string())
}

pub async fn reset_episode_grouping(
    pool: &PgPool,
    user_id: &str,
    episode_id: &str,
) -> Result<bool> {
    let workspace_id = sqlx::query_scalar::<_, String>(
        "DELETE FROM trade_episodes
         WHERE id=$1 AND user_id=$2 AND grouping_source='manual'
           AND NOT EXISTS (
             SELECT 1 FROM brokerage_episode_publications p
             WHERE p.episode_id=trade_episodes.id
           )
         RETURNING workspace_id",
    )
    .bind(episode_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some(workspace_id) = workspace_id else {
        return Ok(false);
    };
    rebuild_workspace(pool, user_id, &workspace_id).await?;
    Ok(true)
}

pub async fn request_execution_check(pool: &PgPool, user_id: &str, plan_id: &str) -> Result<usize> {
    let row = sqlx::query(
        "UPDATE position_calculator_plans SET execution_check_requested_at=now()
         WHERE id=$1 AND user_id=$2 RETURNING workspace_id",
    )
    .bind(plan_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("plan not found"))?;
    let workspace_id: String = row.try_get(0)?;
    rebuild_workspace(pool, user_id, &workspace_id).await
}

pub async fn list_inbox(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
) -> Result<Vec<TradeReviewInboxItem>> {
    let episode_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM trade_episodes WHERE user_id=$1 AND workspace_id=$2",
    )
    .bind(user_id)
    .bind(workspace_id)
    .fetch_one(pool)
    .await?;
    if episode_count == 0 {
        rebuild_workspace(pool, user_id, workspace_id).await?;
    } else {
        refresh_suggestions(pool, user_id, workspace_id).await?;
    }
    let rows = sqlx::query(
        "SELECT e.id,e.instrument_key,e.direction,e.opened_at,e.closed_at,e.current_quantity,e.status,e.block_reason,
                cm.status,cm.id,cm.plan_id,
                COALESCE((SELECT jsonb_agg(jsonb_build_object('matchId',m.id,'planId',m.plan_id,'score',m.score::text,'evidence',m.evidence_json) ORDER BY m.score,m.plan_id)
                          FROM trade_episode_matches m WHERE m.episode_id=e.id AND m.status='suggested'),'[]'::jsonb)::text,
                (SELECT jsonb_build_object('id',v.id,'stage',v.stage,'version',v.version_number,'calculation',v.calculation_json,'reflection',v.reflection_json,'journalDraft',v.journal_draft_json,
                                           'journalEntryId',(SELECT p.journal_entry_id FROM trade_review_publications p WHERE p.review_version_id=v.id))::text
                   FROM trade_review_versions v JOIN trade_episode_matches vm ON vm.id=v.match_id
                  WHERE vm.episode_id=e.id ORDER BY v.version_number DESC LIMIT 1)
         FROM trade_episodes e
         LEFT JOIN trade_episode_matches cm ON cm.episode_id=e.id AND cm.status='confirmed'
         WHERE e.user_id=$1 AND e.workspace_id=$2
         ORDER BY e.opened_at DESC",
    )
    .bind(user_id)
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(TradeReviewInboxItem {
                episode_id: row.try_get(0)?,
                instrument_key: row.try_get(1)?,
                direction: row.try_get(2)?,
                opened_at: row.try_get::<DateTime<Utc>, _>(3)?.to_rfc3339(),
                closed_at: row
                    .try_get::<Option<DateTime<Utc>>, _>(4)?
                    .map(|date| date.to_rfc3339()),
                current_quantity: row.try_get::<Decimal, _>(5)?.normalize().to_string(),
                status: row.try_get(6)?,
                block_reason: row.try_get(7)?,
                match_status: row.try_get(8)?,
                confirmed_match_id: row.try_get(9)?,
                confirmed_plan_id: row.try_get(10)?,
                suggestions_json: row.try_get(11)?,
                latest_review_json: row.try_get(12)?,
            })
        })
        .collect()
}

/// Computes the exact review payload that would be frozen for this
/// episode-plan pair without confirming the match or writing a review version.
/// The preview is intentionally read-only so selecting a plan in the brokerage
/// review UI never changes authoritative matching state.
pub async fn preview_review_json(
    pool: &PgPool,
    user_id: &str,
    episode_id: &str,
    plan_id: &str,
) -> Result<String> {
    let plan = load_plan_snapshot(pool, user_id, plan_id).await?;
    let episode = load_episode(pool, user_id, episode_id).await?;
    ensure!(
        plan.workspace_id == episode.workspace_id,
        "the plan and broker trade belong to different brokerage accounts"
    );
    ensure!(
        plan.instrument.key() == episode.draft.instrument.key()
            && plan.direction == episode.draft.direction,
        "the plan does not match this broker instrument and direction"
    );
    let entries: Vec<_> = episode.draft.entry_allocations().cloned().collect();
    let reconciliation = reconcile_tranches(&plan.tranches, &entries);
    let calculation = calculate_review(&plan, &episode.draft, &reconciliation)
        .ok_or_else(|| anyhow!("cannot calculate a review without planned and actual entries"))?;
    Ok(serde_json::to_string(&calculation)?)
}

pub async fn confirm_match(
    pool: &PgPool,
    user_id: &str,
    episode_id: &str,
    plan_id: &str,
) -> Result<String> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE manual_execution_claims c
         SET status='pending',reconciled_match_id=NULL,updated_at=now()
         FROM trade_episode_matches m
         WHERE c.reconciled_match_id=m.id AND m.episode_id=$1 AND c.user_id=$2",
    )
    .bind(episode_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE trade_episode_matches SET status='rejected',updated_at=now()
         WHERE episode_id=$1 AND user_id=$2 AND status='confirmed'",
    )
    .bind(episode_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    let row = sqlx::query(
        "UPDATE trade_episode_matches SET status='confirmed',updated_at=now()
         WHERE episode_id=$1 AND plan_id=$2 AND user_id=$3
         RETURNING id,workspace_id",
    )
    .bind(episode_id)
    .bind(plan_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| anyhow!("the plan is not an eligible deterministic match"))?;
    let match_id: String = row.try_get(0)?;
    let workspace_id: String = row.try_get(1)?;
    tx.commit().await?;

    manual_execution_claim_table::reconcile_for_confirmed_match(
        pool, user_id, &match_id, episode_id, plan_id,
    )
    .await?;

    create_review_version(
        pool,
        user_id,
        &workspace_id,
        &match_id,
        episode_id,
        plan_id,
        "entry",
        None,
        None,
    )
    .await
}

pub async fn finalize_review(
    pool: &PgPool,
    user_id: &str,
    match_id: &str,
    reflection_json: &str,
    journal_draft_json: Option<&str>,
    no_additional_context: bool,
) -> Result<String> {
    let reflection: serde_json::Value =
        serde_json::from_str(reflection_json).context("reflection must be valid JSON")?;
    let has_context = reflection
        .as_object()
        .map(|object| {
            object.values().any(|value| match value {
                serde_json::Value::String(text) => !text.trim().is_empty(),
                serde_json::Value::Array(values) => !values.is_empty(),
                serde_json::Value::Null => false,
                _ => true,
            })
        })
        .unwrap_or(false);
    if !no_additional_context && !has_context {
        return Err(anyhow!(
            "respond to the review flags or choose no additional context"
        ));
    }
    let journal_draft = journal_draft_json
        .map(serde_json::from_str)
        .transpose()
        .context("journal draft must be valid JSON")?;
    let row = sqlx::query(
        "SELECT m.workspace_id,m.episode_id,m.plan_id,e.closed_at
         FROM trade_episode_matches m JOIN trade_episodes e ON e.id=m.episode_id
         WHERE m.id=$1 AND m.user_id=$2 AND m.status='confirmed'",
    )
    .bind(match_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("confirmed match not found"))?;
    let closed_at: Option<DateTime<Utc>> = row.try_get(3)?;
    if closed_at.is_none() {
        return Err(anyhow!(
            "the broker position is still open; final review requires a closing fill"
        ));
    }
    create_review_version(
        pool,
        user_id,
        row.try_get::<String, _>(0)?.as_str(),
        match_id,
        row.try_get::<String, _>(1)?.as_str(),
        row.try_get::<String, _>(2)?.as_str(),
        "final",
        Some(reflection),
        journal_draft,
    )
    .await
}

pub async fn publish_review(pool: &PgPool, user_id: &str, match_id: &str) -> Result<String> {
    let row = sqlx::query(
        "SELECT v.id,v.reflection_json,v.journal_draft_json,m.workspace_id,m.episode_id,m.plan_id
         FROM trade_review_versions v JOIN trade_episode_matches m ON m.id=v.match_id
         WHERE v.match_id=$1 AND v.user_id=$2 AND v.stage='final'
         ORDER BY v.version_number DESC LIMIT 1",
    )
    .bind(match_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("finalized review not found"))?;
    let review_version_id: String = row.try_get(0)?;
    if let Some(existing) = sqlx::query_scalar::<_, String>(
        "SELECT journal_entry_id FROM trade_review_publications WHERE review_version_id=$1",
    )
    .bind(&review_version_id)
    .fetch_optional(pool)
    .await?
    {
        return Ok(existing);
    }
    let reflection: Option<serde_json::Value> = row.try_get(1)?;
    let draft: Option<serde_json::Value> = row.try_get(2)?;
    let workspace_id: String = row.try_get(3)?;
    let episode_id: String = row.try_get(4)?;
    let plan_id: String = row.try_get(5)?;
    let episode = load_episode(pool, user_id, &episode_id).await?;
    let plan = load_plan_snapshot(pool, user_id, &plan_id).await?;
    let closed_at = episode.draft.closed_at.ok_or_else(|| {
        anyhow!("the broker position is still open; final review can be published after it closes")
    })?;
    let entry_quantity = episode.draft.entry_quantity();
    let entry_price = weighted_price(episode.draft.entry_allocations(), entry_quantity)?;
    let exit_quantity: Decimal = episode
        .draft
        .exit_allocations()
        .map(|fill| fill.quantity)
        .sum();
    let exit_price = weighted_price(episode.draft.exit_allocations(), exit_quantity)?;
    let (symbol, multiplier) = match &episode.draft.instrument {
        ExecutionInstrument::Equity { symbol } => (symbol.clone(), Decimal::ONE),
        ExecutionInstrument::Option {
            underlying,
            multiplier,
            ..
        } => (underlying.clone(), *multiplier),
    };
    let violated_principle_ids = reflection
        .as_ref()
        .and_then(|value| value.get("violatedPrincipleIds"))
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let tag_ids = draft
        .as_ref()
        .and_then(|value| value.get("tagIds"))
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let notes = draft
        .as_ref()
        .and_then(|value| value.get("notes"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            reflection
                .as_ref()
                .and_then(|value| value.get("deviationReason"))
                .and_then(serde_json::Value::as_str)
                .map(|reason| format!("Plan vs actual review: {reason}"))
        });
    let journal_id = Uuid::new_v4().to_string();
    let args = crate::service::db::schema::tables::journal_table::JournalWriteArgs {
        id: journal_id.clone(),
        workspace_id,
        open_date: episode.draft.opened_at.to_rfc3339(),
        close_date: closed_at.to_rfc3339(),
        entry_price: entry_price
            .to_f64()
            .ok_or_else(|| anyhow!("entry price cannot be represented"))?,
        exit_price: exit_price
            .to_f64()
            .ok_or_else(|| anyhow!("exit price cannot be represented"))?,
        position_size: entry_quantity
            .to_f64()
            .ok_or_else(|| anyhow!("position size cannot be represented"))?,
        stop_loss: plan.stop_loss.to_f64(),
        symbol: symbol.clone(),
        symbol_name: symbol,
        trade_type: direction_str(episode.draft.direction).to_string(),
        playbook_id: draft
            .as_ref()
            .and_then(|value| value.get("playbookId"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        notes,
        broke_30min_rule: None,
        pre_trade_conviction: None,
        market_regime: None,
        is_planned_pre_market: None,
        revenge_trade: None,
        rule_adherence_score: None,
        tag_ids,
        violated_principle_ids,
        contract_multiplier: multiplier.to_f64().unwrap_or(1.0),
    };
    let transaction_ids: Vec<String> = episode
        .draft
        .allocations
        .iter()
        .map(|fill| fill.transaction_id.clone())
        .collect();
    let link_ids: Vec<String> = transaction_ids
        .iter()
        .map(|_| Uuid::new_v4().to_string())
        .collect();
    let mut tx = pool.begin().await?;
    crate::service::db::schema::tables::journal_table::create_journal_entry_tx(
        &mut tx,
        user_id,
        &args,
        &crate::service::hlc::stamp(),
    )
    .await?;
    sqlx::query(
        "INSERT INTO journal_brokerage_links (id,journal_entry_id,brokerage_transaction_id,user_id)
         SELECT link_id,$3,transaction_id,$4
         FROM unnest($1::text[],$2::text[]) AS links(link_id,transaction_id)
         ON CONFLICT DO NOTHING",
    )
    .bind(&link_ids)
    .bind(&transaction_ids)
    .bind(&journal_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO trade_review_publications (review_version_id,journal_entry_id) VALUES ($1,$2)",
    )
    .bind(&review_version_id)
    .bind(&journal_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE position_calculator_plans SET status='completed' WHERE id=$1 AND user_id=$2",
    )
    .bind(&plan_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(journal_id)
}

pub async fn publish_episode_review(
    pool: &PgPool,
    user_id: &str,
    input: PublishEpisodeReviewInput,
) -> Result<String> {
    if let Some(existing) = sqlx::query_scalar::<_, String>(
        "SELECT journal_entry_id FROM brokerage_episode_publications
         WHERE episode_id=$1 AND user_id=$2",
    )
    .bind(&input.episode_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    {
        return Ok(existing);
    }

    let episode = load_episode(pool, user_id, &input.episode_id).await?;
    ensure!(
        episode.draft.closed_at.is_some(),
        "the broker position is still open"
    );

    if let Some(plan_id) = input.plan_id.as_deref() {
        if let Some(existing) = sqlx::query_scalar::<_, String>(
            "SELECT p.journal_entry_id
             FROM trade_review_publications p
             JOIN trade_review_versions v ON v.id=p.review_version_id
             JOIN trade_episode_matches m ON m.id=v.match_id
             WHERE m.episode_id=$1 AND m.user_id=$2
             ORDER BY p.created_at DESC LIMIT 1",
        )
        .bind(&input.episode_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        {
            record_episode_publication(
                pool,
                user_id,
                &episode.workspace_id,
                &input.episode_id,
                &existing,
                Some(plan_id),
            )
            .await?;
            return Ok(existing);
        }

        let match_id = sqlx::query_scalar::<_, String>(
            "SELECT id FROM trade_episode_matches
             WHERE episode_id=$1 AND plan_id=$2 AND user_id=$3 AND status='confirmed'",
        )
        .bind(&input.episode_id)
        .bind(plan_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        let match_id = match match_id {
            Some(id) => id,
            None => {
                confirm_match(pool, user_id, &input.episode_id, plan_id).await?;
                sqlx::query_scalar::<_, String>(
                    "SELECT id FROM trade_episode_matches
                     WHERE episode_id=$1 AND plan_id=$2 AND user_id=$3 AND status='confirmed'",
                )
                .bind(&input.episode_id)
                .bind(plan_id)
                .bind(user_id)
                .fetch_one(pool)
                .await?
            }
        };
        let reflection = serde_json::json!({
            "planAdherence": input.plan_adherence,
            "lesson": input.lesson,
            "notes": input.notes,
            "deviationReason": input.notes,
            "violatedPrincipleIds": input.violated_principle_ids,
        });
        let journal_draft = serde_json::json!({
            "playbookId": input.playbook_id,
            "notes": compose_review_notes(
                input.plan_adherence.as_deref(),
                input.lesson.as_deref(),
                input.notes.as_deref(),
            ),
            "tagIds": input.tag_ids,
        });
        finalize_review(
            pool,
            user_id,
            &match_id,
            &reflection.to_string(),
            Some(&journal_draft.to_string()),
            true,
        )
        .await?;
        let journal_id = publish_review(pool, user_id, &match_id).await?;
        record_episode_publication(
            pool,
            user_id,
            &episode.workspace_id,
            &input.episode_id,
            &journal_id,
            Some(plan_id),
        )
        .await?;
        return Ok(journal_id);
    }

    let confirmed_match_id = sqlx::query_scalar::<_, String>(
        "SELECT id FROM trade_episode_matches
         WHERE episode_id=$1 AND user_id=$2 AND status='confirmed'",
    )
    .bind(&input.episode_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    if let Some(match_id) = confirmed_match_id {
        let mut tx = pool.begin().await?;
        sqlx::query(
            "UPDATE manual_execution_claims
             SET status='pending',reconciled_match_id=NULL,updated_at=now()
             WHERE user_id=$1 AND reconciled_match_id=$2",
        )
        .bind(user_id)
        .bind(&match_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE trade_episode_matches SET status='rejected',updated_at=now()
             WHERE id=$1 AND user_id=$2",
        )
        .bind(&match_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
    }
    publish_unplanned_episode(pool, user_id, &episode, input).await
}

async fn publish_unplanned_episode(
    pool: &PgPool,
    user_id: &str,
    episode: &StoredEpisode,
    input: PublishEpisodeReviewInput,
) -> Result<String> {
    let closed_at = episode
        .draft
        .closed_at
        .expect("closed episode checked above");
    let entry_quantity = episode.draft.entry_quantity();
    let entry_price = weighted_price(episode.draft.entry_allocations(), entry_quantity)?;
    let exit_quantity: Decimal = episode
        .draft
        .exit_allocations()
        .map(|fill| fill.quantity)
        .sum();
    let exit_price = weighted_price(episode.draft.exit_allocations(), exit_quantity)?;
    let (symbol, multiplier) = match &episode.draft.instrument {
        ExecutionInstrument::Equity { symbol } => (symbol.clone(), Decimal::ONE),
        ExecutionInstrument::Option {
            underlying,
            multiplier,
            ..
        } => (underlying.clone(), *multiplier),
    };
    let transaction_ids: Vec<String> = episode
        .draft
        .allocations
        .iter()
        .map(|fill| fill.transaction_id.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let shared_fill_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (
           SELECT brokerage_transaction_id FROM trade_episode_fills
           WHERE brokerage_transaction_id=ANY($1)
           GROUP BY brokerage_transaction_id HAVING COUNT(DISTINCT episode_id) > 1
         ) shared",
    )
    .bind(&transaction_ids)
    .fetch_one(pool)
    .await?;
    ensure!(
        shared_fill_count == 0,
        "a reversal execution spans multiple positions; adjust the fills manually"
    );
    let symbol_name = sqlx::query_scalar::<_, Option<String>>(
        "SELECT symbol_description FROM brokerage_transactions
         WHERE id=ANY($1) AND user_id=$2
         ORDER BY trade_date,id LIMIT 1",
    )
    .bind(&transaction_ids)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .flatten()
    .filter(|name| !name.trim().is_empty())
    .unwrap_or_else(|| symbol.clone());
    let journal_id = Uuid::new_v4().to_string();
    let args = crate::service::db::schema::tables::journal_table::JournalWriteArgs {
        id: journal_id.clone(),
        workspace_id: episode.workspace_id.clone(),
        open_date: episode.draft.opened_at.to_rfc3339(),
        close_date: closed_at.to_rfc3339(),
        entry_price: entry_price
            .to_f64()
            .ok_or_else(|| anyhow!("entry price cannot be represented"))?,
        exit_price: exit_price
            .to_f64()
            .ok_or_else(|| anyhow!("exit price cannot be represented"))?,
        position_size: entry_quantity
            .to_f64()
            .ok_or_else(|| anyhow!("position size cannot be represented"))?,
        stop_loss: input.stop_loss,
        symbol,
        symbol_name,
        trade_type: direction_str(episode.draft.direction).to_string(),
        playbook_id: input.playbook_id,
        notes: compose_review_notes(
            input.plan_adherence.as_deref(),
            input.lesson.as_deref(),
            input.notes.as_deref(),
        ),
        broke_30min_rule: None,
        pre_trade_conviction: None,
        market_regime: None,
        is_planned_pre_market: None,
        revenge_trade: None,
        rule_adherence_score: None,
        tag_ids: input.tag_ids,
        violated_principle_ids: input.violated_principle_ids,
        contract_multiplier: multiplier.to_f64().unwrap_or(1.0),
    };
    let link_ids: Vec<String> = transaction_ids
        .iter()
        .map(|_| Uuid::new_v4().to_string())
        .collect();
    let mut tx = pool.begin().await?;
    sqlx::query(
        "SELECT id FROM brokerage_transactions
         WHERE id=ANY($1) AND user_id=$2 FOR UPDATE",
    )
    .bind(&transaction_ids)
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await?;
    let linked_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_brokerage_links
         WHERE brokerage_transaction_id=ANY($1)",
    )
    .bind(&transaction_ids)
    .fetch_one(&mut *tx)
    .await?;
    ensure!(
        linked_count == 0,
        "one or more fills are already in the journal"
    );
    crate::service::db::schema::tables::journal_table::create_journal_entry_tx(
        &mut tx,
        user_id,
        &args,
        &crate::service::hlc::stamp(),
    )
    .await?;
    sqlx::query(
        "INSERT INTO journal_brokerage_links
         (id,journal_entry_id,brokerage_transaction_id,user_id)
         SELECT link_id,$3,transaction_id,$4
         FROM unnest($1::text[],$2::text[]) links(link_id,transaction_id)",
    )
    .bind(&link_ids)
    .bind(&transaction_ids)
    .bind(&journal_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO brokerage_episode_publications
         (episode_id,user_id,workspace_id,journal_entry_id)
         VALUES ($1,$2,$3,$4)",
    )
    .bind(&input.episode_id)
    .bind(user_id)
    .bind(&args.workspace_id)
    .bind(&journal_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(journal_id)
}

async fn record_episode_publication(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    episode_id: &str,
    journal_id: &str,
    plan_id: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO brokerage_episode_publications
         (episode_id,user_id,workspace_id,journal_entry_id,plan_id)
         VALUES ($1,$2,$3,$4,$5) ON CONFLICT (episode_id) DO NOTHING",
    )
    .bind(episode_id)
    .bind(user_id)
    .bind(workspace_id)
    .bind(journal_id)
    .bind(plan_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn compose_review_notes(
    plan_adherence: Option<&str>,
    lesson: Option<&str>,
    notes: Option<&str>,
) -> Option<String> {
    let mut sections = Vec::new();
    if let Some(value) = plan_adherence.filter(|value| !value.trim().is_empty()) {
        sections.push(format!("Plan adherence: {}", value.trim()));
    }
    if let Some(value) = lesson.filter(|value| !value.trim().is_empty()) {
        sections.push(format!("Lesson: {}", value.trim()));
    }
    if let Some(value) = notes.filter(|value| !value.trim().is_empty()) {
        sections.push(value.trim().to_string());
    }
    (!sections.is_empty()).then(|| sections.join("\n\n"))
}

pub async fn list_workspace_episodes(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
) -> Result<Vec<StoredEpisode>> {
    load_episodes(pool, user_id, workspace_id).await
}

fn weighted_price<'a>(
    fills: impl Iterator<Item = &'a FillAllocation>,
    quantity: Decimal,
) -> Result<Decimal> {
    if quantity <= Decimal::ZERO {
        return Err(anyhow!("execution has no quantity"));
    }
    Ok(fills
        .map(|fill| fill.price * fill.quantity)
        .sum::<Decimal>()
        / quantity)
}

async fn refresh_suggestions(pool: &PgPool, user_id: &str, workspace_id: &str) -> Result<()> {
    sqlx::query(
        "DELETE FROM trade_episode_matches WHERE user_id=$1 AND workspace_id=$2 AND status='suggested'",
    )
    .bind(user_id)
    .bind(workspace_id)
    .execute(pool)
    .await?;
    let plans = load_plan_snapshots(pool, user_id, workspace_id).await?;
    let episodes = load_episodes(pool, user_id, workspace_id).await?;
    for stored in episodes {
        let suggestion = suggest_plan_match(&stored.draft, &plans);
        for candidate in suggestion.candidates {
            sqlx::query(
                "INSERT INTO trade_episode_matches
                 (id,user_id,workspace_id,episode_id,plan_id,status,score,evidence_json)
                 VALUES ($1,$2,$3,$4,$5,'suggested',$6,$7)
                 ON CONFLICT (episode_id,plan_id) DO UPDATE SET
                   score=EXCLUDED.score,evidence_json=EXCLUDED.evidence_json,updated_at=now()",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(user_id)
            .bind(workspace_id)
            .bind(&stored.id)
            .bind(&candidate.plan_id)
            .bind(candidate.score)
            .bind(serde_json::to_value(candidate.evidence)?)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_review_version(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    match_id: &str,
    episode_id: &str,
    plan_id: &str,
    stage: &str,
    reflection: Option<serde_json::Value>,
    journal_draft: Option<serde_json::Value>,
) -> Result<String> {
    let plan = load_plan_snapshot(pool, user_id, plan_id).await?;
    let episode = load_episode(pool, user_id, episode_id).await?;
    let entries: Vec<_> = episode.draft.entry_allocations().cloned().collect();
    let reconciliation = reconcile_tranches(&plan.tranches, &entries);
    let calculation = calculate_review(&plan, &episode.draft, &reconciliation)
        .ok_or_else(|| anyhow!("cannot calculate a review without planned and actual entries"))?;
    let row = sqlx::query(
        "SELECT COALESCE(MAX(version_number),0)+1,
                (array_agg(id ORDER BY version_number DESC))[1]
         FROM trade_review_versions WHERE match_id=$1",
    )
    .bind(match_id)
    .fetch_one(pool)
    .await?;
    let version: i32 = row.try_get(0)?;
    let supersedes_id: Option<String> = row.try_get(1)?;
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO trade_review_versions
         (id,user_id,workspace_id,match_id,version_number,stage,plan_snapshot_json,calculation_json,reflection_json,journal_draft_json,finalized_at,supersedes_id)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,CASE WHEN $6='final' THEN now() ELSE NULL END,$11)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(workspace_id)
    .bind(match_id)
    .bind(version)
    .bind(stage)
    .bind(serde_json::to_value(plan)?)
    .bind(serde_json::to_value(calculation)?)
    .bind(reflection)
    .bind(journal_draft)
    .bind(supersedes_id)
    .execute(pool)
    .await?;
    Ok(id)
}

async fn load_plan_snapshots(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
) -> Result<Vec<PlanSnapshot>> {
    let plans = position_calculator_plans_table::list_plans(pool, user_id, workspace_id).await?;
    plans.into_iter().map(plan_to_snapshot).collect()
}

async fn load_plan_snapshot(pool: &PgPool, user_id: &str, plan_id: &str) -> Result<PlanSnapshot> {
    let plan = position_calculator_plans_table::find_plan(pool, plan_id, user_id)
        .await?
        .ok_or_else(|| anyhow!("plan not found"))?;
    plan_to_snapshot(plan)
}

fn plan_to_snapshot(
    plan: position_calculator_plans_table::PositionCalculatorPlan,
) -> Result<PlanSnapshot> {
    let direction = if plan.position_type.eq_ignore_ascii_case("short") {
        EpisodeDirection::Short
    } else {
        EpisodeDirection::Long
    };
    let instrument = plan
        .instrument_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .context("invalid stored plan instrument")?
        .unwrap_or(ExecutionInstrument::Equity {
            symbol: plan.symbol,
        });
    Ok(PlanSnapshot {
        plan_id: plan.id,
        workspace_id: plan.workspace_id,
        instrument,
        direction,
        stop_loss: Decimal::from_f64(plan.stop_loss).ok_or_else(|| anyhow!("invalid stop loss"))?,
        created_at: parse_datetime(&plan.created_at)
            .ok_or_else(|| anyhow!("invalid plan creation time"))?,
        active_at_episode_open: !matches!(plan.status.as_str(), "cancelled" | "deleted"),
        tranches: plan
            .tranches
            .into_iter()
            .enumerate()
            .map(|(order, tranche)| {
                Ok(PlanTranche {
                    id: tranche.id,
                    order,
                    quantity: Decimal::from_f64(tranche.shares)
                        .ok_or_else(|| anyhow!("invalid tranche quantity"))?,
                    entry_price: Decimal::from_f64(tranche.target_price)
                        .ok_or_else(|| anyhow!("invalid tranche price"))?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
    })
}

async fn load_episodes(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
) -> Result<Vec<StoredEpisode>> {
    let rows = sqlx::query("SELECT id FROM trade_episodes WHERE user_id=$1 AND workspace_id=$2")
        .bind(user_id)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;
    let mut episodes = Vec::new();
    for row in rows {
        episodes.push(load_episode(pool, user_id, row.try_get::<String, _>(0)?.as_str()).await?);
    }
    Ok(episodes)
}

async fn load_episode(pool: &PgPool, user_id: &str, episode_id: &str) -> Result<StoredEpisode> {
    let row = sqlx::query("SELECT id,workspace_id,grouping_source,instrument_json,direction,opened_at,closed_at,current_quantity,fingerprint FROM trade_episodes WHERE id=$1 AND user_id=$2")
        .bind(episode_id).bind(user_id).fetch_optional(pool).await?.ok_or_else(|| anyhow!("episode not found"))?;
    let instrument: ExecutionInstrument = serde_json::from_value(row.try_get(3)?)?;
    let fill_rows = sqlx::query("SELECT brokerage_transaction_id,role,quantity,price,fee,executed_at FROM trade_episode_fills WHERE episode_id=$1 ORDER BY executed_at,brokerage_transaction_id")
        .bind(episode_id).fetch_all(pool).await?;
    let allocations = fill_rows
        .into_iter()
        .map(|fill| {
            Ok(FillAllocation {
                transaction_id: fill.try_get(0)?,
                role: if fill.try_get::<String, _>(1)? == "entry" {
                    FillRole::Entry
                } else {
                    FillRole::Exit
                },
                quantity: fill.try_get(2)?,
                price: fill.try_get(3)?,
                fee: fill.try_get(4)?,
                executed_at: fill.try_get(5)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(StoredEpisode {
        id: row.try_get(0)?,
        workspace_id: row.try_get(1)?,
        grouping_source: row.try_get(2)?,
        draft: TradeEpisodeDraft {
            instrument,
            direction: if row.try_get::<String, _>(4)? == "short" {
                EpisodeDirection::Short
            } else {
                EpisodeDirection::Long
            },
            allocations,
            opened_at: row.try_get(5)?,
            closed_at: row.try_get(6)?,
            current_quantity: row.try_get(7)?,
            fingerprint: row.try_get(8)?,
        },
    })
}

fn parse_datetime(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.with_timezone(&Utc))
}
fn direction_str(direction: EpisodeDirection) -> &'static str {
    if direction == EpisodeDirection::Long {
        "long"
    } else {
        "short"
    }
}
fn role_str(role: FillRole) -> &'static str {
    if role == FillRole::Entry {
        "entry"
    } else {
        "exit"
    }
}
