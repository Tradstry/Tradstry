use anyhow::{Context, Result, anyhow, ensure};
use async_graphql::{InputObject, SimpleObject};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use finance_query::Ticker;
use libsql::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub stop_loss: f64,
    pub risk_reward: f64,
    pub trade_type: String,
    pub mistakes: String,
    pub entry_tactics: String,
    pub edges_spotted: String,
    pub playbook_id: Option<String>,
    pub notes: Option<String>,
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
    pub brokerage_transaction_ids: Option<Vec<String>>,
    /// Tag ids to attach to this trade. Persisted separately via
    /// `tags_table::set_trade_tags`; ignored by the journal_entries writer.
    #[graphql(default)]
    pub tag_ids: Vec<String>,
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
    /// Tag ids to attach to this trade. Persisted separately via
    /// `tags_table::set_trade_tags`; ignored by the journal_entries writer.
    /// `None` (field omitted) leaves the trade's tags untouched; `Some([])`
    /// explicitly clears all tags.
    pub tag_ids: Option<Vec<String>>,
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
    stop_loss: f64,
    risk_reward: f64,
    trade_type: String,
    mistakes: String,
    entry_tactics: String,
    edges_spotted: String,
    playbook_id: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, Clone)]
struct DerivedMetrics {
    status: String,
    total_pl: f64,
    net_roi: f64,
    duration: i64,
    risk_reward: f64,
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

#[derive(Debug, Clone, Copy)]
pub enum ExtremeKind {
    Best,
    Worst,
}

const SELECT_COLS: &str = "id, user_id, account_id, open_date, close_date, entry_price, exit_price, position_size, symbol, symbol_name, status, total_pl, net_roi, duration, stop_loss, risk_reward, trade_type, mistakes, entry_tactics, edges_spotted, playbook_id, notes";

const DOLLAR_PL_EXPR: &str = "position_size * entry_price * total_pl / 100.0";

fn nullable_text(row: &libsql::Row, index: i32) -> Option<String> {
    row.get::<libsql::Value>(index)
        .ok()
        .and_then(|value| match value {
            libsql::Value::Text(text) => Some(text),
            _ => None,
        })
}

fn row_to_journal_entry(row: &libsql::Row) -> Result<JournalEntry> {
    Ok(JournalEntry {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        account_id: row.get::<String>(2)?,
        open_date: row.get::<String>(3)?,
        close_date: row.get::<String>(4)?,
        entry_price: row.get::<f64>(5)?,
        exit_price: row.get::<f64>(6)?,
        position_size: row.get::<f64>(7)?,
        symbol: row.get::<String>(8)?,
        symbol_name: row.get::<String>(9)?,
        status: row.get::<String>(10)?,
        total_pl: row.get::<f64>(11)?,
        net_roi: row.get::<f64>(12)?,
        duration: row.get::<i64>(13)?,
        stop_loss: row.get::<f64>(14)?,
        risk_reward: row.get::<f64>(15)?,
        trade_type: row.get::<String>(16)?,
        mistakes: row.get::<String>(17)?,
        entry_tactics: row.get::<String>(18)?,
        edges_spotted: row.get::<String>(19)?,
        playbook_id: nullable_text(row, 20),
        notes: nullable_text(row, 21),
    })
}

fn parse_trade_datetime(value: &str) -> Result<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Ok(parsed.with_timezone(&Utc));
    }

    for format in [
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(value, format) {
            return Ok(DateTime::<Utc>::from_naive_utc_and_offset(parsed, Utc));
        }
    }

    if let Ok(parsed) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let midnight = parsed
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow!("Invalid date provided"))?;
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(midnight, Utc));
    }

    Err(anyhow!(
        "Invalid datetime format. Use RFC3339, YYYY-MM-DD HH:MM[:SS], or YYYY-MM-DD"
    ))
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
    stop_loss: f64,
    trade_type: &str,
) -> Result<DerivedMetrics> {
    let open = parse_trade_datetime(open_date)?;
    let close = parse_trade_datetime(close_date)?;
    ensure!(close >= open, "close_date must be on or after open_date");

    let pl_ratio = match trade_type {
        "long" => (exit_price - entry_price) / entry_price,
        "short" => (entry_price - exit_price) / entry_price,
        _ => return Err(anyhow!("Unsupported trade_type")),
    };

    // stop_loss == 0 means the trade was taken with no stop. It's recorded as-is
    // (risk_reward 0); R-based analytics already skip trades without a stop.
    let risk_reward = if stop_loss == 0.0 {
        0.0
    } else {
        let risk_distance = match trade_type {
            "long" => entry_price - stop_loss,
            "short" => stop_loss - entry_price,
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
        reward_distance / risk_distance
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
    let stop_loss = input.stop_loss.unwrap_or(current.stop_loss);
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
        brokerage_transaction_ids: None,
        tag_ids: Vec::new(),
    })
    .await
}

async fn validate_playbook_exists(
    conn: &Connection,
    user_id: &str,
    playbook_id: Option<String>,
) -> Result<Option<String>> {
    match playbook_id {
        Some(playbook_id) => {
            let mut rows = conn
                .query(
                    "SELECT 1 FROM playbooks WHERE id = ?1 AND user_id = ?2 LIMIT 1",
                    libsql::params![playbook_id.as_str(), user_id],
                )
                .await
                .context("Failed to validate playbook reference")?;

            ensure!(
                rows.next().await?.is_some(),
                "playbook '{playbook_id}' was not found"
            );
            Ok(Some(playbook_id))
        }
        None => Ok(None),
    }
}

pub async fn list_journal_entries(conn: &Connection, user_id: &str) -> Result<Vec<JournalEntry>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {SELECT_COLS} FROM journal_entries WHERE user_id = ?1 ORDER BY open_date DESC, close_date DESC"
            ),
            libsql::params![user_id],
        )
        .await
        .context("Failed to list journal entries")?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await? {
        entries.push(row_to_journal_entry(&row)?);
    }
    Ok(entries)
}

pub async fn aggregate_journal_analytics(
    conn: &Connection,
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
            COALESCE(SUM(CASE WHEN total_pl < 0 THEN ABS(total_pl) ELSE 0.0 END), 0.0) AS sum_loss_pct
        FROM journal_entries
        WHERE user_id = ?1
          AND account_id = ?2
          AND datetime(close_date) >= datetime(?3)
          AND datetime(close_date) <= datetime(?4)
    "
    );

    let mut rows = conn
        .query(
            &sql,
            libsql::params![user_id, account_id, start_iso, end_iso],
        )
        .await
        .context("Failed to aggregate journal analytics")?;

    let row = rows
        .next()
        .await?
        .ok_or_else(|| anyhow!("Aggregate query returned no rows"))?;

    Ok(JournalAggregateRow {
        total_trades: row.get::<i64>(0)?,
        winning_trades: row.get::<i64>(1)?,
        losing_trades: row.get::<i64>(2)?,
        cumulative_profit: row.get::<f64>(3)?,
        gross_profit: row.get::<f64>(4)?,
        gross_loss: row.get::<f64>(5)?,
        sum_risk_reward: row.get::<f64>(6)?,
        sum_win_pct: row.get::<f64>(7)?,
        sum_loss_pct: row.get::<f64>(8)?,
    })
}

/// Returns `None` if no profitable (Best) or losing (Worst) trade exists in the window.
pub async fn find_extreme_trade(
    conn: &Connection,
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
         WHERE user_id = ?1
           AND account_id = ?2
           AND datetime(close_date) >= datetime(?3)
           AND datetime(close_date) <= datetime(?4)
           AND total_pl {sign_filter}
         ORDER BY amount {order}
         LIMIT 1"
    );

    let mut rows = conn
        .query(
            &sql,
            libsql::params![user_id, account_id, start_iso, end_iso],
        )
        .await
        .context("Failed to query extreme trade")?;

    match rows.next().await? {
        Some(row) => Ok(Some(TradeOutcomeRow {
            symbol: row.get::<String>(0)?,
            symbol_name: row.get::<String>(1)?,
            amount: row.get::<f64>(2)?,
        })),
        None => Ok(None),
    }
}

pub async fn aggregate_calendar_days(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
    month_start_iso: &str,
    month_end_iso: &str,
) -> Result<Vec<CalendarDayAggregateRow>> {
    let sql = format!(
        "
        SELECT
            date(close_date) AS day,
            COALESCE(SUM({DOLLAR_PL_EXPR}), 0.0) AS profit,
            COUNT(*) AS trade_count,
            COALESCE(SUM(CASE WHEN total_pl > 0 THEN 1 ELSE 0 END), 0) AS winning_trade_count
        FROM journal_entries
        WHERE user_id = ?1
          AND account_id = ?2
          AND date(close_date) >= date(?3)
          AND date(close_date) <= date(?4)
        GROUP BY date(close_date)
    "
    );

    let mut rows = conn
        .query(
            &sql,
            libsql::params![user_id, account_id, month_start_iso, month_end_iso],
        )
        .await
        .context("Failed to aggregate calendar days")?;

    let mut days = Vec::new();
    while let Some(row) = rows.next().await? {
        days.push(CalendarDayAggregateRow {
            date: row.get::<String>(0)?,
            profit: row.get::<f64>(1)?,
            trade_count: row.get::<i64>(2)?,
            winning_trade_count: row.get::<i64>(3)?,
        });
    }
    Ok(days)
}

pub async fn find_journal_entry(
    conn: &Connection,
    id: &str,
    user_id: &str,
) -> Result<Option<JournalEntry>> {
    let mut rows = conn
        .query(
            &format!("SELECT {SELECT_COLS} FROM journal_entries WHERE id = ?1 AND user_id = ?2"),
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to find journal entry")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_journal_entry(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_journal_entry(
    conn: &Connection,
    user_id: &str,
    input: CreateJournalEntryInput,
) -> Result<JournalEntry> {
    let id = Uuid::new_v4().to_string();
    let brokerage_tx_ids = input.brokerage_transaction_ids.clone();
    let mut entry = prepare_new_entry(input).await?;
    entry.playbook_id = validate_playbook_exists(conn, user_id, entry.playbook_id).await?;

    conn.execute(
        "INSERT INTO journal_entries (id, user_id, account_id, open_date, close_date, entry_price, exit_price, position_size, symbol, symbol_name, status, total_pl, net_roi, duration, stop_loss, risk_reward, trade_type, mistakes, entry_tactics, edges_spotted, playbook_id, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
        libsql::params![
            id.as_str(),
            user_id,
            entry.account_id.as_str(),
            entry.open_date.as_str(),
            entry.close_date.as_str(),
            entry.entry_price,
            entry.exit_price,
            entry.position_size,
            entry.symbol.as_str(),
            entry.symbol_name.as_str(),
            entry.status.as_str(),
            entry.total_pl,
            entry.net_roi,
            entry.duration,
            entry.stop_loss,
            entry.risk_reward,
            entry.trade_type.as_str(),
            entry.mistakes.as_str(),
            entry.entry_tactics.as_str(),
            entry.edges_spotted.as_str(),
            entry.playbook_id.as_deref(),
            entry.notes.as_deref(),
        ],
    )
    .await
    .context("Failed to insert journal entry")?;

    if let Some(ref tx_ids) = brokerage_tx_ids
        && !tx_ids.is_empty()
    {
        insert_brokerage_links(conn, &id, user_id, tx_ids).await?;
    }

    find_journal_entry(conn, &id, user_id)
        .await?
        .context("Journal entry not found after insert")
}

pub async fn update_journal_entry(
    conn: &Connection,
    id: &str,
    user_id: &str,
    input: UpdateJournalEntryInput,
) -> Result<JournalEntry> {
    let current = find_journal_entry(conn, id, user_id)
        .await?
        .context("Journal entry not found")?;
    let entry = prepare_updated_entry(&current, input).await?;
    let entry = PreparedJournalEntry {
        playbook_id: validate_playbook_exists(conn, user_id, entry.playbook_id).await?,
        ..entry
    };

    conn.execute(
        // Legacy freeform columns (mistakes/entry_tactics/edges_spotted) are
        // intentionally omitted from the UPDATE set so they stay frozen.
        "UPDATE journal_entries SET account_id = ?1, open_date = ?2, close_date = ?3, entry_price = ?4, exit_price = ?5, position_size = ?6, symbol = ?7, symbol_name = ?8, status = ?9, total_pl = ?10, net_roi = ?11, duration = ?12, stop_loss = ?13, risk_reward = ?14, trade_type = ?15, playbook_id = ?16, notes = ?17 WHERE id = ?18 AND user_id = ?19",
        libsql::params![
            entry.account_id.as_str(),
            entry.open_date.as_str(),
            entry.close_date.as_str(),
            entry.entry_price,
            entry.exit_price,
            entry.position_size,
            entry.symbol.as_str(),
            entry.symbol_name.as_str(),
            entry.status.as_str(),
            entry.total_pl,
            entry.net_roi,
            entry.duration,
            entry.stop_loss,
            entry.risk_reward,
            entry.trade_type.as_str(),
            entry.playbook_id.as_deref(),
            entry.notes.as_deref(),
            id,
            user_id,
        ],
    )
    .await
    .context("Failed to update journal entry")?;

    find_journal_entry(conn, id, user_id)
        .await?
        .context("Journal entry not found after update")
}

pub async fn delete_journal_entry(conn: &Connection, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM journal_entries WHERE id = ?1 AND user_id = ?2",
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to delete journal entry")?;

    Ok(rows_affected > 0)
}

/// Insert links between a journal entry and brokerage transactions.
pub async fn insert_brokerage_links(
    conn: &Connection,
    journal_entry_id: &str,
    user_id: &str,
    brokerage_transaction_ids: &[String],
) -> Result<()> {
    for tx_id in brokerage_transaction_ids {
        let link_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO journal_brokerage_links (id, journal_entry_id, brokerage_transaction_id, user_id) VALUES (?1, ?2, ?3, ?4)",
            libsql::params![link_id.as_str(), journal_entry_id, tx_id.as_str(), user_id],
        )
        .await
        .context(format!("Failed to insert brokerage link for transaction {}", tx_id))?;
    }
    Ok(())
}

/// Get all brokerage transaction IDs that are already linked to journal entries for a user+account.
pub async fn list_linked_brokerage_transaction_ids(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<String>> {
    let mut rows = conn
        .query(
            "SELECT jbl.brokerage_transaction_id FROM journal_brokerage_links jbl
             INNER JOIN journal_entries je ON je.id = jbl.journal_entry_id
             WHERE jbl.user_id = ?1 AND je.account_id = ?2",
            libsql::params![user_id, account_id],
        )
        .await
        .context("Failed to query linked brokerage transaction IDs")?;

    let mut ids = Vec::new();
    while let Some(row) = rows.next().await? {
        let id: String = row.get(0)?;
        ids.push(id);
    }
    Ok(ids)
}

/// Load all journal entries for `account_id` whose `close_date` falls within
/// [`start_iso`, `end_iso`] (inclusive), using the same `datetime()` comparison
/// convention as `aggregate_journal_analytics`. Results are ordered by
/// `close_date ASC` so callers can use them directly for equity-curve work.
pub async fn list_journal_entries_for_account_in_range(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
    start_iso: &str,
    end_iso: &str,
) -> Result<Vec<JournalEntry>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM journal_entries
         WHERE user_id = ?1
           AND account_id = ?2
           AND datetime(close_date) >= datetime(?3)
           AND datetime(close_date) <= datetime(?4)
         ORDER BY close_date ASC"
    );
    let mut rows = conn
        .query(
            &sql,
            libsql::params![user_id, account_id, start_iso, end_iso],
        )
        .await
        .context("Failed to list journal entries for account in range")?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await? {
        entries.push(row_to_journal_entry(&row)?);
    }
    Ok(entries)
}

pub async fn aggregate_stats_per_playbook(
    conn: &Connection,
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
         WHERE user_id = ?1
           AND playbook_id IS NOT NULL
         GROUP BY playbook_id"
    );

    let mut rows = conn
        .query(&sql, libsql::params![user_id])
        .await
        .context("Failed to aggregate stats per playbook")?;

    let mut stats = Vec::new();
    while let Some(row) = rows.next().await? {
        stats.push(PlaybookStatsRow {
            playbook_id: row.get::<String>(0)?,
            total_trades: row.get::<i64>(1)?,
            winning_trades: row.get::<i64>(2)?,
            losing_trades: row.get::<i64>(3)?,
            cumulative_profit: row.get::<f64>(4)?,
            gross_profit: row.get::<f64>(5)?,
            gross_loss: row.get::<f64>(6)?,
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
            calculate_derived_metrics("2026-01-01", "2026-01-03", 100.0, 110.0, 95.0, "long")
                .unwrap();

        assert_eq!(metrics.status, "profit");
        assert_eq!(metrics.total_pl, 10.0);
        assert_eq!(metrics.net_roi, 10.0);
        assert_eq!(metrics.duration, 172800);
        assert_eq!(metrics.risk_reward, 2.0);
    }

    #[test]
    fn calculates_metrics_for_short_trade() {
        let metrics = calculate_derived_metrics(
            "2026-01-01T09:00:00Z",
            "2026-01-01T11:30:00Z",
            100.0,
            90.0,
            105.0,
            "short",
        )
        .unwrap();

        assert_eq!(metrics.status, "profit");
        assert_eq!(metrics.total_pl, 10.0);
        assert_eq!(metrics.net_roi, 10.0);
        assert_eq!(metrics.duration, 9000);
        assert_eq!(metrics.risk_reward, 2.0);
    }

    #[test]
    fn rejects_invalid_stop_loss_position() {
        let error =
            calculate_derived_metrics("2026-01-01", "2026-01-02", 100.0, 102.0, 101.0, "long")
                .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("stop_loss must be below entry_price")
        );
    }
}
