//! Derives "pending trades" — round-trip trade lifecycles assembled from
//! raw broker fills, scoped to those that haven't been fully journaled yet.
//!
//! A lifecycle is the span between a position opening (qty leaves 0) and the
//! next time it returns to 0 (or end-of-history if still open). Reversals
//! (fills that cross zero in a single execution) close the prior lifecycle
//! and reset the running qty — the leftover side is dropped on the floor in
//! this simple v1, which matches the 99% case where users journal clean
//! round-trips. Real fractional-fill tax-lot tracking is out of scope.

use std::collections::HashSet;

use anyhow::Result;
use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::service::db::schema::tables::brokerage_table::{self, BrokerageTransaction};
use crate::service::db::schema::tables::journal_table;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PendingTrade {
    /// Synthetic id: `{accountId}:{symbol}:{firstFillId}`. Stable across
    /// refreshes as long as the underlying fills don't change.
    pub id: String,
    pub symbol: String,
    /// "long" | "short"
    pub direction: String,
    /// "open" | "closed". An "open" trade still has a non-zero position.
    pub status: String,
    /// First fill's trade_date (ISO string).
    pub open_date: String,
    /// Last fill's trade_date when status=closed, None when still open.
    pub close_date: Option<String>,
    /// Total opening qty (sum of |units| for fills matching `direction`).
    pub entry_units: f64,
    /// Weighted average price across opening fills.
    pub avg_entry_price: f64,
    /// Weighted average across closing fills. None when status=open.
    pub avg_exit_price: Option<f64>,
    /// Realized P&L after fees. None when status=open.
    pub realized_pnl: Option<f64>,
    pub transaction_ids: Vec<String>,
    pub fill_count: i32,
    /// True iff every fill in this lifecycle is already linked to a journal
    /// entry. Such lifecycles are filtered out of the result by default;
    /// the flag is retained so the UI can mark partially-journaled trades.
    pub is_fully_linked: bool,
    /// True iff at least one fill (but not all) is already linked. Renders
    /// a "partially journaled" pill in the UI.
    pub is_partially_linked: bool,
}

/// Returns +1 for a buying fill, -1 for a selling fill, 0 for anything else
/// (dividend, fee, transfer, etc.). Uses prefix matching to be tolerant of
/// option activity types like "BUY_TO_OPEN" / "SELL_TO_CLOSE".
fn fill_direction(transaction_type: &str) -> f64 {
    let upper = transaction_type.to_ascii_uppercase();
    if upper.starts_with("BUY") {
        1.0
    } else if upper.starts_with("SELL") {
        -1.0
    } else {
        0.0
    }
}

#[derive(Debug)]
struct LifecycleBuilder<'a> {
    direction_sign: f64,
    fills: Vec<&'a BrokerageTransaction>,
}

impl<'a> LifecycleBuilder<'a> {
    fn new(direction_sign: f64) -> Self {
        Self {
            direction_sign,
            fills: Vec::new(),
        }
    }

    fn push(&mut self, fill: &'a BrokerageTransaction) {
        self.fills.push(fill);
    }

    fn build(
        self,
        account_id: &str,
        status: &str,
        linked_ids: &HashSet<String>,
    ) -> Option<PendingTrade> {
        let first = self.fills.first()?;
        let last = self.fills.last()?;
        let symbol = first.symbol.clone().unwrap_or_default();
        if symbol.is_empty() {
            return None;
        }

        let direction = if self.direction_sign > 0.0 {
            "long"
        } else {
            "short"
        };
        let open_dir = self.direction_sign;

        // Aggregate opening vs closing legs (opening = same direction as the
        // lifecycle; closing = opposite).
        let mut open_units = 0.0_f64;
        let mut open_notional = 0.0_f64;
        let mut close_units = 0.0_f64;
        let mut close_notional = 0.0_f64;
        let mut total_fees = 0.0_f64;
        let mut tx_ids: Vec<String> = Vec::with_capacity(self.fills.len());
        let mut linked_count = 0_usize;

        for fill in &self.fills {
            total_fees += fill.fee;
            tx_ids.push(fill.id.clone());
            if linked_ids.contains(&fill.id) {
                linked_count += 1;
            }

            let dir = fill_direction(&fill.transaction_type);
            if dir == 0.0 {
                continue;
            }
            let units = fill.units.abs();
            if dir == open_dir {
                open_units += units;
                open_notional += units * fill.price;
            } else {
                close_units += units;
                close_notional += units * fill.price;
            }
        }

        if open_units <= 0.0 {
            // Shouldn't happen — a lifecycle always has at least one opening fill.
            return None;
        }

        let avg_entry_price = open_notional / open_units;
        let (avg_exit_price, realized_pnl) = if status == "closed" && close_units > 0.0 {
            let avg_exit = close_notional / close_units;
            // P&L uses the qty that actually closed out (min of the two sides).
            // For a clean close they're equal; in a reversal the close side
            // overshoots and we clamp to the opening qty.
            let realized_qty = open_units.min(close_units);
            let gross = if open_dir > 0.0 {
                (avg_exit - avg_entry_price) * realized_qty
            } else {
                (avg_entry_price - avg_exit) * realized_qty
            };
            (Some(avg_exit), Some(gross - total_fees))
        } else {
            (None, None)
        };

        let is_fully_linked = linked_count == tx_ids.len();
        let is_partially_linked = linked_count > 0 && !is_fully_linked;

        let synthetic_id = format!("{}:{}:{}", account_id, symbol, first.id);

        Some(PendingTrade {
            id: synthetic_id,
            symbol,
            direction: direction.to_string(),
            status: status.to_string(),
            open_date: first.trade_date.clone().unwrap_or_default(),
            close_date: if status == "closed" {
                last.trade_date.clone()
            } else {
                None
            },
            entry_units: open_units,
            avg_entry_price,
            avg_exit_price,
            realized_pnl,
            transaction_ids: tx_ids,
            fill_count: self.fills.len() as i32,
            is_fully_linked,
            is_partially_linked,
        })
    }
}

/// Compute pending trades for a (user, account). Returns lifecycles that
/// have at least one unlinked fill, sorted with open trades first, then by
/// most-recent close/open date.
pub async fn compute_pending_trades(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<PendingTrade>> {
    let fills = brokerage_table::list_all_for_lifecycle(pool, user_id, account_id).await?;
    let linked_vec =
        journal_table::list_linked_brokerage_transaction_ids(pool, user_id, account_id).await?;
    let linked_ids: HashSet<String> = linked_vec.into_iter().collect();

    let mut result: Vec<PendingTrade> = Vec::new();

    // Group fills by symbol. Input is already ordered (symbol, trade_date, id).
    let mut current_symbol: Option<String> = None;
    let mut qty: f64 = 0.0;
    let mut builder: Option<LifecycleBuilder<'_>> = None;

    for fill in &fills {
        let symbol = fill.symbol.clone().unwrap_or_default();
        if symbol.is_empty() {
            continue;
        }

        // New symbol — close out any dangling lifecycle as "open".
        if current_symbol.as_deref() != Some(symbol.as_str()) {
            if let Some(lc) = builder.take()
                && let Some(pt) = lc.build(account_id, "open", &linked_ids)
            {
                result.push(pt);
            }
            current_symbol = Some(symbol.clone());
            qty = 0.0;
        }

        let dir = fill_direction(&fill.transaction_type);
        if dir == 0.0 {
            // Non-trade activity (dividend, fee, transfer). Attach to current
            // lifecycle so its fees roll up correctly, but skip qty math.
            if let Some(ref mut lc) = builder {
                lc.push(fill);
            }
            continue;
        }

        let delta = fill.units.abs() * dir;
        let new_qty = qty + delta;

        if builder.is_none() {
            // Starting fresh.
            if delta != 0.0 {
                let mut lc = LifecycleBuilder::new(dir);
                lc.push(fill);
                builder = Some(lc);
            }
            qty = new_qty;
            continue;
        }

        let lc = builder.as_mut().expect("builder is Some by check above");
        lc.push(fill);

        let crossed_zero = (qty > 0.0 && new_qty < 0.0) || (qty < 0.0 && new_qty > 0.0);
        let returned_to_zero = new_qty == 0.0;

        if returned_to_zero || crossed_zero {
            if let Some(lc) = builder.take()
                && let Some(pt) = lc.build(account_id, "closed", &linked_ids)
            {
                result.push(pt);
            }
            // Reset qty. In the reversal case this drops the leftover side
            // on the floor — documented v1 limitation.
            qty = 0.0;
        } else {
            qty = new_qty;
        }
    }

    // Flush trailing open lifecycle for the last symbol.
    if let Some(lc) = builder.take()
        && let Some(pt) = lc.build(account_id, "open", &linked_ids)
    {
        result.push(pt);
    }

    // Drop fully-journaled lifecycles.
    result.retain(|t| !t.is_fully_linked);

    // Sort: open trades first, then by close_date DESC (most recent first),
    // breaking ties by open_date DESC.
    result.sort_by(|a, b| match (a.status.as_str(), b.status.as_str()) {
        ("open", "closed") => std::cmp::Ordering::Less,
        ("closed", "open") => std::cmp::Ordering::Greater,
        _ => b
            .close_date
            .clone()
            .unwrap_or_default()
            .cmp(&a.close_date.clone().unwrap_or_default())
            .then_with(|| b.open_date.cmp(&a.open_date)),
    });

    Ok(result)
}
