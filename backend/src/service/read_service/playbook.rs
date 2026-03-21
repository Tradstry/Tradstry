use anyhow::Result;
use async_graphql::SimpleObject;
use std::collections::HashMap;

use crate::service::turso::client::UserDb;
use crate::service::turso::schema::tables::{
    journal_table::{self, JournalEntry},
    playbook_table::{self, CreatePlaybookInput, Playbook, UpdatePlaybookInput},
};

#[derive(Debug, Clone)]
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

fn dollar_pl(entry: &JournalEntry) -> f64 {
    (entry.total_pl * entry.position_size) / 100.0
}

fn calculate_playbook_stats(trades: &[JournalEntry]) -> PlaybookStats {
    if trades.is_empty() {
        return PlaybookStats {
            win_rate: 0.0,
            cumulative_profit: 0.0,
            average_gain: 0.0,
            average_loss: 0.0,
            trade_count: 0,
        };
    }

    let total_trades = trades.len();
    let cumulative_profit = trades.iter().map(dollar_pl).sum::<f64>();

    let winning = trades
        .iter()
        .filter(|entry| dollar_pl(entry) > 0.0)
        .collect::<Vec<_>>();

    let losing = trades
        .iter()
        .filter(|entry| dollar_pl(entry) < 0.0)
        .collect::<Vec<_>>();

    let average_gain = if winning.is_empty() {
        0.0
    } else {
        winning.iter().map(|entry| dollar_pl(entry)).sum::<f64>() / winning.len() as f64
    };

    let average_loss = if losing.is_empty() {
        0.0
    } else {
        losing
            .iter()
            .map(|entry| dollar_pl(entry).abs())
            .sum::<f64>()
            / losing.len() as f64
    };

    PlaybookStats {
        win_rate: (winning.len() as f64 / total_trades as f64) * 100.0,
        cumulative_profit,
        average_gain,
        average_loss,
        trade_count: total_trades,
    }
}

fn build_playbook_with_stats(
    record: Playbook,
    entries_by_playbook: &HashMap<String, Vec<JournalEntry>>,
) -> PlaybookWithStats {
    let trades = entries_by_playbook
        .get(&record.id)
        .cloned()
        .unwrap_or_default();
    let stats = calculate_playbook_stats(&trades);

    PlaybookWithStats::from_record(record, stats)
}

fn group_entries_by_playbook(entries: Vec<JournalEntry>) -> HashMap<String, Vec<JournalEntry>> {
    let mut by_playbook: HashMap<String, Vec<JournalEntry>> = HashMap::new();

    for mut entry in entries {
        if let Some(playbook_id) = entry.playbook_id.take() {
            by_playbook.entry(playbook_id).or_default().push(entry);
        }
    }

    by_playbook
}

pub async fn list_playbooks(user_db: &UserDb) -> Result<Vec<PlaybookWithStats>> {
    let user_id = user_db.user_id().to_string();
    let playbooks = playbook_table::list_playbooks(user_db.conn(), &user_id).await?;
    let entries = journal_table::list_journal_entries(user_db.conn(), user_db.user_id()).await?;
    let by_playbook = group_entries_by_playbook(entries);

    Ok(playbooks
        .into_iter()
        .map(|playbook| build_playbook_with_stats(playbook, &by_playbook))
        .collect())
}

pub async fn get_playbook(user_db: &UserDb, id: &str) -> Result<Option<PlaybookWithStats>> {
    let playbook = playbook_table::find_playbook(user_db.conn(), id, user_db.user_id()).await?;
    if playbook.is_none() {
        return Ok(None);
    }

    let entries = journal_table::list_journal_entries(user_db.conn(), user_db.user_id()).await?;
    let by_playbook = group_entries_by_playbook(entries);

    Ok(playbook.map(|record| build_playbook_with_stats(record, &by_playbook)))
}

pub async fn create_playbook(
    user_db: &UserDb,
    input: CreatePlaybookInput,
) -> Result<PlaybookWithStats> {
    let playbook =
        playbook_table::create_playbook(user_db.conn(), user_db.user_id(), input).await?;
    let entries = journal_table::list_journal_entries(user_db.conn(), user_db.user_id()).await?;
    let by_playbook = group_entries_by_playbook(entries);

    Ok(build_playbook_with_stats(playbook, &by_playbook))
}

pub async fn update_playbook(
    user_db: &UserDb,
    id: &str,
    input: UpdatePlaybookInput,
) -> Result<PlaybookWithStats> {
    let playbook =
        playbook_table::update_playbook(user_db.conn(), id, user_db.user_id(), input).await?;
    let entries = journal_table::list_journal_entries(user_db.conn(), user_db.user_id()).await?;
    let by_playbook = group_entries_by_playbook(entries);

    Ok(build_playbook_with_stats(playbook, &by_playbook))
}

pub async fn delete_playbook(user_db: &UserDb, id: &str) -> Result<bool> {
    playbook_table::delete_playbook(user_db.conn(), id, user_db.user_id()).await
}

#[cfg(test)]
mod tests {
    use super::calculate_playbook_stats;
    use crate::service::turso::schema::tables::journal_table::JournalEntry;

    fn entry(total_pl: f64, position_size: f64) -> JournalEntry {
        JournalEntry {
            id: String::new(),
            user_id: String::new(),
            account_id: String::new(),
            reviewed: false,
            open_date: String::new(),
            close_date: String::new(),
            entry_price: 0.0,
            exit_price: 0.0,
            position_size,
            symbol: String::new(),
            symbol_name: String::new(),
            status: String::new(),
            total_pl,
            net_roi: 0.0,
            duration: 0,
            stop_loss: 0.0,
            risk_reward: 0.0,
            trade_type: String::new(),
            mistakes: String::new(),
            entry_tactics: String::new(),
            edges_spotted: String::new(),
            playbook_id: Some(String::from("playbook-id")),
            notes: None,
        }
    }

    #[test]
    fn computes_zero_metrics_for_empty_slice() {
        let stats = calculate_playbook_stats(&[]);
        assert_eq!(stats.trade_count, 0);
        assert_eq!(stats.win_rate, 0.0);
        assert_eq!(stats.cumulative_profit, 0.0);
        assert_eq!(stats.average_gain, 0.0);
        assert_eq!(stats.average_loss, 0.0);
    }

    #[test]
    fn computes_wins_and_losses() {
        let entries = vec![entry(8.0, 100.0), entry(-4.0, 200.0), entry(1.0, 50.0)];
        let stats = calculate_playbook_stats(&entries);
        assert_eq!(stats.trade_count, 3);
        assert_eq!(stats.win_rate, (2.0 / 3.0) * 100.0);
        assert_eq!(stats.cumulative_profit, 8.0 + -8.0 + 0.5);
        assert_eq!(stats.average_gain, (8.0 + 0.5) / 2.0);
        assert_eq!(stats.average_loss, 8.0);
    }
}
