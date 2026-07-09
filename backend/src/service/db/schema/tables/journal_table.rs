use anyhow::{Context, Result, anyhow, ensure};
use async_graphql::{InputObject, SimpleObject};
use finance_query::Ticker;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::service::db::util::parse_flexible_datetime;
use crate::service::read_service::journal::JournalFilter;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase", complex)]
pub struct JournalEntry {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub open_date: String,
    pub close_date: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub position_size: f64,
    pub symbol: String,
    pub symbol_name: String,
    pub status: String,
    pub total_pl: f64,
    pub net_roi: f64,
    pub duration: i64,
    pub stop_loss: Option<f64>,
    pub risk_reward: Option<f64>,
    pub trade_type: String,
    pub mistakes: String,
    pub entry_tactics: String,
    pub edges_spotted: String,
    pub playbook_id: Option<String>,
    pub notes: Option<String>,
    pub broke_30min_rule: Option<bool>,
    pub pre_trade_conviction: Option<i32>,
    pub market_regime: Option<String>,
    pub is_planned_pre_market: Option<bool>,
    pub revenge_trade: Option<bool>,
    pub rule_adherence_score: Option<i32>,
    /// Immutable insertion time (RFC3339, microsecond precision). The stable
    /// keyset-pagination cursor field (close_date is user-editable, so unsafe).
    pub created_at: String,
}

#[derive(Debug, InputObject)]
pub struct CreateJournalEntryInput {
    pub account_id: String,
    pub open_date: String,
    pub close_date: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub position_size: f64,
    pub symbol: String,
    pub symbol_name: Option<String>,
    pub stop_loss: f64,
    pub trade_type: String,
    pub playbook_id: Option<String>,
    pub notes: Option<String>,
    pub broke_30min_rule: Option<bool>,
    pub pre_trade_conviction: Option<i32>,
    pub market_regime: Option<String>,
    pub is_planned_pre_market: Option<bool>,
    pub revenge_trade: Option<bool>,
    pub rule_adherence_score: Option<i32>,
    pub brokerage_transaction_ids: Option<Vec<String>>,
    /// Tag ids to attach to this trade. Persisted separately via
    /// `tags_table::set_trade_tags`; ignored by the journal_entries writer.
    #[graphql(default)]
    pub tag_ids: Vec<String>,
    /// Principle ids this trade violated. Persisted separately via
    /// `trading_principle_table::set_trade_principle_violations`; ignored by the
    /// `journal_entries` writer.
    #[graphql(default)]
    pub violated_principle_ids: Vec<String>,
}

#[derive(Debug, InputObject)]
pub struct UpdateJournalEntryInput {
    pub account_id: Option<String>,
    pub open_date: Option<String>,
    pub close_date: Option<String>,
    pub entry_price: Option<f64>,
    pub exit_price: Option<f64>,
    pub position_size: Option<f64>,
    pub symbol: Option<String>,
    pub symbol_name: Option<String>,
    pub stop_loss: Option<f64>,
    pub trade_type: Option<String>,
    pub playbook_id: Option<String>,
    #[graphql(default)]
    pub clear_playbook: bool,
    pub notes: Option<String>,
    #[graphql(default)]
    pub clear_notes: bool,
    pub broke_30min_rule: Option<bool>,
    pub pre_trade_conviction: Option<i32>,
    pub market_regime: Option<String>,
    pub is_planned_pre_market: Option<bool>,
    pub revenge_trade: Option<bool>,
    pub rule_adherence_score: Option<i32>,
    /// Tag ids to attach to this trade. Persisted separately via
    /// `tags_table::set_trade_tags`; ignored by the journal_entries writer.
    /// `None` (field omitted) leaves the trade's tags untouched; `Some([])`
    /// explicitly clears all tags.
    pub tag_ids: Option<Vec<String>>,
    /// `None` leaves existing violation links untouched; `Some(_)` replaces them.
    pub violated_principle_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
struct PreparedJournalEntry {
    account_id: String,
    open_date: String,
    close_date: String,
    entry_price: f64,
    exit_price: f64,
    position_size: f64,
    symbol: String,
    symbol_name: String,
    status: String,
    total_pl: f64,
    net_roi: f64,
    duration: i64,
    stop_loss: Option<f64>,
    risk_reward: Option<f64>,
    trade_type: String,
    mistakes: String,
    entry_tactics: String,
    edges_spotted: String,
    playbook_id: Option<String>,
    notes: Option<String>,
    broke_30min_rule: Option<bool>,
    pre_trade_conviction: Option<i32>,
    market_regime: Option<String>,
    is_planned_pre_market: Option<bool>,
    revenge_trade: Option<bool>,
    rule_adherence_score: Option<i32>,
}

#[derive(Debug, Clone)]
struct DerivedMetrics {
    status: String,
    total_pl: f64,
    net_roi: f64,
    duration: i64,
    risk_reward: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct JournalAggregateRow {
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub cumulative_profit: f64,
    pub gross_profit: f64,
    pub gross_loss: f64,
    pub sum_risk_reward: f64,
    /// Sum of percent returns over winning trades (for average gain %).
    pub sum_win_pct: f64,
    /// Sum of absolute percent returns over losing trades (for average loss %).
    pub sum_loss_pct: f64,
    /// Count of trades with a non-null `risk_reward` (denominator for the R:R average).
    pub risk_reward_count: i64,
}

#[derive(Debug, Clone)]
pub struct TradeOutcomeRow {
    pub symbol: String,
    pub symbol_name: String,
    pub amount: f64,
}

#[derive(Debug, Clone)]
pub struct CalendarDayAggregateRow {
    pub date: String,
    pub profit: f64,
    pub trade_count: i64,
    pub winning_trade_count: i64,
}

#[derive(Debug, Clone)]
pub struct PlaybookStatsRow {
    pub playbook_id: String,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub cumulative_profit: f64,
    pub gross_profit: f64,
    pub gross_loss: f64,
}

#[derive(Debug, Clone)]
pub struct PrincipleStatsRow {
    pub principle_id: String,
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    /// Dollars, via DOLLAR_PL_EXPR.
    pub cumulative_profit: f64,
    /// Percent — the raw SUM(total_pl).
    pub cumulative_roi: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum ExtremeKind {
    Best,
    Worst,
}

const SELECT_COLS: &str = "id, user_id, account_id, to_char(open_date AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS open_date, to_char(close_date AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS close_date, entry_price, exit_price, position_size, symbol, symbol_name, status, total_pl, net_roi, duration, stop_loss, risk_reward, trade_type, mistakes, entry_tactics, edges_spotted, playbook_id, notes, broke_30min_rule, pre_trade_conviction, market_regime, is_planned_pre_market, revenge_trade, rule_adherence_score, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at";

const DOLLAR_PL_EXPR: &str = "position_size * entry_price * total_pl / 100.0";

fn row_to_journal_entry(row: &sqlx::postgres::PgRow) -> Result<JournalEntry> {
    Ok(JournalEntry {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        account_id: row.try_get::<String, _>(2)?,
        open_date: row.try_get::<String, _>(3)?,
        close_date: row.try_get::<String, _>(4)?,
        entry_price: row.try_get::<f64, _>(5)?,
        exit_price: row.try_get::<f64, _>(6)?,
        position_size: row.try_get::<f64, _>(7)?,
        symbol: row.try_get::<String, _>(8)?,
        symbol_name: row.try_get::<String, _>(9)?,
        status: row.try_get::<String, _>(10)?,
        total_pl: row.try_get::<f64, _>(11)?,
        net_roi: row.try_get::<f64, _>(12)?,
        duration: row.try_get::<i64, _>(13)?,
        stop_loss: row.try_get::<Option<f64>, _>(14)?,
        risk_reward: row.try_get::<Option<f64>, _>(15)?,
        trade_type: row.try_get::<String, _>(16)?,
        mistakes: row.try_get::<String, _>(17)?,
        entry_tactics: row.try_get::<String, _>(18)?,
        edges_spotted: row.try_get::<String, _>(19)?,
        playbook_id: row.try_get::<Option<String>, _>(20)?,
        notes: row.try_get::<Option<String>, _>(21)?,
        broke_30min_rule: row.try_get::<Option<bool>, _>(22)?,
        pre_trade_conviction: row.try_get::<Option<i32>, _>(23)?,
        market_regime: row.try_get::<Option<String>, _>(24)?,
        is_planned_pre_market: row.try_get::<Option<bool>, _>(25)?,
        revenge_trade: row.try_get::<Option<bool>, _>(26)?,
        rule_adherence_score: row.try_get::<Option<i32>, _>(27)?,
        created_at: row.try_get::<String, _>(28)?,
    })
}

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

fn normalize_optional_notes(notes: Option<String>) -> Option<String> {
    normalize_optional_text(notes)
}

fn normalize_symbol_name_candidates(
    short_name: Option<String>,
    long_name: Option<String>,
    symbol: &str,
) -> Result<String> {
    if let Some(short_name) = short_name {
        let trimmed = short_name.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    if let Some(long_name) = long_name {
        let trimmed = long_name.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    Err(anyhow!(
        "No symbol name returned for symbol '{symbol}' from finance-query"
    ))
}

async fn resolve_symbol_name(symbol: &str, provided_symbol_name: Option<String>) -> Result<String> {
    if let Some(symbol_name) = provided_symbol_name {
        return normalize_required_text(&symbol_name, "symbol_name");
    }

    let ticker = Ticker::new(symbol)
        .await
        .with_context(|| format!("Failed to initialize finance-query ticker for '{symbol}'"))?;
    let quote = ticker
        .quote()
        .await
        .with_context(|| format!("Failed to fetch quote data for symbol '{symbol}'"))?;

    normalize_symbol_name_candidates(quote.short_name, quote.long_name, symbol)
}

fn normalize_trade_type(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_lowercase();
    ensure!(
        matches!(normalized.as_str(), "long" | "short"),
        "trade_type must be either 'long' or 'short'"
    );
    Ok(normalized)
}

fn ensure_positive_price(value: f64, field: &str) -> Result<f64> {
    ensure!(value.is_finite(), "{field} must be a finite number");
    ensure!(value > 0.0, "{field} must be greater than zero");
    Ok(value)
}

/// Like `ensure_positive_price` but allows zero. Used for stop_loss, where 0
/// means "no stop loss recorded".
fn ensure_non_negative_price(value: f64, field: &str) -> Result<f64> {
    ensure!(value.is_finite(), "{field} must be a finite number");
    ensure!(value >= 0.0, "{field} must be zero or greater");
    Ok(value)
}

fn calculate_derived_metrics(
    open_date: &str,
    close_date: &str,
    entry_price: f64,
    exit_price: f64,
    stop_loss: Option<f64>,
    trade_type: &str,
) -> Result<DerivedMetrics> {
    let open = parse_flexible_datetime(open_date)?;
    let close = parse_flexible_datetime(close_date)?;
    ensure!(close >= open, "close_date must be on or after open_date");

    let pl_ratio = match trade_type {
        "long" => (exit_price - entry_price) / entry_price,
        "short" => (entry_price - exit_price) / entry_price,
        _ => return Err(anyhow!("Unsupported trade_type")),
    };

    // A trade with no stop (`None`) has no determinable R; its risk_reward is
    // NULL and R-based analytics skip it.
    let risk_reward = match stop_loss {
        None => None,
        Some(stop) => {
            let risk_distance = match trade_type {
                "long" => entry_price - stop,
                "short" => stop - entry_price,
                _ => unreachable!(),
            };
            ensure!(
                risk_distance > 0.0,
                "stop_loss must be below entry_price for long trades and above entry_price for short trades"
            );
            let reward_distance = match trade_type {
                "long" => exit_price - entry_price,
                "short" => entry_price - exit_price,
                _ => unreachable!(),
            };
            Some(reward_distance / risk_distance)
        }
    };

    let total_pl = pl_ratio * 100.0;
    let duration = (close - open).num_seconds();

    Ok(DerivedMetrics {
        status: if total_pl < 0.0 {
            "loss".to_string()
        } else {
            "profit".to_string()
        },
        total_pl,
        net_roi: total_pl,
        duration,
        risk_reward,
    })
}

async fn prepare_new_entry(input: CreateJournalEntryInput) -> Result<PreparedJournalEntry> {
    let account_id = normalize_required_text(&input.account_id, "account_id")?;
    let open_date = normalize_required_text(&input.open_date, "open_date")?;
    let close_date = normalize_required_text(&input.close_date, "close_date")?;
    let symbol = normalize_required_text(&input.symbol, "symbol")?.to_ascii_uppercase();
    let trade_type = normalize_trade_type(&input.trade_type)?;
    let entry_price = ensure_positive_price(input.entry_price, "entry_price")?;
    let exit_price = ensure_positive_price(input.exit_price, "exit_price")?;
    let position_size = ensure_positive_price(input.position_size, "position_size")?;
    let stop_loss = ensure_non_negative_price(input.stop_loss, "stop_loss")?;
    // 0 means "no stop recorded" and is stored as NULL, not the 0.0 sentinel.
    let stop_loss = if stop_loss == 0.0 {
        None
    } else {
        Some(stop_loss)
    };
    if let Some(v) = input.pre_trade_conviction {
        ensure!(
            (1..=5).contains(&v),
            "pre_trade_conviction must be between 1 and 5"
        );
    }
    if let Some(v) = input.rule_adherence_score {
        ensure!(
            (1..=5).contains(&v),
            "rule_adherence_score must be between 1 and 5"
        );
    }
    let symbol_name = resolve_symbol_name(&symbol, input.symbol_name).await?;
    let metrics = calculate_derived_metrics(
        &open_date,
        &close_date,
        entry_price,
        exit_price,
        stop_loss,
        &trade_type,
    )?;

    Ok(PreparedJournalEntry {
        account_id,
        open_date,
        close_date,
        entry_price,
        exit_price,
        position_size,
        symbol,
        symbol_name,
        status: metrics.status,
        total_pl: metrics.total_pl,
        net_roi: metrics.net_roi,
        duration: metrics.duration,
        stop_loss,
        risk_reward: metrics.risk_reward,
        trade_type,
        // Legacy freeform columns are frozen: new trades write empty strings
        // (tags replace them). Existing rows keep their historical values.
        mistakes: String::new(),
        entry_tactics: String::new(),
        edges_spotted: String::new(),
        playbook_id: normalize_optional_text(input.playbook_id),
        notes: normalize_optional_notes(input.notes),
        broke_30min_rule: input.broke_30min_rule,
        pre_trade_conviction: input.pre_trade_conviction,
        market_regime: normalize_optional_text(input.market_regime),
        is_planned_pre_market: input.is_planned_pre_market,
        revenge_trade: input.revenge_trade,
        rule_adherence_score: input.rule_adherence_score,
    })
}

async fn prepare_updated_entry(
    current: &JournalEntry,
    input: UpdateJournalEntryInput,
) -> Result<PreparedJournalEntry> {
    let symbol_from_input = input.symbol.clone();
    let symbol_name_from_input = input.symbol_name.clone();
    let account_id = input
        .account_id
        .unwrap_or_else(|| current.account_id.clone());
    let open_date = input.open_date.unwrap_or_else(|| current.open_date.clone());
    let close_date = input
        .close_date
        .unwrap_or_else(|| current.close_date.clone());
    let entry_price = input.entry_price.unwrap_or(current.entry_price);
    let exit_price = input.exit_price.unwrap_or(current.exit_price);
    let position_size = input.position_size.unwrap_or(current.position_size);
    let symbol = input.symbol.unwrap_or_else(|| current.symbol.clone());
    let stop_loss = input
        .stop_loss
        .unwrap_or_else(|| current.stop_loss.unwrap_or(0.0));
    let trade_type = input
        .trade_type
        .unwrap_or_else(|| current.trade_type.clone());
    let playbook_id = if input.clear_playbook {
        None
    } else if input.playbook_id.is_some() {
        normalize_optional_text(input.playbook_id)
    } else {
        current.playbook_id.clone()
    };

    let notes = if input.clear_notes {
        None
    } else if input.notes.is_some() {
        normalize_optional_notes(input.notes)
    } else {
        current.notes.clone()
    };

    let symbol_name = if let Some(symbol_name) = symbol_name_from_input {
        Some(symbol_name)
    } else if symbol_from_input.is_some() && symbol != current.symbol {
        Some(symbol.clone())
    } else {
        Some(current.symbol_name.clone())
    };

    // Provided value wins; otherwise keep the current stored value.
    let broke_30min_rule = input.broke_30min_rule.or(current.broke_30min_rule);
    let pre_trade_conviction = input.pre_trade_conviction.or(current.pre_trade_conviction);
    let market_regime = input
        .market_regime
        .or_else(|| current.market_regime.clone());
    let is_planned_pre_market = input
        .is_planned_pre_market
        .or(current.is_planned_pre_market);
    let revenge_trade = input.revenge_trade.or(current.revenge_trade);
    let rule_adherence_score = input.rule_adherence_score.or(current.rule_adherence_score);

    prepare_new_entry(CreateJournalEntryInput {
        account_id,
        open_date,
        close_date,
        entry_price,
        exit_price,
        position_size,
        symbol,
        symbol_name,
        stop_loss,
        trade_type,
        playbook_id,
        notes,
        broke_30min_rule,
        pre_trade_conviction,
        market_regime,
        is_planned_pre_market,
        revenge_trade,
        rule_adherence_score,
        brokerage_transaction_ids: None,
        tag_ids: Vec::new(),
        violated_principle_ids: Vec::new(),
    })
    .await
}

async fn validate_playbook_exists(
    pool: &PgPool,
    user_id: &str,
    playbook_id: Option<String>,
) -> Result<Option<String>> {
    match playbook_id {
        Some(playbook_id) => {
            let row = sqlx::query("SELECT 1 FROM playbooks WHERE id = $1 AND user_id = $2 LIMIT 1")
                .bind(playbook_id.as_str())
                .bind(user_id)
                .fetch_optional(pool)
                .await
                .context("Failed to validate playbook reference")?;

            ensure!(row.is_some(), "playbook '{playbook_id}' was not found");
            Ok(Some(playbook_id))
        }
        None => Ok(None),
    }
}

pub async fn list_journal_entries(pool: &PgPool, user_id: &str) -> Result<Vec<JournalEntry>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM journal_entries WHERE user_id = $1 ORDER BY open_date DESC, close_date DESC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .fetch_all(pool)
        .await
        .context("Failed to list journal entries")?;

    let mut entries = Vec::new();
    for row in &rows {
        entries.push(row_to_journal_entry(row)?);
    }
    Ok(entries)
}

/// The `YYYY-MM-DD` prefix of a date string. `close_date` is stored/rendered in
/// UTC, so its first 10 chars are its UTC calendar date — mirroring the
/// in-memory `date_part` the MCP layer previously used for filtering.
fn date_only(s: &str) -> &str {
    if s.len() >= 10 { &s[..10] } else { s.trim() }
}

/// Escape LIKE wildcards so `term` matches literally, wrapped in `%…%` for a
/// substring (contains) match. Paired with `ILIKE … ESCAPE '\'`.
fn like_contains(term: &str) -> String {
    let escaped = term
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

/// Build the parameterized SELECT for [`list_journal_entries_filtered`].
///
/// Pure and DB-free so the placeholder numbering can be unit-tested. Placeholdered
/// predicates bump `n` (from 2) in a FIXED order; predicates needing no bind
/// (`playbook IS NULL`, `has_stop_loss`) append text WITHOUT consuming a
/// placeholder. The bind order in `list_journal_entries_filtered` MUST mirror
/// this sequence exactly.
fn build_filtered_sql(filter: &JournalFilter) -> String {
    let mut sql = format!("SELECT {SELECT_COLS} FROM journal_entries WHERE user_id = $1");
    let mut n = 1;
    if filter.account_id.is_some() {
        n += 1;
        sql.push_str(&format!(" AND account_id = ${n}"));
    }
    if filter.symbol.is_some() {
        n += 1;
        sql.push_str(&format!(" AND UPPER(symbol) = ${n}"));
    }
    match &filter.playbook_id {
        None => {}
        Some(None) => sql.push_str(" AND playbook_id IS NULL"),
        Some(Some(_)) => {
            n += 1;
            sql.push_str(&format!(" AND playbook_id = ${n}"));
        }
    }
    if filter.status.is_some() {
        n += 1;
        sql.push_str(&format!(" AND status = ${n}"));
    }
    if filter.min_pl_pct.is_some() {
        n += 1;
        sql.push_str(&format!(" AND total_pl >= ${n}"));
    }
    if filter.max_pl_pct.is_some() {
        n += 1;
        sql.push_str(&format!(" AND total_pl <= ${n}"));
    }
    match filter.has_stop_loss {
        None => {}
        Some(true) => sql.push_str(" AND stop_loss IS NOT NULL"),
        Some(false) => sql.push_str(" AND stop_loss IS NULL"),
    }
    if filter.mistake_contains.is_some() {
        n += 1;
        sql.push_str(&format!(" AND mistakes ILIKE ${n} ESCAPE '\\'"));
    }
    if filter.date_from.is_some() {
        n += 1;
        sql.push_str(&format!(
            " AND to_char(close_date AT TIME ZONE 'UTC', 'YYYY-MM-DD') >= ${n}"
        ));
    }
    if filter.date_to.is_some() {
        n += 1;
        sql.push_str(&format!(
            " AND to_char(close_date AT TIME ZONE 'UTC', 'YYYY-MM-DD') <= ${n}"
        ));
    }
    if filter.after.is_some() {
        // Keyset: rows strictly after the cursor in (created_at, id) DESC order.
        // Row-value comparison is the SQL-standard, index-friendly form.
        let a = n + 1;
        let b = n + 2;
        n = b;
        sql.push_str(&format!(
            " AND (created_at, id) < (${a}::timestamptz, ${b}::text)"
        ));
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC");
    n += 1;
    sql.push_str(&format!(" LIMIT ${n}"));
    sql
}

/// List a user's journal entries with the given [`JournalFilter`] applied in SQL
/// (indexes: `idx_journal_entries_symbol_upper`, `idx_journal_entries_playbook_id`,
/// `idx_journal_entries_user_open_close`), ordered newest-first and capped by
/// `filter.limit` (default 50, max 500). Replaces the previous
/// load-every-entry-then-filter-in-memory path.
pub async fn list_journal_entries_filtered(
    pool: &PgPool,
    user_id: &str,
    filter: &JournalFilter,
) -> Result<Vec<JournalEntry>> {
    let sql = build_filtered_sql(filter);

    // Bind order MUST mirror the `if`/`match` order in `build_filtered_sql`.
    // Predicates that emit no `$n` (playbook IS NULL, has_stop_loss) bind nothing.
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(user_id);
    if let Some(account_id) = &filter.account_id {
        query = query.bind(account_id.clone());
    }
    if let Some(symbol) = &filter.symbol {
        query = query.bind(symbol.to_ascii_uppercase());
    }
    if let Some(Some(playbook_id)) = &filter.playbook_id {
        query = query.bind(playbook_id.clone());
    }
    if let Some(status) = filter.status {
        query = query.bind(status.as_str());
    }
    if let Some(min_pl) = filter.min_pl_pct {
        query = query.bind(min_pl);
    }
    if let Some(max_pl) = filter.max_pl_pct {
        query = query.bind(max_pl);
    }
    if let Some(term) = &filter.mistake_contains {
        query = query.bind(like_contains(term));
    }
    if let Some(date_from) = &filter.date_from {
        query = query.bind(date_only(date_from).to_string());
    }
    if let Some(date_to) = &filter.date_to {
        query = query.bind(date_only(date_to).to_string());
    }
    if let Some((created_at, id)) = &filter.after {
        query = query.bind(parse_flexible_datetime(created_at)?);
        query = query.bind(id.clone());
    }
    let cap = filter.limit.unwrap_or(50).min(500) as i64;
    query = query.bind(cap);

    let rows = query
        .fetch_all(pool)
        .await
        .context("Failed to list filtered journal entries")?;

    let mut entries = Vec::with_capacity(rows.len());
    for row in &rows {
        entries.push(row_to_journal_entry(row)?);
    }
    Ok(entries)
}

pub async fn aggregate_journal_analytics(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    start_iso: &str,
    end_iso: &str,
) -> Result<JournalAggregateRow> {
    let sql = format!(
        "
        SELECT
            COUNT(*) AS total_trades,
            COALESCE(SUM(CASE WHEN total_pl > 0 THEN 1 ELSE 0 END), 0) AS winning_trades,
            COALESCE(SUM(CASE WHEN total_pl < 0 THEN 1 ELSE 0 END), 0) AS losing_trades,
            COALESCE(SUM({DOLLAR_PL_EXPR}), 0.0) AS cumulative_profit,
            COALESCE(SUM(CASE WHEN total_pl > 0 THEN {DOLLAR_PL_EXPR} ELSE 0.0 END), 0.0) AS gross_profit,
            COALESCE(SUM(CASE WHEN total_pl < 0 THEN ABS({DOLLAR_PL_EXPR}) ELSE 0.0 END), 0.0) AS gross_loss,
            COALESCE(SUM(risk_reward), 0.0) AS sum_risk_reward,
            COALESCE(SUM(CASE WHEN total_pl > 0 THEN total_pl ELSE 0.0 END), 0.0) AS sum_win_pct,
            COALESCE(SUM(CASE WHEN total_pl < 0 THEN ABS(total_pl) ELSE 0.0 END), 0.0) AS sum_loss_pct,
            COUNT(risk_reward) AS risk_reward_count
        FROM journal_entries
        WHERE user_id = $1
          AND account_id = $2
          AND close_date >= $3
          AND close_date <= $4
    "
    );

    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .bind(parse_flexible_datetime(start_iso)?)
        .bind(parse_flexible_datetime(end_iso)?)
        .fetch_optional(pool)
        .await
        .context("Failed to aggregate journal analytics")?
        .ok_or_else(|| anyhow!("Aggregate query returned no rows"))?;

    Ok(JournalAggregateRow {
        total_trades: row.try_get::<i64, _>(0)?,
        winning_trades: row.try_get::<i64, _>(1)?,
        losing_trades: row.try_get::<i64, _>(2)?,
        cumulative_profit: row.try_get::<f64, _>(3)?,
        gross_profit: row.try_get::<f64, _>(4)?,
        gross_loss: row.try_get::<f64, _>(5)?,
        sum_risk_reward: row.try_get::<f64, _>(6)?,
        sum_win_pct: row.try_get::<f64, _>(7)?,
        sum_loss_pct: row.try_get::<f64, _>(8)?,
        risk_reward_count: row.try_get::<i64, _>(9)?,
    })
}

/// Returns `None` if no profitable (Best) or losing (Worst) trade exists in the window.
pub async fn find_extreme_trade(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    start_iso: &str,
    end_iso: &str,
    kind: ExtremeKind,
) -> Result<Option<TradeOutcomeRow>> {
    let (sign_filter, order) = match kind {
        ExtremeKind::Best => ("> 0", "DESC"),
        ExtremeKind::Worst => ("< 0", "ASC"),
    };

    let sql = format!(
        "SELECT symbol, symbol_name, {DOLLAR_PL_EXPR} AS amount
         FROM journal_entries
         WHERE user_id = $1
           AND account_id = $2
           AND close_date >= $3
           AND close_date <= $4
           AND total_pl {sign_filter}
         ORDER BY amount {order}
         LIMIT 1"
    );

    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .bind(parse_flexible_datetime(start_iso)?)
        .bind(parse_flexible_datetime(end_iso)?)
        .fetch_optional(pool)
        .await
        .context("Failed to query extreme trade")?;

    match row {
        Some(row) => Ok(Some(TradeOutcomeRow {
            symbol: row.try_get::<String, _>(0)?,
            symbol_name: row.try_get::<String, _>(1)?,
            amount: row.try_get::<f64, _>(2)?,
        })),
        None => Ok(None),
    }
}

pub async fn aggregate_calendar_days(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    month_start_iso: &str,
    month_end_iso: &str,
) -> Result<Vec<CalendarDayAggregateRow>> {
    let sql = format!(
        "
        SELECT
            to_char(close_date AT TIME ZONE 'UTC', 'YYYY-MM-DD') AS day,
            COALESCE(SUM({DOLLAR_PL_EXPR}), 0.0) AS profit,
            COUNT(*) AS trade_count,
            COALESCE(SUM(CASE WHEN total_pl > 0 THEN 1 ELSE 0 END), 0) AS winning_trade_count
        FROM journal_entries
        WHERE user_id = $1
          AND account_id = $2
          AND close_date >= $3
          AND close_date <= $4
        GROUP BY to_char(close_date AT TIME ZONE 'UTC', 'YYYY-MM-DD')
    "
    );

    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .bind(parse_flexible_datetime(month_start_iso)?)
        .bind(parse_flexible_datetime(month_end_iso)?)
        .fetch_all(pool)
        .await
        .context("Failed to aggregate calendar days")?;

    let mut days = Vec::new();
    for row in &rows {
        days.push(CalendarDayAggregateRow {
            date: row.try_get::<String, _>(0)?,
            profit: row.try_get::<f64, _>(1)?,
            trade_count: row.try_get::<i64, _>(2)?,
            winning_trade_count: row.try_get::<i64, _>(3)?,
        });
    }
    Ok(days)
}

pub async fn find_journal_entry(
    pool: &PgPool,
    id: &str,
    user_id: &str,
) -> Result<Option<JournalEntry>> {
    let sql = format!("SELECT {SELECT_COLS} FROM journal_entries WHERE id = $1 AND user_id = $2");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("Failed to find journal entry")?;

    match row {
        Some(row) => Ok(Some(row_to_journal_entry(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_journal_entry(
    pool: &PgPool,
    user_id: &str,
    input: CreateJournalEntryInput,
) -> Result<JournalEntry> {
    let id = Uuid::new_v4().to_string();
    let brokerage_tx_ids = input.brokerage_transaction_ids.clone();
    let mut entry = prepare_new_entry(input).await?;
    entry.playbook_id = validate_playbook_exists(pool, user_id, entry.playbook_id).await?;

    let open_ts = parse_flexible_datetime(&entry.open_date)?;
    let close_ts = parse_flexible_datetime(&entry.close_date)?;

    sqlx::query(
        "INSERT INTO journal_entries (id, user_id, account_id, open_date, close_date, entry_price, exit_price, position_size, symbol, symbol_name, status, total_pl, net_roi, duration, stop_loss, risk_reward, trade_type, mistakes, entry_tactics, edges_spotted, playbook_id, notes, broke_30min_rule, pre_trade_conviction, market_regime, is_planned_pre_market, revenge_trade, rule_adherence_score) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28)",
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(entry.account_id.as_str())
    .bind(open_ts)
    .bind(close_ts)
    .bind(entry.entry_price)
    .bind(entry.exit_price)
    .bind(entry.position_size)
    .bind(entry.symbol.as_str())
    .bind(entry.symbol_name.as_str())
    .bind(entry.status.as_str())
    .bind(entry.total_pl)
    .bind(entry.net_roi)
    .bind(entry.duration)
    .bind(entry.stop_loss)
    .bind(entry.risk_reward)
    .bind(entry.trade_type.as_str())
    .bind(entry.mistakes.as_str())
    .bind(entry.entry_tactics.as_str())
    .bind(entry.edges_spotted.as_str())
    .bind(entry.playbook_id.as_deref())
    .bind(entry.notes.as_deref())
    .bind(entry.broke_30min_rule)
    .bind(entry.pre_trade_conviction)
    .bind(entry.market_regime.as_deref())
    .bind(entry.is_planned_pre_market)
    .bind(entry.revenge_trade)
    .bind(entry.rule_adherence_score)
    .execute(pool)
    .await
    .context("Failed to insert journal entry")?;

    if let Some(ref tx_ids) = brokerage_tx_ids
        && !tx_ids.is_empty()
    {
        insert_brokerage_links(pool, &id, user_id, tx_ids).await?;
    }

    find_journal_entry(pool, &id, user_id)
        .await?
        .context("Journal entry not found after insert")
}

pub async fn update_journal_entry(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    input: UpdateJournalEntryInput,
) -> Result<JournalEntry> {
    let current = find_journal_entry(pool, id, user_id)
        .await?
        .context("Journal entry not found")?;
    let entry = prepare_updated_entry(&current, input).await?;
    let entry = PreparedJournalEntry {
        playbook_id: validate_playbook_exists(pool, user_id, entry.playbook_id).await?,
        ..entry
    };

    let open_ts = parse_flexible_datetime(&entry.open_date)?;
    let close_ts = parse_flexible_datetime(&entry.close_date)?;

    sqlx::query(
        // Legacy freeform columns (mistakes/entry_tactics/edges_spotted) are
        // intentionally omitted from the UPDATE set so they stay frozen.
        "UPDATE journal_entries SET account_id = $1, open_date = $2, close_date = $3, entry_price = $4, exit_price = $5, position_size = $6, symbol = $7, symbol_name = $8, status = $9, total_pl = $10, net_roi = $11, duration = $12, stop_loss = $13, risk_reward = $14, trade_type = $15, playbook_id = $16, notes = $17, broke_30min_rule = $18, pre_trade_conviction = $19, market_regime = $20, is_planned_pre_market = $21, revenge_trade = $22, rule_adherence_score = $23 WHERE id = $24 AND user_id = $25",
    )
    .bind(entry.account_id.as_str())
    .bind(open_ts)
    .bind(close_ts)
    .bind(entry.entry_price)
    .bind(entry.exit_price)
    .bind(entry.position_size)
    .bind(entry.symbol.as_str())
    .bind(entry.symbol_name.as_str())
    .bind(entry.status.as_str())
    .bind(entry.total_pl)
    .bind(entry.net_roi)
    .bind(entry.duration)
    .bind(entry.stop_loss)
    .bind(entry.risk_reward)
    .bind(entry.trade_type.as_str())
    .bind(entry.playbook_id.as_deref())
    .bind(entry.notes.as_deref())
    .bind(entry.broke_30min_rule)
    .bind(entry.pre_trade_conviction)
    .bind(entry.market_regime.as_deref())
    .bind(entry.is_planned_pre_market)
    .bind(entry.revenge_trade)
    .bind(entry.rule_adherence_score)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Failed to update journal entry")?;

    find_journal_entry(pool, id, user_id)
        .await?
        .context("Journal entry not found after update")
}

pub async fn delete_journal_entry(pool: &PgPool, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = sqlx::query("DELETE FROM journal_entries WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to delete journal entry")?
        .rows_affected();

    Ok(rows_affected > 0)
}

/// Insert links between a journal entry and brokerage transactions.
pub async fn insert_brokerage_links(
    pool: &PgPool,
    journal_entry_id: &str,
    user_id: &str,
    brokerage_transaction_ids: &[String],
) -> Result<()> {
    for tx_id in brokerage_transaction_ids {
        let link_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO journal_brokerage_links (id, journal_entry_id, brokerage_transaction_id, user_id) VALUES ($1, $2, $3, $4)",
        )
        .bind(link_id.as_str())
        .bind(journal_entry_id)
        .bind(tx_id.as_str())
        .bind(user_id)
        .execute(pool)
        .await
        .context(format!("Failed to insert brokerage link for transaction {}", tx_id))?;
    }
    Ok(())
}

/// Get all brokerage transaction IDs that are already linked to journal entries for a user+account.
pub async fn list_linked_brokerage_transaction_ids(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<String>> {
    let rows = sqlx::query(
        "SELECT jbl.brokerage_transaction_id FROM journal_brokerage_links jbl
             INNER JOIN journal_entries je ON je.id = jbl.journal_entry_id
             WHERE jbl.user_id = $1 AND je.account_id = $2",
    )
    .bind(user_id)
    .bind(account_id)
    .fetch_all(pool)
    .await
    .context("Failed to query linked brokerage transaction IDs")?;

    let mut ids = Vec::new();
    for row in &rows {
        let id: String = row.try_get(0)?;
        ids.push(id);
    }
    Ok(ids)
}

/// Load all journal entries for `account_id` whose `close_date` falls within
/// [`start_iso`, `end_iso`] (inclusive), using the same `datetime()` comparison
/// convention as `aggregate_journal_analytics`. Results are ordered by
/// `close_date ASC` so callers can use them directly for equity-curve work.
pub async fn list_journal_entries_for_account_in_range(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    start_iso: &str,
    end_iso: &str,
) -> Result<Vec<JournalEntry>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM journal_entries
         WHERE user_id = $1
           AND account_id = $2
           AND close_date >= $3
           AND close_date <= $4
         ORDER BY close_date ASC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .bind(parse_flexible_datetime(start_iso)?)
        .bind(parse_flexible_datetime(end_iso)?)
        .fetch_all(pool)
        .await
        .context("Failed to list journal entries for account in range")?;

    let mut entries = Vec::new();
    for row in &rows {
        entries.push(row_to_journal_entry(row)?);
    }
    Ok(entries)
}

pub async fn aggregate_stats_per_playbook(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<PlaybookStatsRow>> {
    let sql = format!(
        "SELECT
            playbook_id,
            COUNT(*) AS total_trades,
            COALESCE(SUM(CASE WHEN total_pl > 0 THEN 1 ELSE 0 END), 0) AS winning_trades,
            COALESCE(SUM(CASE WHEN total_pl < 0 THEN 1 ELSE 0 END), 0) AS losing_trades,
            COALESCE(SUM({DOLLAR_PL_EXPR}), 0.0) AS cumulative_profit,
            COALESCE(SUM(CASE WHEN total_pl > 0 THEN {DOLLAR_PL_EXPR} ELSE 0.0 END), 0.0) AS gross_profit,
            COALESCE(SUM(CASE WHEN total_pl < 0 THEN ABS({DOLLAR_PL_EXPR}) ELSE 0.0 END), 0.0) AS gross_loss
         FROM journal_entries
         WHERE user_id = $1
           AND playbook_id IS NOT NULL
         GROUP BY playbook_id"
    );

    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .fetch_all(pool)
        .await
        .context("Failed to aggregate stats per playbook")?;

    let mut stats = Vec::new();
    for row in &rows {
        stats.push(PlaybookStatsRow {
            playbook_id: row.try_get::<String, _>(0)?,
            total_trades: row.try_get::<i64, _>(1)?,
            winning_trades: row.try_get::<i64, _>(2)?,
            losing_trades: row.try_get::<i64, _>(3)?,
            cumulative_profit: row.try_get::<f64, _>(4)?,
            gross_profit: row.try_get::<f64, _>(5)?,
            gross_loss: row.try_get::<f64, _>(6)?,
        });
    }
    Ok(stats)
}

/// Per-principle stats over the trades that violated it, scoped to one account.
///
/// `total_pl` is a percent; dollars must be derived. Win/loss counts exclude
/// breakeven, matching `aggregate_stats_per_playbook`.
pub async fn aggregate_violation_stats_per_principle(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<PrincipleStatsRow>> {
    // Table-aliased form of DOLLAR_PL_EXPR; this query joins journal_entries as `e`.
    const ALIASED_DOLLAR_PL: &str = "e.position_size * e.entry_price * e.total_pl / 100.0";

    let sql = format!(
        "SELECT
            v.principle_id,
            COUNT(*) AS total_trades,
            COALESCE(SUM(CASE WHEN e.total_pl > 0 THEN 1 ELSE 0 END), 0) AS winning_trades,
            COALESCE(SUM(CASE WHEN e.total_pl < 0 THEN 1 ELSE 0 END), 0) AS losing_trades,
            COALESCE(SUM({ALIASED_DOLLAR_PL}), 0.0) AS cumulative_profit,
            COALESCE(SUM(e.total_pl), 0.0) AS cumulative_roi
         FROM trade_principle_violations v
         JOIN journal_entries e ON e.id = v.journal_entry_id
         WHERE e.user_id = $1
           AND e.account_id = $2
         GROUP BY v.principle_id"
    );

    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .fetch_all(pool)
        .await
        .context("Failed to aggregate violation stats per principle")?;

    let mut stats = Vec::new();
    for row in &rows {
        stats.push(PrincipleStatsRow {
            principle_id: row.try_get::<String, _>(0)?,
            total_trades: row.try_get::<i64, _>(1)?,
            winning_trades: row.try_get::<i64, _>(2)?,
            losing_trades: row.try_get::<i64, _>(3)?,
            cumulative_profit: row.try_get::<f64, _>(4)?,
            cumulative_roi: row.try_get::<f64, _>(5)?,
        });
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::calculate_derived_metrics;

    #[test]
    fn calculates_metrics_for_long_trade() {
        let metrics =
            calculate_derived_metrics("2026-01-01", "2026-01-03", 100.0, 110.0, Some(95.0), "long")
                .unwrap();

        assert_eq!(metrics.status, "profit");
        assert_eq!(metrics.total_pl, 10.0);
        assert_eq!(metrics.net_roi, 10.0);
        assert_eq!(metrics.duration, 172800);
        assert_eq!(metrics.risk_reward, Some(2.0));
    }

    #[test]
    fn calculates_metrics_for_short_trade() {
        let metrics = calculate_derived_metrics(
            "2026-01-01T09:00:00Z",
            "2026-01-01T11:30:00Z",
            100.0,
            90.0,
            Some(105.0),
            "short",
        )
        .unwrap();

        assert_eq!(metrics.status, "profit");
        assert_eq!(metrics.total_pl, 10.0);
        assert_eq!(metrics.net_roi, 10.0);
        assert_eq!(metrics.duration, 9000);
        assert_eq!(metrics.risk_reward, Some(2.0));
    }

    #[test]
    fn rejects_invalid_stop_loss_position() {
        let error = calculate_derived_metrics(
            "2026-01-01",
            "2026-01-02",
            100.0,
            102.0,
            Some(101.0),
            "long",
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("stop_loss must be below entry_price")
        );
    }

    use super::{JournalFilter, build_filtered_sql, like_contains};
    use crate::service::read_service::journal::TradeStatus;

    fn empty_filter() -> JournalFilter {
        JournalFilter::default()
    }

    #[test]
    fn filtered_sql_no_filters_is_user_scoped_with_limit_placeholder() {
        let sql = build_filtered_sql(&empty_filter());
        assert!(sql.contains("WHERE user_id = $1"), "{sql}");
        assert!(!sql.contains(" AND "), "{sql}");
        assert!(sql.contains("ORDER BY created_at DESC, id DESC"), "{sql}");
        assert!(sql.trim_end().ends_with("LIMIT $2"), "{sql}");
    }

    #[test]
    fn filtered_sql_symbol_is_case_insensitive_and_indexable() {
        let sql = build_filtered_sql(&JournalFilter {
            symbol: Some("aapl".into()),
            ..empty_filter()
        });
        assert!(sql.contains("AND UPPER(symbol) = $2"), "{sql}");
        assert!(sql.trim_end().ends_with("LIMIT $3"), "{sql}");
    }

    #[test]
    fn filtered_sql_playbook_untagged_is_null_without_placeholder() {
        let sql = build_filtered_sql(&JournalFilter {
            playbook_id: Some(None),
            ..empty_filter()
        });
        assert!(sql.contains("AND playbook_id IS NULL"), "{sql}");
        assert!(!sql.contains("playbook_id ="), "{sql}");
        assert!(sql.trim_end().ends_with("LIMIT $2"), "{sql}");
    }

    #[test]
    fn filtered_sql_playbook_specific_binds_placeholder() {
        let sql = build_filtered_sql(&JournalFilter {
            playbook_id: Some(Some("pb".into())),
            ..empty_filter()
        });
        assert!(sql.contains("AND playbook_id = $2"), "{sql}");
        assert!(sql.trim_end().ends_with("LIMIT $3"), "{sql}");
    }

    #[test]
    fn filtered_sql_has_stop_loss_is_literal_without_placeholder() {
        let with = build_filtered_sql(&JournalFilter {
            has_stop_loss: Some(true),
            ..empty_filter()
        });
        assert!(with.contains("AND stop_loss IS NOT NULL"), "{with}");
        assert!(with.trim_end().ends_with("LIMIT $2"), "{with}");

        let without = build_filtered_sql(&JournalFilter {
            has_stop_loss: Some(false),
            ..empty_filter()
        });
        assert!(without.contains("AND stop_loss IS NULL"), "{without}");
        assert!(without.trim_end().ends_with("LIMIT $2"), "{without}");
    }

    #[test]
    fn filtered_sql_status_and_pl_bounds() {
        let sql = build_filtered_sql(&JournalFilter {
            status: Some(TradeStatus::Loss),
            min_pl_pct: Some(-100.0),
            max_pl_pct: Some(-30.0),
            ..empty_filter()
        });
        assert!(sql.contains("AND status = $2"), "{sql}");
        assert!(sql.contains("AND total_pl >= $3"), "{sql}");
        assert!(sql.contains("AND total_pl <= $4"), "{sql}");
        assert!(sql.trim_end().ends_with("LIMIT $5"), "{sql}");
    }

    #[test]
    fn filtered_sql_mistake_contains_uses_ilike_escape() {
        let sql = build_filtered_sql(&JournalFilter {
            mistake_contains: Some("30-min".into()),
            ..empty_filter()
        });
        assert!(sql.contains("AND mistakes ILIKE $2 ESCAPE '\\'"), "{sql}");
        assert!(sql.trim_end().ends_with("LIMIT $3"), "{sql}");
    }

    #[test]
    fn filtered_sql_no_placeholder_predicates_dont_shift_numbering() {
        // account($2) + untagged-playbook(no ph) + status($3) + has_stop_loss(no ph)
        // + mistake($4) -> LIMIT $5.
        let sql = build_filtered_sql(&JournalFilter {
            account_id: Some("acct".into()),
            playbook_id: Some(None),
            status: Some(TradeStatus::Profit),
            has_stop_loss: Some(true),
            mistake_contains: Some("rule".into()),
            ..empty_filter()
        });
        assert!(sql.contains("AND account_id = $2"), "{sql}");
        assert!(sql.contains("AND playbook_id IS NULL"), "{sql}");
        assert!(sql.contains("AND status = $3"), "{sql}");
        assert!(sql.contains("AND stop_loss IS NOT NULL"), "{sql}");
        assert!(sql.contains("AND mistakes ILIKE $4 ESCAPE '\\'"), "{sql}");
        assert!(sql.trim_end().ends_with("LIMIT $5"), "{sql}");
    }

    #[test]
    fn filtered_sql_all_placeholdered_filters_number_in_order() {
        let sql = build_filtered_sql(&JournalFilter {
            account_id: Some("acct".into()),
            symbol: Some("AAPL".into()),
            playbook_id: Some(Some("pb".into())),
            status: Some(TradeStatus::Profit),
            min_pl_pct: Some(0.0),
            max_pl_pct: Some(100.0),
            has_stop_loss: None,
            mistake_contains: Some("x".into()),
            date_from: Some("2025-01-01".into()),
            date_to: Some("2025-12-31".into()),
            after: None,
            limit: Some(10),
        });
        assert!(sql.contains("AND account_id = $2"), "{sql}");
        assert!(sql.contains("AND UPPER(symbol) = $3"), "{sql}");
        assert!(sql.contains("AND playbook_id = $4"), "{sql}");
        assert!(sql.contains("AND status = $5"), "{sql}");
        assert!(sql.contains("AND total_pl >= $6"), "{sql}");
        assert!(sql.contains("AND total_pl <= $7"), "{sql}");
        assert!(sql.contains("AND mistakes ILIKE $8 ESCAPE '\\'"), "{sql}");
        assert!(sql.contains("'YYYY-MM-DD') >= $9"), "{sql}");
        assert!(sql.contains("'YYYY-MM-DD') <= $10"), "{sql}");
        assert!(sql.trim_end().ends_with("LIMIT $11"), "{sql}");
    }

    #[test]
    fn filtered_sql_after_cursor_adds_keyset_tuple() {
        let sql = build_filtered_sql(&JournalFilter {
            after: Some(("2025-01-01T00:00:00.000000Z".into(), "id1".into())),
            ..empty_filter()
        });
        assert!(
            sql.contains("AND (created_at, id) < ($2::timestamptz, $3::text)"),
            "{sql}"
        );
        assert!(sql.contains("ORDER BY created_at DESC, id DESC"), "{sql}");
        assert!(sql.trim_end().ends_with("LIMIT $4"), "{sql}");
    }

    #[test]
    fn like_contains_escapes_wildcards() {
        assert_eq!(like_contains("30%_x"), "%30\\%\\_x%");
    }
}
