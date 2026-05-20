use anyhow::Result;
use async_graphql::SimpleObject;
use std::collections::HashMap;

use crate::service::turso::client::UserDb;
use crate::service::turso::schema::tables::{
    journal_table::{self, PlaybookStatsRow},
    playbook_table::{self, CreatePlaybookInput, Playbook, UpdatePlaybookInput},
};

#[derive(Debug, Clone, Default)]
pub struct PlaybookStats {
    pub win_rate: f64,
    pub cumulative_profit: f64,
    pub average_gain: f64,
    pub average_loss: f64,
    pub trade_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PlaybookWithStats {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub edge_name: String,
    pub entry_rules: String,
    pub exit_rules: String,
    pub position_sizing_rules: String,
    pub additional_rules: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub win_rate: f64,
    pub cumulative_profit: f64,
    pub average_gain: f64,
    pub average_loss: f64,
    pub trade_count: usize,
}

impl PlaybookWithStats {
    fn from_record(record: Playbook, stats: PlaybookStats) -> Self {
        Self {
            id: record.id,
            user_id: record.user_id,
            name: record.name,
            edge_name: record.edge_name,
            entry_rules: record.entry_rules,
            exit_rules: record.exit_rules,
            position_sizing_rules: record.position_sizing_rules,
            additional_rules: record.additional_rules,
            created_at: record.created_at,
            updated_at: record.updated_at,
            win_rate: stats.win_rate,
            cumulative_profit: stats.cumulative_profit,
            average_gain: stats.average_gain,
            average_loss: stats.average_loss,
            trade_count: stats.trade_count,
        }
    }
}

fn stats_from_row(row: PlaybookStatsRow) -> PlaybookStats {
    let total_trades = row.total_trades as f64;
    let win_rate = (row.winning_trades as f64 / total_trades) * 100.0;
    let average_gain = if row.winning_trades == 0 {
        0.0
    } else {
        row.gross_profit / row.winning_trades as f64
    };
    let average_loss = if row.losing_trades == 0 {
        0.0
    } else {
        row.gross_loss / row.losing_trades as f64
    };

    PlaybookStats {
        win_rate,
        cumulative_profit: row.cumulative_profit,
        average_gain,
        average_loss,
        trade_count: row.total_trades as usize,
    }
}

async fn fetch_stats_map(user_db: &UserDb) -> Result<HashMap<String, PlaybookStats>> {
    let rows =
        journal_table::aggregate_stats_per_playbook(user_db.conn(), user_db.user_id()).await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.playbook_id.clone(), stats_from_row(row)))
        .collect())
}

fn build_with_stats(
    record: Playbook,
    stats_map: &HashMap<String, PlaybookStats>,
) -> PlaybookWithStats {
    let stats = stats_map.get(&record.id).cloned().unwrap_or_default();
    PlaybookWithStats::from_record(record, stats)
}

pub async fn list_playbooks(user_db: &UserDb) -> Result<Vec<PlaybookWithStats>> {
    let playbooks = playbook_table::list_playbooks(user_db.conn(), user_db.user_id()).await?;
    let stats_map = fetch_stats_map(user_db).await?;

    Ok(playbooks
        .into_iter()
        .map(|playbook| build_with_stats(playbook, &stats_map))
        .collect())
}

pub async fn get_playbook(user_db: &UserDb, id: &str) -> Result<Option<PlaybookWithStats>> {
    let playbook = playbook_table::find_playbook(user_db.conn(), id, user_db.user_id()).await?;
    let Some(playbook) = playbook else {
        return Ok(None);
    };
    let stats_map = fetch_stats_map(user_db).await?;
    Ok(Some(build_with_stats(playbook, &stats_map)))
}

pub async fn create_playbook(
    user_db: &UserDb,
    input: CreatePlaybookInput,
) -> Result<PlaybookWithStats> {
    let playbook =
        playbook_table::create_playbook(user_db.conn(), user_db.user_id(), input).await?;
    let stats_map = fetch_stats_map(user_db).await?;
    Ok(build_with_stats(playbook, &stats_map))
}

pub async fn update_playbook(
    user_db: &UserDb,
    id: &str,
    input: UpdatePlaybookInput,
) -> Result<PlaybookWithStats> {
    let playbook =
        playbook_table::update_playbook(user_db.conn(), id, user_db.user_id(), input).await?;
    let stats_map = fetch_stats_map(user_db).await?;
    Ok(build_with_stats(playbook, &stats_map))
}

pub async fn delete_playbook(user_db: &UserDb, id: &str) -> Result<bool> {
    playbook_table::delete_playbook(user_db.conn(), id, user_db.user_id()).await
}
