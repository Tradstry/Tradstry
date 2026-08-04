//! Output shapes for the MCP tools.
//!
//! The tools used to serialize database rows straight to the model. That leaked internal
//! identifiers, spent tokens on columns that are null for every row, and — worst —
//! published `total_pl` under a name that reads like dollars when it is a percent. A model
//! summing it reports losses that are simply wrong. These structs are the contract instead:
//! every money field says which unit it is in, and nothing internal crosses the boundary.

use serde::Serialize;
use tradstry_backend::service::db::schema::tables::accounts_table::Account;
use tradstry_backend::service::db::schema::tables::journal_table::JournalEntry;
use tradstry_backend::service::db::schema::tables::tags_table::TradeTag;

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// `journal_entries.total_pl` is a percent of entry price, so money is only recoverable
/// with the position: `position_size * entry_price * total_pl / 100`. Mirrors the
/// backend's `DOLLAR_PL_EXPR`, which is what every dollar figure elsewhere is built from.
pub fn pl_dollars(entry: &JournalEntry) -> f64 {
    entry.position_size * entry.entry_price * entry.total_pl / 100.0
}

#[derive(Debug, Serialize)]
pub struct McpTrade {
    pub id: String,
    pub account_id: String,
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_name: Option<String>,
    pub status: String,
    pub trade_type: String,
    pub open_date: String,
    pub close_date: String,
    pub duration_seconds: i64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub position_size: f64,
    /// Percent change from entry to exit. NOT money — see `pl_dollars`.
    pub pl_percent: f64,
    /// Realized P&L in the account's currency.
    pub pl_dollars: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_reward: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playbook_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mistakes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_tactics: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edges_spotted: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_regime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_trade_conviction: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_adherence_score: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revenge_trade: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broke_30min_rule: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_planned_pre_market: Option<bool>,
    /// The tags on this trade. A tag whose `role` is `mistake` is what marks the trade
    /// flawed, which is what the clean-vs-flawed and mistake-cost analytics are built from.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<McpTradeTag>,
    /// Principles this trade was recorded as violating.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub violated_principle_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct McpTradeTag {
    pub id: String,
    pub name: String,
    pub category: String,
    /// `mistake` | `tactic` | `edge`, or absent for a user-defined category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

impl From<&TradeTag> for McpTradeTag {
    fn from(t: &TradeTag) -> Self {
        Self {
            id: t.tag.id.clone(),
            name: t.tag.name.clone(),
            category: t.category_name.clone(),
            role: t.role.as_ref().map(|r| r.as_str().to_string()),
        }
    }
}

impl From<&JournalEntry> for McpTrade {
    fn from(e: &JournalEntry) -> Self {
        Self {
            id: e.id.clone(),
            account_id: e.account_id.clone(),
            symbol: e.symbol.clone(),
            symbol_name: non_empty(&e.symbol_name),
            status: e.status.clone(),
            trade_type: e.trade_type.clone(),
            open_date: e.open_date.clone(),
            close_date: e.close_date.clone(),
            duration_seconds: e.duration,
            entry_price: e.entry_price,
            exit_price: e.exit_price,
            position_size: e.position_size,
            pl_percent: e.total_pl,
            pl_dollars: pl_dollars(e),
            stop_loss: e.stop_loss,
            risk_reward: e.risk_reward,
            playbook_id: e.playbook_id.clone(),
            notes: e.notes.as_deref().and_then(non_empty),
            mistakes: non_empty(&e.mistakes),
            entry_tactics: non_empty(&e.entry_tactics),
            edges_spotted: non_empty(&e.edges_spotted),
            market_regime: e.market_regime.as_deref().and_then(non_empty),
            pre_trade_conviction: e.pre_trade_conviction,
            rule_adherence_score: e.rule_adherence_score,
            revenge_trade: e.revenge_trade,
            broke_30min_rule: e.broke_30min_rule,
            is_planned_pre_market: e.is_planned_pre_market,
            // Attached by the caller, which batches them across the page.
            tags: Vec::new(),
            violated_principle_ids: Vec::new(),
        }
    }
}

pub const TRADE_FIELDS: &[&str] = &[
    "id",
    "account_id",
    "symbol",
    "symbol_name",
    "status",
    "trade_type",
    "open_date",
    "close_date",
    "duration_seconds",
    "entry_price",
    "exit_price",
    "position_size",
    "pl_percent",
    "pl_dollars",
    "stop_loss",
    "risk_reward",
    "playbook_id",
    "notes",
    "mistakes",
    "entry_tactics",
    "edges_spotted",
    "market_regime",
    "pre_trade_conviction",
    "rule_adherence_score",
    "revenge_trade",
    "broke_30min_rule",
    "is_planned_pre_market",
    "tags",
    "violated_principle_ids",
];

#[derive(Debug, Serialize)]
pub struct McpAccount {
    pub id: String,
    pub name: String,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broker: Option<String>,
    pub risk_profile: String,
    /// The broker's reported total value. Absent when the broker reports none — which is
    /// not the same as zero, and must not be read as an empty account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_value: Option<f64>,
    pub brokerage_connected: bool,
    pub brokerage_connection_disabled: bool,
}

impl From<&Account> for McpAccount {
    fn from(a: &Account) -> Self {
        Self {
            id: a.id.clone(),
            name: a.name.clone(),
            currency: a.currency.clone(),
            broker: a.broker.as_deref().and_then(non_empty),
            risk_profile: a.risk_profile.clone(),
            total_value: a.total_value.filter(|v| *v > 0.0),
            brokerage_connected: a.snaptrade_connection_id.is_some(),
            brokerage_connection_disabled: a.snaptrade_connection_disabled,
        }
    }
}

pub const ANALYTICS_SECTIONS: &[&str] = &[
    "win_rate",
    "cumulative_profit",
    "average_risk_to_reward",
    "average_gain",
    "average_loss",
    "average_gain_pct",
    "average_loss_pct",
    "profit_factor",
    "biggest_win",
    "biggest_loss",
    "range_start",
    "range_end",
];

pub const ADVANCED_SECTIONS: &[&str] = &[
    "trade_count",
    "net_profit",
    "win_rate",
    "expectancy_dollars",
    "expectancy_r",
    "r_trade_count",
    "profit_factor",
    "sqn",
    "average_gain",
    "average_loss",
    "average_gain_pct",
    "average_loss_pct",
    "max_drawdown_dollars",
    "max_drawdown_pct",
    "current_drawdown_dollars",
    "recovery_factor",
    "longest_drawdown_days",
    "equity_curve",
    "starting_equity",
    "account_equity",
    "avg_planned_r",
    "avg_actual_r",
    "r_distribution",
    "longest_win_streak",
    "longest_loss_streak",
    "current_streak",
    "avg_hold_winners_secs",
    "avg_hold_losers_secs",
    "monthly_win_rate_stdev",
    "by_symbol",
    "by_day_of_week",
    "by_session",
    "by_holding",
    "by_direction",
    "by_position_size",
    "by_playbook",
    "clean_vs_flawed",
    "discipline",
    "tag_breakdowns",
    "trades_per_day",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> JournalEntry {
        JournalEntry {
            id: "t1".into(),
            user_id: "u1".into(),
            account_id: "a1".into(),
            open_date: "2026-03-03T17:28:00Z".into(),
            close_date: "2026-03-23T16:20:00Z".into(),
            entry_price: 112.8818,
            exit_price: 112.8543,
            position_size: 0.72,
            symbol: "CRCL".into(),
            symbol_name: "Circle Internet Group, Inc.".into(),
            status: "loss".into(),
            total_pl: -0.024_361_766_024_286_83,
            net_roi: -0.024_361_766_024_286_83,
            duration: 1_723_920,
            stop_loss: None,
            risk_reward: None,
            trade_type: "long".into(),
            mistakes: String::new(),
            entry_tactics: String::new(),
            edges_spotted: String::new(),
            playbook_id: None,
            notes: Some("panic buy".into()),
            broke_30min_rule: None,
            pre_trade_conviction: None,
            market_regime: None,
            is_planned_pre_market: None,
            revenge_trade: None,
            rule_adherence_score: None,
            created_at: "2026-07-08T23:09:35Z".into(),
        }
    }

    /// The bug this whole module exists for: `total_pl` is a percent, so a model that
    /// treats it as money reports -$0.024 when the real loss is -$0.0198.
    #[test]
    fn percent_and_dollars_are_separate_fields() {
        let t = McpTrade::from(&entry());
        assert_eq!(t.pl_percent, -0.024_361_766_024_286_83);
        let expected = 0.72 * 112.8818 * -0.024_361_766_024_286_83 / 100.0;
        assert!((t.pl_dollars - expected).abs() < 1e-12);
        assert!(t.pl_dollars > -0.02 && t.pl_dollars < 0.0);
    }

    #[test]
    fn internal_ids_and_empty_columns_never_reach_the_model() {
        let json = serde_json::to_value(McpTrade::from(&entry())).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("user_id"));
        // An exact duplicate of pl_percent; two names for one number invites the mistake.
        assert!(!obj.contains_key("net_roi"));
        assert!(!obj.contains_key("total_pl"));
        // Null / empty on every trade this user has ever logged.
        assert!(!obj.contains_key("mistakes"));
        assert!(!obj.contains_key("rule_adherence_score"));
        assert!(obj.contains_key("notes"));
    }

    #[test]
    fn a_broker_reporting_no_balance_is_absent_not_zero() {
        let mut a = Account {
            id: "a1".into(),
            user_id: "u1".into(),
            name: "Main".into(),
            icon: "i".into(),
            currency: "USD".into(),
            broker: None,
            risk_profile: "moderate".into(),
            snaptrade_user_id: Some("s".into()),
            snaptrade_user_secret_encrypted: Some("secret".into()),
            snaptrade_connection_id: Some("c".into()),
            snaptrade_account_id: Some("sa".into()),
            total_value: Some(0.0),
            total_value_currency: Some("USD".into()),
            snaptrade_connection_disabled: false,
            snaptrade_connection_disabled_at: None,
            created_at: "2026-05-14T06:12:09Z".into(),
            updated_at: "2026-07-11T05:00:07Z".into(),
        };
        assert_eq!(McpAccount::from(&a).total_value, None);
        a.total_value = Some(1250.0);
        assert_eq!(McpAccount::from(&a).total_value, Some(1250.0));

        let json = serde_json::to_value(McpAccount::from(&a)).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("user_id"));
        assert!(!obj.contains_key("snaptrade_user_id"));
        assert!(!obj.contains_key("snaptrade_connection_id"));
    }
}
