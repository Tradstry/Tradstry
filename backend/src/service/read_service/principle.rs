use anyhow::{Context, Result};
use async_graphql::SimpleObject;
use std::collections::HashMap;

use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::{
    journal_table::{self, PrincipleStatsRow},
    trading_principle_table::{self, CreatePrincipleInput, TradingPrinciple, UpdatePrincipleInput},
};

#[derive(Debug, Clone, Default)]
pub struct ViolationStats {
    pub violation_count: usize,
    pub violated_cumulative_profit: f64,
    pub violated_cumulative_roi: f64,
    pub violated_win_rate: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PrincipleWithStats {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub playbook_id: Option<String>,
    pub evidence_note_id: Option<String>,
    pub evidence_note_title: Option<String>,
    pub title: String,
    pub the_rule: String,
    pub why: String,
    pub intervention: Option<String>,
    pub priority: i64,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
    pub violation_count: usize,
    pub violated_cumulative_profit: f64,
    pub violated_cumulative_roi: f64,
    pub violated_win_rate: f64,
}

impl PrincipleWithStats {
    fn from_record(
        record: TradingPrinciple,
        stats: ViolationStats,
        evidence_note_title: Option<String>,
    ) -> Self {
        Self {
            id: record.id,
            user_id: record.user_id,
            account_id: record.account_id,
            playbook_id: record.playbook_id,
            evidence_note_id: record.evidence_note_id,
            evidence_note_title,
            title: record.title,
            the_rule: record.the_rule,
            why: record.why,
            intervention: record.intervention,
            priority: record.priority,
            is_active: record.is_active,
            created_at: record.created_at,
            updated_at: record.updated_at,
            violation_count: stats.violation_count,
            violated_cumulative_profit: stats.violated_cumulative_profit,
            violated_cumulative_roi: stats.violated_cumulative_roi,
            violated_win_rate: stats.violated_win_rate,
        }
    }
}

/// Breakeven trades are excluded from both sides of the win rate, matching
/// `read_service::playbook::stats_from_row`.
fn stats_from_row(row: PrincipleStatsRow) -> ViolationStats {
    let decisive = (row.winning_trades + row.losing_trades) as f64;
    let violated_win_rate = if decisive > 0.0 {
        (row.winning_trades as f64 / decisive) * 100.0
    } else {
        0.0
    };

    ViolationStats {
        violation_count: row.total_trades as usize,
        violated_cumulative_profit: row.cumulative_profit,
        violated_cumulative_roi: row.cumulative_roi,
        violated_win_rate,
    }
}

async fn fetch_stats_map(
    user_db: &UserDb,
    account_id: &str,
) -> Result<HashMap<String, ViolationStats>> {
    let rows = journal_table::aggregate_violation_stats_per_principle(
        user_db.pool(),
        user_db.user_id(),
        account_id,
    )
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.principle_id.clone(), stats_from_row(row)))
        .collect())
}

/// Titles for the evidence notes referenced by the given principles, in one query.
async fn fetch_note_titles(
    user_db: &UserDb,
    principles: &[TradingPrinciple],
) -> Result<HashMap<String, String>> {
    let note_ids: Vec<String> = principles
        .iter()
        .filter_map(|p| p.evidence_note_id.clone())
        .collect();

    if note_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT id, title FROM notebook_notes WHERE id = ANY($1) AND user_id = $2")
            .bind(&note_ids)
            .bind(user_db.user_id())
            .fetch_all(user_db.pool())
            .await
            .context("Failed to load evidence note titles")?;

    Ok(rows.into_iter().collect())
}

fn build_with_stats(
    record: TradingPrinciple,
    stats_map: &HashMap<String, ViolationStats>,
    note_titles: &HashMap<String, String>,
) -> PrincipleWithStats {
    let stats = stats_map.get(&record.id).cloned().unwrap_or_default();
    let note_title = record
        .evidence_note_id
        .as_ref()
        .and_then(|id| note_titles.get(id).cloned());
    PrincipleWithStats::from_record(record, stats, note_title)
}

pub async fn list_principles(
    user_db: &UserDb,
    account_id: &str,
) -> Result<Vec<PrincipleWithStats>> {
    let principles =
        trading_principle_table::list_principles(user_db.pool(), user_db.user_id(), account_id)
            .await?;
    let stats_map = fetch_stats_map(user_db, account_id).await?;
    let note_titles = fetch_note_titles(user_db, &principles).await?;

    Ok(principles
        .into_iter()
        .map(|p| build_with_stats(p, &stats_map, &note_titles))
        .collect())
}

pub async fn get_principle(user_db: &UserDb, id: &str) -> Result<Option<PrincipleWithStats>> {
    let Some(principle) =
        trading_principle_table::find_principle(user_db.pool(), id, user_db.user_id()).await?
    else {
        return Ok(None);
    };
    let stats_map = fetch_stats_map(user_db, &principle.account_id).await?;
    let note_titles = fetch_note_titles(user_db, std::slice::from_ref(&principle)).await?;
    Ok(Some(build_with_stats(principle, &stats_map, &note_titles)))
}

pub async fn create_principle(
    user_db: &UserDb,
    input: CreatePrincipleInput,
) -> Result<PrincipleWithStats> {
    let principle =
        trading_principle_table::create_principle(user_db.pool(), user_db.user_id(), input).await?;
    let stats_map = fetch_stats_map(user_db, &principle.account_id).await?;
    let note_titles = fetch_note_titles(user_db, std::slice::from_ref(&principle)).await?;
    Ok(build_with_stats(principle, &stats_map, &note_titles))
}

pub async fn update_principle(
    user_db: &UserDb,
    id: &str,
    input: UpdatePrincipleInput,
) -> Result<PrincipleWithStats> {
    let principle =
        trading_principle_table::update_principle(user_db.pool(), id, user_db.user_id(), input)
            .await?;
    let stats_map = fetch_stats_map(user_db, &principle.account_id).await?;
    let note_titles = fetch_note_titles(user_db, std::slice::from_ref(&principle)).await?;
    Ok(build_with_stats(principle, &stats_map, &note_titles))
}

pub async fn delete_principle(user_db: &UserDb, id: &str) -> Result<bool> {
    trading_principle_table::delete_principle(user_db.pool(), id, user_db.user_id()).await
}

pub async fn reorder_principles(user_db: &UserDb, ordered_ids: &[String]) -> Result<bool> {
    trading_principle_table::reorder_principles(user_db.pool(), user_db.user_id(), ordered_ids)
        .await?;
    Ok(true)
}

pub async fn set_trade_principle_violations(
    user_db: &UserDb,
    journal_entry_id: &str,
    principle_ids: &[String],
) -> Result<()> {
    trading_principle_table::set_trade_principle_violations(
        user_db.pool(),
        user_db.user_id(),
        journal_entry_id,
        principle_ids,
    )
    .await
}

pub async fn principles_for_trade(user_db: &UserDb, journal_entry_id: &str) -> Result<Vec<String>> {
    trading_principle_table::principles_for_trade(
        user_db.pool(),
        user_db.user_id(),
        journal_entry_id,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(total: i64, wins: i64, losses: i64) -> PrincipleStatsRow {
        PrincipleStatsRow {
            principle_id: "p-1".to_string(),
            total_trades: total,
            winning_trades: wins,
            losing_trades: losses,
            cumulative_profit: -50.0,
            cumulative_roi: -5.0,
        }
    }

    #[test]
    fn win_rate_excludes_breakeven_trades() {
        let stats = stats_from_row(row(10, 6, 3));
        assert!((stats.violated_win_rate - (6.0 / 9.0 * 100.0)).abs() < 1e-9);
        assert_eq!(stats.violation_count, 10);
    }

    #[test]
    fn stats_from_row_maps_dollars_and_percent_distinctly() {
        let stats = stats_from_row(row(10, 6, 3));
        assert_eq!(
            stats.violated_cumulative_profit, -50.0,
            "dollars come from cumulative_profit"
        );
        assert_eq!(
            stats.violated_cumulative_roi, -5.0,
            "percent comes from cumulative_roi"
        );
    }

    #[test]
    fn never_violated_principle_is_all_zeros() {
        let stats = ViolationStats::default();
        assert_eq!(stats.violation_count, 0);
        assert_eq!(stats.violated_win_rate, 0.0);
        assert_eq!(stats.violated_cumulative_profit, 0.0);
        assert_eq!(stats.violated_cumulative_roi, 0.0);
    }

    #[test]
    fn all_breakeven_gives_zero_win_rate() {
        let stats = stats_from_row(row(4, 0, 0));
        assert_eq!(stats.violated_win_rate, 0.0);
        assert_eq!(stats.violation_count, 4);
    }
}
