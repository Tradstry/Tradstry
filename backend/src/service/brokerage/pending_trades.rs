//! Brokerage review inbox derived from the same deterministic trade episodes
//! used by plan-vs-actual reviews.

use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{Result, anyhow};
use async_graphql::SimpleObject;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::service::db::schema::tables::{brokerage_table, journal_table, trade_review_table};
use crate::service::trade_review::types::{EpisodeDirection, ExecutionInstrument, FillAllocation};

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PendingTrade {
    pub id: String,
    pub episode_id: String,
    pub symbol: String,
    pub direction: String,
    pub status: String,
    pub open_date: String,
    pub close_date: Option<String>,
    pub entry_units: f64,
    pub avg_entry_price: f64,
    pub avg_exit_price: Option<f64>,
    pub realized_pnl: Option<f64>,
    pub transaction_ids: Vec<String>,
    pub fill_count: i32,
    pub is_fully_linked: bool,
    pub is_partially_linked: bool,
    pub multiplier: f64,
    pub is_option: bool,
    pub underlying: Option<String>,
    pub option_kind: Option<String>,
    pub strike: Option<f64>,
    pub expiration: Option<String>,
    pub symbol_name: Option<String>,
    pub requires_manual_grouping: bool,
    pub block_reason: Option<String>,
}

fn weighted_price<'a>(fills: impl Iterator<Item = &'a FillAllocation>) -> Option<Decimal> {
    let fills: Vec<_> = fills.collect();
    let quantity: Decimal = fills.iter().map(|fill| fill.quantity).sum();
    (quantity > Decimal::ZERO).then(|| {
        fills
            .iter()
            .map(|fill| fill.price * fill.quantity)
            .sum::<Decimal>()
            / quantity
    })
}

fn realized_pnl(
    direction: EpisodeDirection,
    allocations: &[FillAllocation],
    multiplier: Decimal,
) -> Option<Decimal> {
    let entry = weighted_price(
        allocations
            .iter()
            .filter(|fill| fill.role == crate::service::trade_review::types::FillRole::Entry),
    )?;
    let exits: Vec<_> = allocations
        .iter()
        .filter(|fill| fill.role == crate::service::trade_review::types::FillRole::Exit)
        .collect();
    let exit_quantity: Decimal = exits.iter().map(|fill| fill.quantity).sum();
    if exit_quantity <= Decimal::ZERO {
        return None;
    }
    let exit_value: Decimal = exits.iter().map(|fill| fill.price * fill.quantity).sum();
    let gross = if direction == EpisodeDirection::Long {
        exit_value - entry * exit_quantity
    } else {
        entry * exit_quantity - exit_value
    };
    let fees: Decimal = allocations.iter().map(|fill| fill.fee).sum();
    Some(gross * multiplier - fees)
}

pub async fn compute_pending_trades(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
) -> Result<Vec<PendingTrade>> {
    trade_review_table::rebuild_workspace(pool, user_id, workspace_id).await?;
    let episodes = trade_review_table::list_workspace_episodes(pool, user_id, workspace_id).await?;
    let brokerage_transactions =
        brokerage_table::list_all_for_lifecycle(pool, user_id, workspace_id).await?;
    let transactions_by_id: HashMap<_, _> = brokerage_transactions
        .iter()
        .map(|transaction| (transaction.id.as_str(), transaction))
        .collect();
    let linked_ids: HashSet<String> =
        journal_table::list_linked_brokerage_transaction_ids(pool, user_id, workspace_id)
            .await?
            .into_iter()
            .collect();

    let episode_transactions: Vec<BTreeSet<String>> = episodes
        .iter()
        .map(|episode| {
            episode
                .draft
                .allocations
                .iter()
                .map(|fill| fill.transaction_id.clone())
                .collect()
        })
        .collect();
    let mut transaction_episode_counts = HashMap::<String, usize>::new();
    for transaction_ids in &episode_transactions {
        for transaction_id in transaction_ids {
            *transaction_episode_counts
                .entry(transaction_id.clone())
                .or_default() += 1;
        }
    }

    let mut result = Vec::new();
    for (episode, transaction_ids) in episodes.into_iter().zip(episode_transactions) {
        if transaction_ids.is_empty() {
            continue;
        }
        let linked_count = transaction_ids
            .iter()
            .filter(|id| linked_ids.contains(*id))
            .count();
        if linked_count == transaction_ids.len() {
            continue;
        }
        let first_transaction = episode.draft.allocations.first().and_then(|fill| {
            transactions_by_id
                .get(fill.transaction_id.as_str())
                .copied()
        });
        let entry_allocations: Vec<_> = episode.draft.entry_allocations().collect();
        let entry_quantity: Decimal = entry_allocations.iter().map(|fill| fill.quantity).sum();
        let avg_entry = weighted_price(entry_allocations.into_iter())
            .ok_or_else(|| anyhow!("episode has no entry quantity"))?;
        let avg_exit = weighted_price(episode.draft.exit_allocations());
        let (symbol, multiplier, is_option, underlying, option_kind, strike, expiration) =
            match &episode.draft.instrument {
                ExecutionInstrument::Equity { symbol } => {
                    (symbol.clone(), Decimal::ONE, false, None, None, None, None)
                }
                ExecutionInstrument::Option {
                    underlying,
                    expiration,
                    strike,
                    option_kind,
                    multiplier,
                } => (
                    underlying.clone(),
                    *multiplier,
                    true,
                    Some(underlying.clone()),
                    Some(option_kind.to_ascii_uppercase()),
                    strike.to_f64(),
                    Some(expiration.to_string()),
                ),
            };
        let requires_manual_grouping = transaction_ids.iter().any(|id| {
            transaction_episode_counts
                .get(id)
                .copied()
                .unwrap_or_default()
                > 1
        });
        let status = if episode.draft.closed_at.is_some() {
            "closed"
        } else {
            "open"
        };
        let pnl = episode.draft.closed_at.and_then(|_| {
            realized_pnl(
                episode.draft.direction,
                &episode.draft.allocations,
                multiplier,
            )
        });

        result.push(PendingTrade {
            id: episode.id.clone(),
            episode_id: episode.id,
            symbol,
            direction: if episode.draft.direction == EpisodeDirection::Long {
                "long".to_string()
            } else {
                "short".to_string()
            },
            status: status.to_string(),
            open_date: episode.draft.opened_at.to_rfc3339(),
            close_date: episode.draft.closed_at.map(|date| date.to_rfc3339()),
            entry_units: entry_quantity.to_f64().unwrap_or_default(),
            avg_entry_price: avg_entry.to_f64().unwrap_or_default(),
            avg_exit_price: avg_exit.and_then(|price| price.to_f64()),
            realized_pnl: pnl.and_then(|value| value.to_f64()),
            transaction_ids: transaction_ids.into_iter().collect(),
            fill_count: episode.draft.allocations.len() as i32,
            is_fully_linked: false,
            is_partially_linked: linked_count > 0,
            multiplier: multiplier.to_f64().unwrap_or(1.0),
            is_option,
            underlying,
            option_kind,
            strike,
            expiration,
            symbol_name: first_transaction.and_then(|transaction| {
                transaction
                    .symbol_description
                    .clone()
                    .filter(|description| !description.trim().is_empty())
            }),
            requires_manual_grouping,
            block_reason: requires_manual_grouping.then(|| {
                "A reversal fill spans two positions. Select the correct fills manually."
                    .to_string()
            }),
        });
    }

    result.sort_by(
        |left, right| match (left.status.as_str(), right.status.as_str()) {
            ("open", "closed") => std::cmp::Ordering::Less,
            ("closed", "open") => std::cmp::Ordering::Greater,
            _ => right
                .close_date
                .as_deref()
                .unwrap_or_default()
                .cmp(left.close_date.as_deref().unwrap_or_default())
                .then_with(|| right.open_date.cmp(&left.open_date)),
        },
    );
    Ok(result)
}
