use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};

use crate::service::db::schema::tables::journal_table::JournalEntry;
use crate::service::db::schema::tables::tags_table::{TagRole, TradeTag};

#[derive(Debug, Clone, Serialize, Default, async_graphql::SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct AdvancedAnalytics {
    pub trade_count: usize,
    pub net_profit: f64,
    pub win_rate: f64, // 0..100
    pub expectancy_dollars: f64,
    pub expectancy_r: Option<f64>,
    pub r_trade_count: usize, // trades with a valid initial risk
    pub profit_factor: Option<f64>,
    pub sqn: Option<f64>,
    /// Average realized $ on winning trades (sum winner $ / number of winners).
    pub average_gain: f64,
    /// Average realized $ on losing trades, as a positive magnitude.
    pub average_loss: f64,
    /// Average percent return on winning trades.
    pub average_gain_pct: f64,
    /// Average percent loss on losing trades, as a positive magnitude.
    pub average_loss_pct: f64,
    // risk/drawdown (Task 2)
    pub max_drawdown_dollars: f64,
    pub max_drawdown_pct: f64,
    pub current_drawdown_dollars: f64,
    pub recovery_factor: Option<f64>,
    pub longest_drawdown_days: i64,
    pub equity_curve: Vec<EquityPoint>,
    /// Account equity at the start of the loaded window (`account_equity -
    /// net_profit`). Set only when real account equity was supplied; the
    /// `equity_curve` itself stays in cumulative-PnL terms.
    pub starting_equity: Option<f64>,
    /// The account's current total equity passed in (SnapTrade `total_value`).
    /// None for manual accounts with no synced equity (drawdown % then falls
    /// back to the peak-cumulative-PnL denominator).
    pub account_equity: Option<f64>,
    pub avg_planned_r: Option<f64>,
    pub avg_actual_r: Option<f64>,
    // distributions & consistency (Task 3)
    pub r_distribution: Vec<RBucket>,
    pub longest_win_streak: usize,
    pub longest_loss_streak: usize,
    pub current_streak: i32, // +k consecutive winners / -k losers / 0 if last breakeven or empty
    pub avg_hold_winners_secs: Option<f64>,
    pub avg_hold_losers_secs: Option<f64>,
    pub monthly_win_rate_stdev: Option<f64>,
    // breakdown dimensions (Task 4)
    pub by_symbol: Vec<DimensionStat>,
    pub by_day_of_week: Vec<DimensionStat>,
    pub by_session: Vec<DimensionStat>,
    pub by_holding: Vec<DimensionStat>,
    pub by_direction: Vec<DimensionStat>,
    pub by_position_size: Vec<DimensionStat>,
    pub by_playbook: Vec<DimensionStat>,
    // behavioral / process (Task 5)
    pub clean_vs_flawed: CleanFlawed,
    /// Discipline / process-adherence block: mistake cost plus the
    /// self-reported behavioral fields and principle-violation tallies.
    pub discipline: Discipline,
    /// Per-category tag expectancy. One entry per distinct tag category present
    /// across the trades' tags (seeded tactic/edge categories + any custom
    /// category). Replaces the old freeform tactic/edge expectancy.
    pub tag_breakdowns: Vec<CategoryBreakdown>,
    pub trades_per_day: TradesPerDay,
    /// Resolved ET calendar start/end of the active range (`YYYY-MM-DD`).
    /// `None` for the unbounded `All` range. Set by `get_advanced_analytics`.
    pub range_start: Option<String>,
    pub range_end: Option<String>,
}

/// Per-tag expectancy grouped within a single tag category. `role` is the
/// category's analytic role (`mistake`/`tactic`/`edge`) when it is a seeded
/// category, else None for user-defined categories.
#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct CategoryBreakdown {
    pub category_name: String,
    pub role: Option<String>,
    pub tags: Vec<DimensionStat>,
}

/// Clean (no mistake-role tag) vs flawed (>=1 mistake-role tag) core metrics,
/// to expose the $ cost of mistakes (compare net_profit / expectancy).
#[derive(Debug, Clone, Serialize, Default, async_graphql::SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct CleanFlawed {
    pub clean: GroupMetrics,
    pub flawed: GroupMetrics,
}

/// Discipline / process-adherence metrics: the $ cost of flawed trades plus
/// the self-reported behavioral fields and principle-violation tallies.
#[derive(Debug, Clone, Serialize, Deserialize, async_graphql::SimpleObject, Default)]
#[graphql(rename_fields = "camelCase")]
pub struct Discipline {
    pub flawed_trade_count: usize,
    /// `clean.expectancy_dollars * flawed_trade_count - flawed.net_profit`: the
    /// $ the flawed trades cost relative to performing at the clean-trade
    /// average. Zero when there are no flawed trades.
    pub mistake_cost: f64,
    pub avg_rule_adherence: Option<f64>,
    pub avg_conviction: Option<f64>,
    pub revenge_trade_count: usize,
    pub broke30_min_count: usize,
    pub trades_with_violations: usize,
    pub total_violations: usize,
}

/// Trades-per-active-day distribution. An overtrading signal.
#[derive(Debug, Clone, Serialize, Default, async_graphql::SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct TradesPerDay {
    pub avg: f64,
    pub max: usize,
    pub stdev: Option<f64>,
}

/// Six core metrics shared by the overall analytics and every breakdown group.
#[derive(Debug, Clone, Serialize, Default, async_graphql::SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct GroupMetrics {
    pub trade_count: usize,
    pub net_profit: f64,
    pub win_rate: f64, // 0..100
    pub expectancy_dollars: f64,
    pub expectancy_r: Option<f64>,
    pub profit_factor: Option<f64>,
}

/// One group of a breakdown dimension: its key plus the shared core metrics.
#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct DimensionStat {
    pub key: String,
    pub metrics: GroupMetrics,
}

#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct EquityPoint {
    pub close_date: String,
    pub equity: f64,
}

#[derive(Debug, Clone, Serialize, async_graphql::SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct RBucket {
    pub label: String,
    pub count: usize,
}

/// Realized dollar P/L for a trade. `total_pl` is stored as a percentage return
/// (`(exit-entry)/entry * 100`), so the dollar value is the trade's notional
/// times that fractional return. Mirrors the journal/dashboard SQL
/// (`position_size * entry_price * total_pl / 100`) so the analytics page and
/// the dashboard report the same dollars.
fn dollar_pl(e: &JournalEntry) -> f64 {
    e.position_size * e.entry_price * e.total_pl / 100.0
}

/// Per-trade R-multiple: realized dollar P/L over initial dollar risk. None when
/// initial dollar risk is not determinable (stop_loss is zero/unset, or the
/// computed risk is zero or negative).
fn r_multiple(e: &JournalEntry) -> Option<f64> {
    let stop = e.stop_loss?;
    let risk = (e.entry_price - stop).abs() * e.position_size;
    if risk > 0.0 {
        Some(dollar_pl(e) / risk)
    } else {
        None
    }
}

/// Mean of an `i32` iterator as `f64`, or `None` when the iterator is empty.
/// Used for the discipline block's self-reported score averages
/// (`rule_adherence_score`, `pre_trade_conviction`), which are `Option<i32>`
/// on the entry and should be averaged only over the trades where present.
fn mean_option<I: Iterator<Item = i32>>(iter: I) -> Option<f64> {
    let (sum, count) = iter.fold((0i64, 0usize), |(sum, count), v| {
        (sum + v as i64, count + 1)
    });
    if count == 0 {
        None
    } else {
        Some(sum as f64 / count as f64)
    }
}

pub fn compute_advanced_analytics(
    entries: &[JournalEntry],
    current_equity: Option<f64>,
    trade_tags: &HashMap<String, Vec<TradeTag>>,
    violation_counts: &HashMap<String, usize>,
) -> AdvancedAnalytics {
    let n = entries.len();
    if n == 0 {
        return AdvancedAnalytics::default();
    }

    let net_profit: f64 = entries.iter().map(dollar_pl).sum();

    // Average gain / loss in dollars and percent. Denominators are winners-only
    // and losers-only (breakeven excluded); loss is a positive magnitude.
    let winners: Vec<&JournalEntry> = entries.iter().filter(|e| e.total_pl > 0.0).collect();
    let losers: Vec<&JournalEntry> = entries.iter().filter(|e| e.total_pl < 0.0).collect();
    let mean_or_zero = |vals: &[f64]| -> f64 {
        if vals.is_empty() {
            0.0
        } else {
            vals.iter().sum::<f64>() / vals.len() as f64
        }
    };
    let average_gain = mean_or_zero(&winners.iter().map(|e| dollar_pl(e)).collect::<Vec<_>>());
    let average_loss = mean_or_zero(
        &losers
            .iter()
            .map(|e| dollar_pl(e).abs())
            .collect::<Vec<_>>(),
    );
    let average_gain_pct = mean_or_zero(&winners.iter().map(|e| e.total_pl).collect::<Vec<_>>());
    let average_loss_pct =
        mean_or_zero(&losers.iter().map(|e| e.total_pl.abs()).collect::<Vec<_>>());

    // Six core metrics computed by the single shared implementation.
    let refs: Vec<&JournalEntry> = entries.iter().collect();
    let core = core_metrics(&refs);
    let win_rate = core.win_rate;
    let expectancy_dollars = core.expectancy_dollars;
    let expectancy_r = core.expectancy_r;
    let profit_factor = core.profit_factor;

    let rs: Vec<f64> = entries.iter().filter_map(r_multiple).collect();
    let r_trade_count = rs.len();
    let sqn = sqn(&rs);

    // Planned-vs-actual R over R-valid trades only.
    let mut planned: Vec<f64> = Vec::new();
    let mut actual: Vec<f64> = Vec::new();
    for e in entries.iter() {
        if let (Some(r), Some(planned_r)) = (r_multiple(e), e.risk_reward) {
            planned.push(planned_r);
            actual.push(r);
        }
    }
    let avg_planned_r = if planned.is_empty() {
        None
    } else {
        Some(planned.iter().sum::<f64>() / planned.len() as f64)
    };
    let avg_actual_r = if actual.is_empty() {
        None
    } else {
        Some(actual.iter().sum::<f64>() / actual.len() as f64)
    };

    // Equity curve + drawdown. Sort a clone by close_date ASC (ISO string sort).
    let mut sorted: Vec<JournalEntry> = entries.to_vec();
    sorted.sort_by(|a, b| a.close_date.cmp(&b.close_date));

    let mut equity_curve: Vec<EquityPoint> = Vec::with_capacity(sorted.len());
    let mut equity = 0.0_f64;
    let mut peak = 0.0_f64;
    let mut max_drawdown_dollars = 0.0_f64;
    let mut peak_at_trough = 0.0_f64; // running peak captured at the deepest trough
    for e in sorted.iter() {
        equity += dollar_pl(e);
        if equity > peak {
            peak = equity;
        }
        let dd = peak - equity;
        if dd > max_drawdown_dollars {
            max_drawdown_dollars = dd;
            peak_at_trough = peak;
        }
        equity_curve.push(EquityPoint {
            close_date: e.close_date.clone(),
            equity,
        });
    }

    // `peak_at_trough` is the peak of the cumulative-PnL curve captured at the
    // deepest trough. Shifting the curve by a constant `starting_equity` does
    // not change `max_drawdown_dollars`; only the % denominator changes.
    let starting_equity = current_equity.map(|equity| equity - net_profit);
    let max_drawdown_pct = if max_drawdown_dollars <= 0.0 {
        0.0
    } else if let Some(start) = starting_equity {
        // Real-equity basis: denominator is the account equity at the peak that
        // precedes the deepest trough (`starting_equity + peak_cumulative_pnl`).
        let peak_account_equity = start + peak_at_trough;
        if peak_account_equity > 0.0 {
            max_drawdown_dollars / peak_account_equity * 100.0
        } else {
            0.0
        }
    } else if peak_at_trough > 0.0 {
        // Fallback (no synced equity): peak-cumulative-PnL denominator.
        max_drawdown_dollars / peak_at_trough * 100.0
    } else {
        0.0
    };
    let current_drawdown_dollars = peak - equity; // final_peak - final_equity
    let recovery_factor = if max_drawdown_dollars > 0.0 {
        Some(net_profit / max_drawdown_dollars)
    } else {
        None
    };
    let longest_drawdown_days = longest_drawdown_days(&sorted);

    // R-distribution over R-valid trades (reuse `rs`). Always 5 buckets in order.
    let r_distribution = r_distribution(&rs);

    // Streaks over entries ordered by close_date ASC (reuse `sorted`).
    let (longest_win_streak, longest_loss_streak, current_streak) = streaks(&sorted);

    // Holding time (seconds) split by winners/losers, over `sorted`.
    let mut hold_winners: Vec<f64> = Vec::new();
    let mut hold_losers: Vec<f64> = Vec::new();
    for e in sorted.iter() {
        if let Some(secs) = holding_secs(e) {
            if e.total_pl > 0.0 {
                hold_winners.push(secs);
            } else if e.total_pl < 0.0 {
                hold_losers.push(secs);
            }
        }
    }
    let avg_hold_winners_secs = mean(&hold_winners);
    let avg_hold_losers_secs = mean(&hold_losers);

    // Monthly win-rate consistency: sample stdev of per-month win rates (0..100).
    let monthly_win_rate_stdev = monthly_win_rate_stdev(&sorted);

    // Breakdown dimensions (Task 4).
    let by_symbol = breakdown(entries, |e| Some(e.symbol.clone()));
    let by_day_of_week = breakdown(entries, |e| Some(day_of_week_key(&e.close_date)));
    let by_session = breakdown(entries, |e| Some(session_key(&e.close_date)));
    let by_holding = breakdown(entries, |e| Some(holding_key(e)));
    let by_direction = breakdown(entries, |e| Some(e.trade_type.clone()));
    let by_position_size = breakdown(entries, |e| Some(position_size_key(e.position_size)));
    let by_playbook = breakdown(entries, |e| e.playbook_id.clone().or(Some("none".into())));

    // Behavioral / process (Task 5).
    // Clean vs flawed: a trade is "flawed" when it carries >=1 tag whose
    // category role is `Mistake`; otherwise it is "clean".
    let is_flawed = |e: &JournalEntry| -> bool {
        trade_tags
            .get(&e.id)
            .is_some_and(|tags| tags.iter().any(|t| t.role == Some(TagRole::Mistake)))
    };
    let flawed_refs: Vec<&JournalEntry> = entries.iter().filter(|e| is_flawed(e)).collect();
    let clean_refs: Vec<&JournalEntry> = entries.iter().filter(|e| !is_flawed(e)).collect();
    let clean = core_metrics(&clean_refs);
    let flawed = core_metrics(&flawed_refs);
    let flawed_trade_count = flawed_refs.len();
    let mistake_cost = if flawed_trade_count == 0 {
        0.0
    } else {
        clean.expectancy_dollars * flawed_trade_count as f64 - flawed.net_profit
    };
    let discipline = Discipline {
        flawed_trade_count,
        mistake_cost,
        avg_rule_adherence: mean_option(entries.iter().filter_map(|e| e.rule_adherence_score)),
        avg_conviction: mean_option(entries.iter().filter_map(|e| e.pre_trade_conviction)),
        revenge_trade_count: entries
            .iter()
            .filter(|e| e.revenge_trade == Some(true))
            .count(),
        broke30_min_count: entries
            .iter()
            .filter(|e| e.broke_30min_rule == Some(true))
            .count(),
        trades_with_violations: entries
            .iter()
            .filter(|e| violation_counts.get(&e.id).is_some_and(|&c| c > 0))
            .count(),
        total_violations: entries
            .iter()
            .filter_map(|e| violation_counts.get(&e.id))
            .sum(),
    };
    let clean_vs_flawed = CleanFlawed { clean, flawed };

    // Per-category tag expectancy: group trades by each tag within each tag
    // category present across `trade_tags`. The seeded tactic/edge categories
    // appear as breakdowns; custom categories appear automatically.
    let tag_breakdowns = tag_breakdowns(entries, trade_tags);

    let trades_per_day = trades_per_day(entries);

    AdvancedAnalytics {
        trade_count: n,
        net_profit,
        win_rate,
        expectancy_dollars,
        expectancy_r,
        r_trade_count,
        profit_factor,
        sqn,
        average_gain,
        average_loss,
        average_gain_pct,
        average_loss_pct,
        max_drawdown_dollars,
        max_drawdown_pct,
        current_drawdown_dollars,
        recovery_factor,
        longest_drawdown_days,
        equity_curve,
        starting_equity,
        account_equity: current_equity,
        avg_planned_r,
        avg_actual_r,
        r_distribution,
        longest_win_streak,
        longest_loss_streak,
        current_streak,
        avg_hold_winners_secs,
        avg_hold_losers_secs,
        monthly_win_rate_stdev,
        by_symbol,
        by_day_of_week,
        by_session,
        by_holding,
        by_direction,
        by_position_size,
        by_playbook,
        clean_vs_flawed,
        discipline,
        tag_breakdowns,
        trades_per_day,
        // Populated by get_advanced_analytics from the resolved range bounds.
        range_start: None,
        range_end: None,
    }
}

/// Per-category tag expectancy. For every distinct tag category present across
/// `trade_tags`, groups the trades by each tag within that category and builds
/// `core_metrics` per tag. A trade contributes to EACH of its tags. Categories
/// are ordered by name ascending; tags within a category by name ascending; the
/// `role` (when the category is a seeded mistake/tactic/edge) is carried
/// through. Trades absent from `trade_tags` (e.g. legacy untagged trades)
/// contribute to no tag metric.
fn tag_breakdowns(
    entries: &[JournalEntry],
    trade_tags: &HashMap<String, Vec<TradeTag>>,
) -> Vec<CategoryBreakdown> {
    use std::collections::BTreeMap;

    // category_id -> (category_name, role, tag_name -> Vec<&entry>)
    struct CatAcc<'a> {
        name: String,
        role: Option<String>,
        tags: BTreeMap<String, Vec<&'a JournalEntry>>,
    }
    let mut cats: BTreeMap<String, CatAcc> = BTreeMap::new();

    for e in entries.iter() {
        let Some(tags) = trade_tags.get(&e.id) else {
            continue;
        };
        for tt in tags.iter() {
            let acc = cats
                .entry(tt.category_id.clone())
                .or_insert_with(|| CatAcc {
                    name: tt.category_name.clone(),
                    role: tt.role.as_ref().map(|r| r.as_str().to_string()),
                    tags: BTreeMap::new(),
                });
            acc.tags.entry(tt.tag.name.clone()).or_default().push(e);
        }
    }

    // Order categories by name ascending for deterministic output.
    let mut breakdowns: Vec<CategoryBreakdown> = cats
        .into_values()
        .map(|acc| CategoryBreakdown {
            category_name: acc.name,
            role: acc.role,
            tags: acc
                .tags
                .into_iter()
                .map(|(key, group)| DimensionStat {
                    key,
                    metrics: core_metrics(&group),
                })
                .collect(),
        })
        .collect();
    breakdowns.sort_by(|a, b| a.category_name.cmp(&b.category_name));
    breakdowns
}

/// Trades-per-active-day stats grouped by the day (`close_date[..10]`): `avg` =
/// mean trades across active days, `max` = busiest day, `stdev` = sample stdev
/// of per-day counts (None when fewer than 2 active days). Empty input yields
/// the default (avg 0, max 0, stdev None).
fn trades_per_day(entries: &[JournalEntry]) -> TradesPerDay {
    use std::collections::BTreeMap;
    let mut by_day: BTreeMap<String, usize> = BTreeMap::new();
    for e in entries.iter() {
        let end = 10.min(e.close_date.len());
        let day = e.close_date[..end].to_string();
        *by_day.entry(day).or_insert(0) += 1;
    }
    if by_day.is_empty() {
        return TradesPerDay::default();
    }
    let counts: Vec<usize> = by_day.values().copied().collect();
    let avg = counts.iter().sum::<usize>() as f64 / counts.len() as f64;
    let max = counts.iter().copied().max().unwrap_or(0);
    let counts_f: Vec<f64> = counts.iter().map(|&c| c as f64).collect();
    let stdev = stdev(&counts_f);
    TradesPerDay { avg, max, stdev }
}

/// Compute the six shared core metrics over a set of entries. The single
/// implementation of these formulas; both `compute_advanced_analytics` and
/// `breakdown` call it. Returns `Default` (all-zero / None) for empty input.
fn core_metrics(entries: &[&JournalEntry]) -> GroupMetrics {
    let n = entries.len();
    if n == 0 {
        return GroupMetrics::default();
    }

    let net_profit: f64 = entries.iter().map(|e| dollar_pl(e)).sum();
    let winners: Vec<&&JournalEntry> = entries.iter().filter(|e| e.total_pl > 0.0).collect();
    let losers: Vec<&&JournalEntry> = entries.iter().filter(|e| e.total_pl < 0.0).collect();

    // Win rate excludes breakeven (scratch) trades from both sides — the
    // journal-software standard: wins / (wins + losses).
    let decisive = winners.len() + losers.len();
    let win_rate = if decisive == 0 {
        0.0
    } else {
        winners.len() as f64 / decisive as f64 * 100.0
    };

    let avg_win = if winners.is_empty() {
        0.0
    } else {
        winners.iter().map(|e| dollar_pl(e)).sum::<f64>() / winners.len() as f64
    };
    let avg_loss = if losers.is_empty() {
        0.0
    } else {
        losers.iter().map(|e| dollar_pl(e).abs()).sum::<f64>() / losers.len() as f64
    };
    // Expectancy is average $ realized per trade over ALL trades (breakeven
    // included as zero) = net_profit / n. Expressed via the win/loss fractions
    // over n so it stays equal to net_profit / n.
    let expectancy_dollars =
        (winners.len() as f64 / n as f64) * avg_win - (losers.len() as f64 / n as f64) * avg_loss;

    let gross_profit: f64 = winners.iter().map(|e| dollar_pl(e)).sum();
    let gross_loss: f64 = losers.iter().map(|e| dollar_pl(e).abs()).sum();
    let profit_factor = if gross_loss > 0.0 {
        Some(gross_profit / gross_loss)
    } else {
        None
    };

    let rs: Vec<f64> = entries.iter().filter_map(|e| r_multiple(e)).collect();
    let expectancy_r = if rs.is_empty() {
        None
    } else {
        Some(rs.iter().sum::<f64>() / rs.len() as f64)
    };

    GroupMetrics {
        trade_count: n,
        net_profit,
        win_rate,
        expectancy_dollars,
        expectancy_r,
        profit_factor,
    }
}

/// Generic group-by breakdown. Groups `entries` by `key(e)`, skipping entries
/// whose key is `None`, computes `core_metrics` per group, and returns the
/// groups sorted by key ascending for determinism.
fn breakdown<F: Fn(&JournalEntry) -> Option<String>>(
    entries: &[JournalEntry],
    key: F,
) -> Vec<DimensionStat> {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, Vec<&JournalEntry>> = BTreeMap::new();
    for e in entries.iter() {
        if let Some(k) = key(e) {
            groups.entry(k).or_default().push(e);
        }
    }
    groups
        .into_iter()
        .map(|(key, group)| DimensionStat {
            key,
            metrics: core_metrics(&group),
        })
        .collect()
}

/// Weekday name of `close_date` ("Monday".."Sunday"); "unknown" if unparseable.
fn day_of_week_key(s: &str) -> String {
    match parse_day(s) {
        Some(d) => d.format("%A").to_string(),
        None => "unknown".into(),
    }
}

/// Session bucket from the TIME-of-day of `close_date` (RFC3339 only). Uses the
/// stored offset's local time (no timezone conversion). "unknown" when the
/// value is date-only / has no time component.
fn session_key(s: &str) -> String {
    let dt = match DateTime::parse_from_rfc3339(s) {
        Ok(dt) => dt,
        Err(_) => return "unknown".into(),
    };
    let t = dt.time();
    let mins = t.hour() * 60 + t.minute();
    if mins < 9 * 60 + 30 {
        "premarket".into()
    } else if mins < 12 * 60 {
        "morning".into()
    } else if mins < 16 * 60 {
        "afternoon".into()
    } else {
        "after-hours".into()
    }
}

/// Holding bucket from holding seconds (reuses `holding_secs`): scalp <1d,
/// swing 1–5d, position >5d; "unknown" when holding is indeterminable.
fn holding_key(e: &JournalEntry) -> String {
    match holding_secs(e) {
        Some(secs) => {
            let s = secs as i64;
            if s < 86_400 {
                "scalp".into()
            } else if s <= 432_000 {
                "swing".into()
            } else {
                "position".into()
            }
        }
        None => "unknown".into(),
    }
}

/// Fixed position-size buckets. Only non-empty buckets are emitted (the generic
/// helper drops keys with no entries since every entry maps to exactly one).
fn position_size_key(size: f64) -> String {
    if size < 100.0 {
        "<100".into()
    } else if size < 500.0 {
        "100-500".into()
    } else if size < 1000.0 {
        "500-1000".into()
    } else {
        ">=1000".into()
    }
}

/// Bucket R-multiples into the fixed half-open ranges. Always returns all 5
/// buckets in order (counts may be 0). Empty input still yields 5 zero buckets.
fn r_distribution(rs: &[f64]) -> Vec<RBucket> {
    let mut counts = [0usize; 5];
    for &r in rs {
        let idx = if r <= -1.0 {
            0 // <= -1
        } else if r < 0.0 {
            1 // -1 < r < 0
        } else if r < 1.0 {
            2 // 0 <= r < 1
        } else if r < 2.0 {
            3 // 1 <= r < 2
        } else {
            4 // >= 2
        };
        counts[idx] += 1;
    }
    let labels = ["<=-1R", "-1..0R", "0..1R", "1..2R", ">=2R"];
    labels
        .iter()
        .zip(counts.iter())
        .map(|(&label, &count)| RBucket {
            label: label.to_string(),
            count,
        })
        .collect()
}

/// Win/loss streaks over close_date-ordered entries. Winner = total_pl > 0,
/// loser = total_pl < 0; breakeven (== 0) breaks both and counts as neither.
/// Returns (longest_win, longest_loss, current_streak). current_streak is
/// signed: +k trailing winners, -k trailing losers, 0 if last is breakeven/empty.
fn streaks(sorted: &[JournalEntry]) -> (usize, usize, i32) {
    let mut longest_win = 0usize;
    let mut longest_loss = 0usize;
    let mut cur_win = 0usize;
    let mut cur_loss = 0usize;
    for e in sorted.iter() {
        if e.total_pl > 0.0 {
            cur_win += 1;
            cur_loss = 0;
            if cur_win > longest_win {
                longest_win = cur_win;
            }
        } else if e.total_pl < 0.0 {
            cur_loss += 1;
            cur_win = 0;
            if cur_loss > longest_loss {
                longest_loss = cur_loss;
            }
        } else {
            // breakeven breaks both streaks
            cur_win = 0;
            cur_loss = 0;
        }
    }
    let current = if cur_win > 0 {
        cur_win as i32
    } else if cur_loss > 0 {
        -(cur_loss as i32)
    } else {
        0
    };
    (longest_win, longest_loss, current)
}

/// Holding time in seconds: prefer `(close_date - open_date)` parsed via chrono
/// when BOTH parse (RFC3339, else `%Y-%m-%d`); otherwise fall back to
/// `duration` which is stored in seconds (`(close - open).num_seconds()` in the
/// journal create/update path). None when neither source is determinable.
fn holding_secs(e: &JournalEntry) -> Option<f64> {
    if let (Some(open), Some(close)) = (parse_dt(&e.open_date), parse_dt(&e.close_date)) {
        return Some((close - open).num_seconds() as f64);
    }
    // duration is in seconds; only meaningful when set (> 0).
    if e.duration > 0 {
        return Some(e.duration as f64);
    }
    None
}

/// Parse a datetime to seconds-resolution. RFC3339 first (preserves time),
/// then date-only `%Y-%m-%d` (midnight UTC). None when unparseable.
fn parse_dt(s: &str) -> Option<DateTime<chrono::Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    let end = 10.min(s.len());
    NaiveDate::parse_from_str(&s[..end], "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|ndt| DateTime::<chrono::Utc>::from_naive_utc_and_offset(ndt, chrono::Utc))
}

/// Sample stdev (n-1) of monthly win rates (winners/total * 100 per `YYYY-MM`
/// prefix of close_date). None when fewer than 2 distinct months.
fn monthly_win_rate_stdev(sorted: &[JournalEntry]) -> Option<f64> {
    use std::collections::BTreeMap;
    // (winners, decisive) per month, in month order. Decisive = wins + losses;
    // breakeven (scratch) trades are excluded from the win rate on both sides.
    let mut by_month: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for e in sorted.iter() {
        if e.close_date.len() < 7 {
            continue;
        }
        let key = e.close_date[..7].to_string();
        let entry = by_month.entry(key).or_insert((0, 0));
        if e.total_pl > 0.0 {
            entry.0 += 1;
            entry.1 += 1;
        } else if e.total_pl < 0.0 {
            entry.1 += 1;
        }
    }
    let rates: Vec<f64> = by_month
        .values()
        .filter(|&&(_, decisive)| decisive > 0)
        .map(|&(w, decisive)| w as f64 / decisive as f64 * 100.0)
        .collect();
    stdev(&rates)
}

/// Mean of a slice. None when empty.
fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        None
    } else {
        Some(xs.iter().sum::<f64>() / xs.len() as f64)
    }
}

/// Sample standard deviation (n-1). None when len < 2.
fn stdev(xs: &[f64]) -> Option<f64> {
    let n = xs.len();
    if n < 2 {
        return None;
    }
    let mean = xs.iter().sum::<f64>() / n as f64;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    Some(var.sqrt())
}

/// Parse a close_date into a `NaiveDate`. Tries RFC3339 (with time) first,
/// then a date-only `%Y-%m-%d` on the first 10 chars. None when unparseable.
fn parse_day(s: &str) -> Option<NaiveDate> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.date_naive());
    }
    let end = 10.min(s.len());
    NaiveDate::parse_from_str(&s[..end], "%Y-%m-%d").ok()
}

/// Longest drawdown duration in days: max gap from a running peak to the first
/// later point whose equity meets/exceeds that peak. Unrecovered drawdowns are
/// measured to the last trade's date. Points with unparseable dates are skipped
/// for the duration calc. Returns 0 when there is no drawdown.
fn longest_drawdown_days(sorted: &[JournalEntry]) -> i64 {
    // Build (date, equity) pairs, skipping entries whose date can't parse.
    let mut points: Vec<(NaiveDate, f64)> = Vec::with_capacity(sorted.len());
    let mut equity = 0.0_f64;
    for e in sorted.iter() {
        equity += e.total_pl;
        if let Some(d) = parse_day(&e.close_date) {
            points.push((d, equity));
        }
    }
    if points.is_empty() {
        return 0;
    }
    let last_date = points[points.len() - 1].0;

    let mut longest = 0_i64;
    let mut peak = f64::NEG_INFINITY;
    let mut peak_date: Option<NaiveDate> = None;
    let mut in_drawdown = false;
    for &(date, eq) in points.iter() {
        if eq >= peak {
            // New (or matched) peak: any open drawdown has just recovered.
            if in_drawdown {
                if let Some(pd) = peak_date {
                    let days = (date - pd).num_days();
                    if days > longest {
                        longest = days;
                    }
                }
                in_drawdown = false;
            }
            peak = eq;
            peak_date = Some(date);
        } else {
            // Below the peak => in a drawdown.
            in_drawdown = true;
        }
    }
    // Unrecovered drawdown at the end: measure to the last trade's date.
    if in_drawdown && let Some(pd) = peak_date {
        let days = (last_date - pd).num_days();
        if days > longest {
            longest = days;
        }
    }
    longest
}

/// SQN = sqrt(N) * mean(R)/stdev(R) (sample stdev). None when N<2 or stdev==0.
fn sqn(rs: &[f64]) -> Option<f64> {
    let n = rs.len();
    let sd = stdev(rs)?; // None when n < 2
    if sd == 0.0 {
        return None;
    }
    let mean = rs.iter().sum::<f64>() / n as f64;
    Some((n as f64).sqrt() * (mean / sd))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::db::schema::tables::journal_table::JournalEntry;

    // Minimal builder: only the fields the math uses.
    fn t(total_pl: f64, entry: f64, stop: f64, size: f64, close: &str) -> JournalEntry {
        JournalEntry {
            id: "x".into(),
            user_id: "u".into(),
            account_id: "a".into(),
            open_date: close.into(),
            close_date: close.into(),
            entry_price: entry,
            exit_price: 0.0,
            position_size: size,
            symbol: "AAA".into(),
            symbol_name: "AAA".into(),
            status: "closed".into(),
            total_pl,
            net_roi: 0.0,
            duration: 0,
            stop_loss: if stop == 0.0 { None } else { Some(stop) },
            risk_reward: None,
            trade_type: "long".into(),
            mistakes: String::new(),
            entry_tactics: String::new(),
            edges_spotted: String::new(),
            playbook_id: None,
            notes: None,
            broke_30min_rule: None,
            pre_trade_conviction: None,
            market_regime: None,
            is_planned_pre_market: None,
            revenge_trade: None,
            rule_adherence_score: None,
            created_at: close.into(),
        }
    }

    use crate::service::db::schema::tables::tags_table::Tag;

    // A `t()` entry with an explicit id (so it can key into a trade_tags map).
    // entry=100, stop=99, size=1 makes dollar P/L == total_pl and R == total_pl,
    // so the fixture numbers read directly as dollars.
    fn t_id(id: &str, total_pl: f64, close: &str) -> JournalEntry {
        let mut e = t(total_pl, 100.0, 99.0, 1.0, close);
        e.id = id.into();
        e
    }

    // Build a hydrated TradeTag for a given category/tag/role fixture.
    fn tt(
        category_id: &str,
        category_name: &str,
        role: Option<TagRole>,
        tag_name: &str,
    ) -> TradeTag {
        TradeTag {
            tag: Tag {
                id: format!("{category_id}:{tag_name}"),
                user_id: "u".into(),
                category_id: category_id.into(),
                name: tag_name.into(),
                color: None,
                created_at: String::new(),
                updated_at: String::new(),
            },
            category_id: category_id.into(),
            category_name: category_name.into(),
            role,
        }
    }

    #[test]
    fn r_multiple_uses_initial_dollar_risk() {
        // risk = |100-90| * 1 = $10; dollar pnl +$15 => +1.5R ; -$5 => -0.5R
        let e = vec![
            t(15.0, 100.0, 90.0, 1.0, "2026-01-01"),
            t(-5.0, 100.0, 90.0, 1.0, "2026-01-02"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert!((a.expectancy_r.unwrap() - 0.5).abs() < 1e-9); // (1.5 + -0.5)/2
    }

    #[test]
    fn trade_without_stop_excluded_from_r_but_counts_in_dollars() {
        let e = vec![
            t(100.0, 100.0, 0.0, 1.0, "2026-01-01"), // no stop => $100, R-invalid
            t(60.0, 100.0, 99.0, 1.0, "2026-01-02"), // $60, R-valid
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert_eq!(a.r_trade_count, 1); // only the stop'd trade
        assert!((a.net_profit - 160.0).abs() < 1e-9); // both count in $
    }

    #[test]
    fn expectancy_dollars_matches_formula() {
        // winner +$100 (1 of 2 => wr .5), loser -$40 => exp = .5*100 - .5*40 = 30
        let e = vec![
            t(100.0, 100.0, 99.0, 1.0, "2026-01-01"),
            t(-40.0, 100.0, 99.0, 1.0, "2026-01-02"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert!((a.expectancy_dollars - 30.0).abs() < 1e-9);
    }

    #[test]
    fn win_rate_excludes_breakeven_but_expectancy_counts_it() {
        // 2 wins, 1 loss, 1 breakeven. Win rate excludes the scratch from both
        // sides => 2 / (2 + 1) = 66.67%, NOT 2 / 4 = 50%. Expectancy is $ per
        // trade over ALL trades => net (10+20-10+0) / 4 = 5.
        let e = vec![
            t(10.0, 100.0, 99.0, 1.0, "2026-01-01"),  // win
            t(20.0, 100.0, 99.0, 1.0, "2026-01-02"),  // win
            t(-10.0, 100.0, 99.0, 1.0, "2026-01-03"), // loss
            t(0.0, 100.0, 99.0, 1.0, "2026-01-04"),   // breakeven (scratch)
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert!((a.win_rate - (2.0 / 3.0 * 100.0)).abs() < 1e-9);
        assert!((a.expectancy_dollars - 5.0).abs() < 1e-9);
    }

    #[test]
    fn max_drawdown_dollars_peak_to_trough() {
        let e = vec![
            t(100.0, 100.0, 99.0, 1.0, "2026-01-01"),
            t(-150.0, 100.0, 99.0, 1.0, "2026-01-02"),
            t(50.0, 100.0, 99.0, 1.0, "2026-01-03"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert!((a.max_drawdown_dollars - 150.0).abs() < 1e-9);
        assert!((a.current_drawdown_dollars - 100.0).abs() < 1e-9); // peak 100, final 0
    }

    #[test]
    fn max_drawdown_pct_uses_account_equity_when_supplied() {
        // Same PnL series as `max_drawdown_dollars_peak_to_trough`:
        //   cumulative PnL: +100, -50, 0 => net_profit = 0
        //   peak cumulative PnL at the deepest trough = 100
        //   max_drawdown_dollars = 100 - (-50) = 150 (unchanged)
        let e = vec![
            t(100.0, 100.0, 99.0, 1.0, "2026-01-01"),
            t(-150.0, 100.0, 99.0, 1.0, "2026-01-02"),
            t(50.0, 100.0, 99.0, 1.0, "2026-01-03"),
        ];

        // Fallback (None): denominator is peak cumulative PnL = 100.
        let none = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert!((none.max_drawdown_dollars - 150.0).abs() < 1e-9);
        assert!((none.max_drawdown_pct - 150.0).abs() < 1e-9); // 150/100*100
        assert!(none.starting_equity.is_none());
        assert!(none.account_equity.is_none());

        // With real equity 10000 and net_profit 0 => starting_equity = 10000.
        // peak_account_equity_at_trough = 10000 + 100 = 10100.
        // expected pct = 150 / 10100 * 100 = 1.4851485148514851
        let some = compute_advanced_analytics(&e, Some(10000.0), &HashMap::new(), &HashMap::new());
        assert!((some.max_drawdown_dollars - 150.0).abs() < 1e-9); // unchanged
        let expected = 150.0 / 10100.0 * 100.0;
        assert!((some.max_drawdown_pct - expected).abs() < 1e-9);
        assert!((some.starting_equity.unwrap() - 10000.0).abs() < 1e-9);
        assert!((some.account_equity.unwrap() - 10000.0).abs() < 1e-9);
    }

    #[test]
    fn no_drawdown_when_monotonic_up() {
        let e = vec![
            t(10.0, 10.0, 9.0, 1.0, "2026-01-01"),
            t(20.0, 10.0, 9.0, 1.0, "2026-01-02"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert_eq!(a.max_drawdown_dollars, 0.0);
    }

    // Build an entry with explicit open/close dates (for holding-time tests).
    fn t_oc(total_pl: f64, open: &str, close: &str) -> JournalEntry {
        let mut e = t(total_pl, 100.0, 99.0, 1.0, close);
        e.open_date = open.into();
        e
    }

    #[test]
    fn r_distribution_buckets() {
        // R-multiples [-1, -0.5, 0.5, 1.5, 2.5] => counts [1,1,1,1,1].
        // risk = |100-99| * 1 = $1, so R == dollar pnl == total_pl here.
        let e = vec![
            t(-1.0, 100.0, 99.0, 1.0, "2026-01-01"), // -1.0
            t(-0.5, 100.0, 99.0, 1.0, "2026-01-02"), // -0.5
            t(0.5, 100.0, 99.0, 1.0, "2026-01-03"),  // 0.5
            t(1.5, 100.0, 99.0, 1.0, "2026-01-04"),  // 1.5
            t(2.5, 100.0, 99.0, 1.0, "2026-01-05"),  // 2.5
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert_eq!(a.r_distribution.len(), 5);
        let labels: Vec<&str> = a.r_distribution.iter().map(|b| b.label.as_str()).collect();
        assert_eq!(labels, vec!["<=-1R", "-1..0R", "0..1R", "1..2R", ">=2R"]);
        let counts: Vec<usize> = a.r_distribution.iter().map(|b| b.count).collect();
        assert_eq!(counts, vec![1, 1, 1, 1, 1]);
    }

    #[test]
    fn r_distribution_always_five_buckets_when_empty() {
        let a = compute_advanced_analytics(&[], None, &HashMap::new(), &HashMap::new());
        // empty input => default => empty distribution
        assert!(a.r_distribution.is_empty());
        // a trade with no stop is R-invalid => all-zero counts but 5 buckets
        let e = vec![t(100.0, 50.0, 0.0, 5.0, "2026-01-01")];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert_eq!(a.r_distribution.len(), 5);
        assert!(a.r_distribution.iter().all(|b| b.count == 0));
    }

    #[test]
    fn streaks_count_consecutive() {
        let e = vec![
            t(10.0, 1.0, 0.5, 1.0, "2026-01-01"),
            t(10.0, 1.0, 0.5, 1.0, "2026-01-02"),
            t(-10.0, 1.0, 0.5, 1.0, "2026-01-03"),
            t(10.0, 1.0, 0.5, 1.0, "2026-01-04"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert_eq!(a.longest_win_streak, 2);
        assert_eq!(a.longest_loss_streak, 1);
        assert_eq!(a.current_streak, 1); // last trade is a win
    }

    #[test]
    fn current_streak_negative_for_trailing_losers() {
        let e = vec![
            t(10.0, 1.0, 0.5, 1.0, "2026-01-01"),
            t(-10.0, 1.0, 0.5, 1.0, "2026-01-02"),
            t(-10.0, 1.0, 0.5, 1.0, "2026-01-03"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert_eq!(a.longest_win_streak, 1);
        assert_eq!(a.longest_loss_streak, 2);
        assert_eq!(a.current_streak, -2);
    }

    #[test]
    fn breakeven_breaks_streaks() {
        let e = vec![
            t(10.0, 1.0, 0.5, 1.0, "2026-01-01"),
            t(0.0, 1.0, 0.5, 1.0, "2026-01-02"), // breakeven: counts as neither
            t(10.0, 1.0, 0.5, 1.0, "2026-01-03"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert_eq!(a.longest_win_streak, 1);
        assert_eq!(a.longest_loss_streak, 0);
        assert_eq!(a.current_streak, 1);
    }

    #[test]
    fn current_streak_zero_when_last_breakeven() {
        let e = vec![
            t(10.0, 1.0, 0.5, 1.0, "2026-01-01"),
            t(0.0, 1.0, 0.5, 1.0, "2026-01-02"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert_eq!(a.current_streak, 0);
    }

    #[test]
    fn holding_time_avg_differs_winners_vs_losers() {
        // winner held 1h (3600s), loser held 2h (7200s)
        let e = vec![
            t_oc(50.0, "2026-01-01T09:00:00Z", "2026-01-01T10:00:00Z"),
            t_oc(-50.0, "2026-01-02T09:00:00Z", "2026-01-02T11:00:00Z"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert!((a.avg_hold_winners_secs.unwrap() - 3600.0).abs() < 1e-9);
        assert!((a.avg_hold_losers_secs.unwrap() - 7200.0).abs() < 1e-9);
    }

    #[test]
    fn holding_time_date_only_zero_diff() {
        // date-only parse: same day => 0 seconds held
        let e = vec![t_oc(50.0, "2026-01-01", "2026-01-01")];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert!((a.avg_hold_winners_secs.unwrap() - 0.0).abs() < 1e-9);
        assert!(a.avg_hold_losers_secs.is_none());
    }

    #[test]
    fn monthly_win_rate_stdev_two_months() {
        // Jan: 1 win of 2 => 50%. Feb: 2 wins of 2 => 100%.
        // sample stdev of [50, 100] = sqrt(((50-75)^2 + (100-75)^2)/1) = sqrt(1250) ~= 35.355
        let e = vec![
            t(10.0, 1.0, 0.5, 1.0, "2026-01-01"),
            t(-10.0, 1.0, 0.5, 1.0, "2026-01-15"),
            t(10.0, 1.0, 0.5, 1.0, "2026-02-01"),
            t(10.0, 1.0, 0.5, 1.0, "2026-02-15"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert!((a.monthly_win_rate_stdev.unwrap() - 1250.0_f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn monthly_win_rate_stdev_none_single_month() {
        let e = vec![
            t(10.0, 1.0, 0.5, 1.0, "2026-01-01"),
            t(-10.0, 1.0, 0.5, 1.0, "2026-01-15"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert!(a.monthly_win_rate_stdev.is_none());
    }

    // Build an entry with an explicit symbol.
    fn t_sym(total_pl: f64, symbol: &str, close: &str) -> JournalEntry {
        let mut e = t(total_pl, 100.0, 99.0, 1.0, close);
        e.symbol = symbol.into();
        e
    }

    #[test]
    fn by_symbol_groups_correctly() {
        // 2 AAPL (+100, -40) and 1 MSFT (+60) => 2 groups.
        let e = vec![
            t_sym(100.0, "AAPL", "2026-01-01"),
            t_sym(-40.0, "AAPL", "2026-01-02"),
            t_sym(60.0, "MSFT", "2026-01-03"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert_eq!(a.by_symbol.len(), 2);
        // sorted ascending by key => AAPL first, MSFT second.
        let aapl = &a.by_symbol[0];
        let msft = &a.by_symbol[1];
        assert_eq!(aapl.key, "AAPL");
        assert_eq!(aapl.metrics.trade_count, 2);
        assert!((aapl.metrics.net_profit - 60.0).abs() < 1e-9);
        // wr .5: exp = .5*100 - .5*40 = 30
        assert!((aapl.metrics.expectancy_dollars - 30.0).abs() < 1e-9);
        assert_eq!(msft.key, "MSFT");
        assert_eq!(msft.metrics.trade_count, 1);
        assert!((msft.metrics.net_profit - 60.0).abs() < 1e-9);
        // single winner: wr 1.0 => exp = avg_win = 60
        assert!((msft.metrics.expectancy_dollars - 60.0).abs() < 1e-9);
    }

    #[test]
    fn by_direction_long_vs_short_split() {
        let mut long_a = t(100.0, 100.0, 99.0, 1.0, "2026-01-01");
        long_a.trade_type = "long".into();
        let mut long_b = t(-40.0, 100.0, 99.0, 1.0, "2026-01-02");
        long_b.trade_type = "long".into();
        let mut short_a = t(50.0, 100.0, 99.0, 1.0, "2026-01-03");
        short_a.trade_type = "short".into();
        let e = vec![long_a, long_b, short_a];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert_eq!(a.by_direction.len(), 2);
        let long = &a.by_direction[0]; // "long" < "short"
        let short = &a.by_direction[1];
        assert_eq!(long.key, "long");
        assert_eq!(long.metrics.trade_count, 2);
        assert!((long.metrics.net_profit - 60.0).abs() < 1e-9);
        assert_eq!(short.key, "short");
        assert_eq!(short.metrics.trade_count, 1);
        assert!((short.metrics.net_profit - 50.0).abs() < 1e-9);
    }

    // --- Task 5: behavioral / process metrics ---

    #[test]
    fn clean_vs_flawed_keys_off_mistake_role_tag() {
        // 2 flawed trades (>=1 mistake-role tag): +50, -30 => net 20
        // 1 clean trade (no mistake-role tag; tagged a tactic instead): +100
        let flawed_a = t_id("f1", 50.0, "2026-01-01");
        let flawed_b = t_id("f2", -30.0, "2026-01-02");
        let clean_a = t_id("c1", 100.0, "2026-01-03");
        let e = vec![flawed_a, flawed_b, clean_a];

        let mut trade_tags: HashMap<String, Vec<TradeTag>> = HashMap::new();
        trade_tags.insert(
            "f1".into(),
            vec![tt(
                "cat-mistake",
                "Mistakes",
                Some(TagRole::Mistake),
                "chased",
            )],
        );
        trade_tags.insert(
            "f2".into(),
            vec![tt(
                "cat-mistake",
                "Mistakes",
                Some(TagRole::Mistake),
                "moved-stop",
            )],
        );
        // Clean trade carries a non-mistake tag => still "clean".
        trade_tags.insert(
            "c1".into(),
            vec![tt(
                "cat-tactic",
                "Tactics",
                Some(TagRole::Tactic),
                "breakout",
            )],
        );

        let a = compute_advanced_analytics(&e, None, &trade_tags, &HashMap::new());
        assert_eq!(a.clean_vs_flawed.flawed.trade_count, 2);
        assert!((a.clean_vs_flawed.flawed.net_profit - 20.0).abs() < 1e-9);
        assert_eq!(a.clean_vs_flawed.clean.trade_count, 1);
        assert!((a.clean_vs_flawed.clean.net_profit - 100.0).abs() < 1e-9);
    }

    #[test]
    fn tag_breakdowns_group_per_tag_within_category() {
        // trade 1: tactics "breakout" + "pullback", +100
        // trade 2: tactics "breakout", -40
        // => Tactics category: "breakout" 2 trades (net 60), "pullback" 1 (net 100).
        let t1 = t_id("t1", 100.0, "2026-01-01");
        let t2 = t_id("t2", -40.0, "2026-01-02");
        let e = vec![t1, t2];

        let mut trade_tags: HashMap<String, Vec<TradeTag>> = HashMap::new();
        trade_tags.insert(
            "t1".into(),
            vec![
                tt("cat-tactic", "Tactics", Some(TagRole::Tactic), "breakout"),
                tt("cat-tactic", "Tactics", Some(TagRole::Tactic), "pullback"),
            ],
        );
        trade_tags.insert(
            "t2".into(),
            vec![tt(
                "cat-tactic",
                "Tactics",
                Some(TagRole::Tactic),
                "breakout",
            )],
        );

        let a = compute_advanced_analytics(&e, None, &trade_tags, &HashMap::new());
        assert_eq!(a.tag_breakdowns.len(), 1);
        let tactics = &a.tag_breakdowns[0];
        assert_eq!(tactics.category_name, "Tactics");
        assert_eq!(tactics.role.as_deref(), Some("tactic"));
        assert_eq!(tactics.tags.len(), 2);
        // tags sorted ascending by key: "breakout" then "pullback"
        assert_eq!(tactics.tags[0].key, "breakout");
        assert_eq!(tactics.tags[0].metrics.trade_count, 2);
        assert!((tactics.tags[0].metrics.net_profit - 60.0).abs() < 1e-9);
        assert_eq!(tactics.tags[1].key, "pullback");
        assert_eq!(tactics.tags[1].metrics.trade_count, 1);
        assert!((tactics.tags[1].metrics.net_profit - 100.0).abs() < 1e-9);
    }

    #[test]
    fn tag_breakdowns_include_custom_category() {
        // A user-defined (role-less) category yields its own breakdown.
        let t1 = t_id("t1", 100.0, "2026-01-01");
        let e = vec![t1];

        let mut trade_tags: HashMap<String, Vec<TradeTag>> = HashMap::new();
        trade_tags.insert(
            "t1".into(),
            vec![
                tt("cat-tactic", "Tactics", Some(TagRole::Tactic), "breakout"),
                tt("cat-mood", "Mood", None, "calm"),
            ],
        );

        let a = compute_advanced_analytics(&e, None, &trade_tags, &HashMap::new());
        // Two categories, ordered by name: "Mood" then "Tactics".
        assert_eq!(a.tag_breakdowns.len(), 2);
        let mood = &a.tag_breakdowns[0];
        assert_eq!(mood.category_name, "Mood");
        assert_eq!(mood.role, None);
        assert_eq!(mood.tags.len(), 1);
        assert_eq!(mood.tags[0].key, "calm");
        assert_eq!(mood.tags[0].metrics.trade_count, 1);
        assert_eq!(a.tag_breakdowns[1].category_name, "Tactics");
    }

    #[test]
    fn untagged_trades_yield_no_tag_breakdowns() {
        // Legacy untagged trades (absent from the map) contribute to nothing.
        let e = vec![t_id("t1", 100.0, "2026-01-01")];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert!(a.tag_breakdowns.is_empty());
        // No mistake tag => all trades clean.
        assert_eq!(a.clean_vs_flawed.clean.trade_count, 1);
        assert_eq!(a.clean_vs_flawed.flawed.trade_count, 0);
    }

    #[test]
    fn trades_per_day_stats() {
        // day 01-01: 3 trades; day 01-02: 1 trade => avg 2, max 3, stdev of [3,1]
        let e = vec![
            t(10.0, 1.0, 0.5, 1.0, "2026-01-01"),
            t(10.0, 1.0, 0.5, 1.0, "2026-01-01"),
            t(10.0, 1.0, 0.5, 1.0, "2026-01-01"),
            t(10.0, 1.0, 0.5, 1.0, "2026-01-02"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert!((a.trades_per_day.avg - 2.0).abs() < 1e-9);
        assert_eq!(a.trades_per_day.max, 3);
        // sample stdev of [3,1] = sqrt(((3-2)^2 + (1-2)^2)/1) = sqrt(2)
        assert!((a.trades_per_day.stdev.unwrap() - 2.0_f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn trades_per_day_stdev_none_single_day() {
        let e = vec![
            t(10.0, 1.0, 0.5, 1.0, "2026-01-01"),
            t(10.0, 1.0, 0.5, 1.0, "2026-01-01"),
        ];
        let a = compute_advanced_analytics(&e, None, &HashMap::new(), &HashMap::new());
        assert!((a.trades_per_day.avg - 2.0).abs() < 1e-9);
        assert_eq!(a.trades_per_day.max, 2);
        assert!(a.trades_per_day.stdev.is_none());
    }
}
