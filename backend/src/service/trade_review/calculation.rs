use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::{
    allocation::TrancheReconciliation,
    types::{EpisodeDirection, PlanSnapshot, TradeEpisodeDraft},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFlag {
    ExcessRisk,
    UnplannedSize,
    MissedTranche,
    EntryBeyondStop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewCalculation {
    pub planned_quantity: Decimal,
    pub actual_quantity: Decimal,
    pub planned_weighted_entry: Decimal,
    pub actual_weighted_entry: Decimal,
    pub entry_slippage: Decimal,
    pub planned_risk: Decimal,
    pub actual_risk: Decimal,
    pub risk_drift: Decimal,
    pub realized_pnl: Option<Decimal>,
    pub planned_r_multiple: Option<Decimal>,
    pub actual_r_multiple: Option<Decimal>,
    pub flags: Vec<ReviewFlag>,
}

pub fn calculate_review(
    plan: &PlanSnapshot,
    episode: &TradeEpisodeDraft,
    reconciliation: &TrancheReconciliation,
) -> Option<ReviewCalculation> {
    let planned_quantity = plan.quantity();
    let actual_quantity = episode.entry_quantity();
    let planned_entry = plan.weighted_entry()?;
    if actual_quantity <= Decimal::ZERO {
        return None;
    }
    let actual_entry = episode
        .entry_allocations()
        .map(|fill| fill.price * fill.quantity)
        .sum::<Decimal>()
        / actual_quantity;
    let multiplier = plan.instrument.multiplier();
    let valid_stop = match plan.direction {
        EpisodeDirection::Long => plan.stop_loss < planned_entry,
        EpisodeDirection::Short => plan.stop_loss > planned_entry,
    };
    let planned_risk = if valid_stop {
        (planned_entry - plan.stop_loss).abs() * planned_quantity * multiplier
    } else {
        Decimal::ZERO
    };
    let actual_stop_valid = match plan.direction {
        EpisodeDirection::Long => plan.stop_loss < actual_entry,
        EpisodeDirection::Short => plan.stop_loss > actual_entry,
    };
    let actual_risk = if actual_stop_valid {
        (actual_entry - plan.stop_loss).abs() * actual_quantity * multiplier
    } else {
        Decimal::ZERO
    };
    let direction_sign = match plan.direction {
        EpisodeDirection::Long => Decimal::ONE,
        EpisodeDirection::Short => -Decimal::ONE,
    };
    let entry_slippage = (actual_entry - planned_entry) * direction_sign;
    let realized_pnl = episode.closed_at.map(|_| {
        let exit_quantity: Decimal = episode.exit_allocations().map(|fill| fill.quantity).sum();
        let exit_value: Decimal = episode
            .exit_allocations()
            .map(|fill| fill.price * fill.quantity)
            .sum();
        let exit_entry_basis = actual_entry * exit_quantity;
        let gross = (exit_value - exit_entry_basis) * direction_sign * multiplier;
        let fees: Decimal = episode.allocations.iter().map(|fill| fill.fee).sum();
        gross - fees
    });
    let mut flags = Vec::new();
    if !valid_stop || !actual_stop_valid {
        flags.push(ReviewFlag::EntryBeyondStop);
    }
    if actual_risk > planned_risk {
        flags.push(ReviewFlag::ExcessRisk);
    }
    if reconciliation.unplanned_actual_quantity > Decimal::ZERO {
        flags.push(ReviewFlag::UnplannedSize);
    }
    if reconciliation.missing_planned_quantity > Decimal::ZERO {
        flags.push(ReviewFlag::MissedTranche);
    }
    Some(ReviewCalculation {
        planned_quantity,
        actual_quantity,
        planned_weighted_entry: planned_entry,
        actual_weighted_entry: actual_entry,
        entry_slippage,
        planned_risk,
        actual_risk,
        risk_drift: actual_risk - planned_risk,
        realized_pnl,
        planned_r_multiple: realized_pnl
            .filter(|_| planned_risk > Decimal::ZERO)
            .map(|pnl| pnl / planned_risk),
        actual_r_multiple: realized_pnl
            .filter(|_| actual_risk > Decimal::ZERO)
            .map(|pnl| pnl / actual_risk),
        flags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::trade_review::{
        reconcile_tranches,
        types::{ExecutionInstrument, FillAllocation, FillRole, PlanTranche},
    };
    use chrono::{TimeZone, Utc};

    #[test]
    fn calculates_direction_aware_risk_and_flags() {
        let now = Utc.with_ymd_and_hms(2026, 8, 14, 12, 0, 0).unwrap();
        let plan = PlanSnapshot {
            plan_id: "p".into(),
            workspace_id: "w".into(),
            instrument: ExecutionInstrument::Equity {
                symbol: "AVGO".into(),
            },
            direction: EpisodeDirection::Long,
            stop_loss: Decimal::new(95, 0),
            created_at: now,
            active_at_episode_open: true,
            tranches: vec![PlanTranche {
                id: "t".into(),
                order: 0,
                quantity: Decimal::new(10, 0),
                entry_price: Decimal::new(100, 0),
            }],
        };
        let allocations = vec![FillAllocation {
            transaction_id: "f".into(),
            role: FillRole::Entry,
            quantity: Decimal::new(12, 0),
            price: Decimal::new(102, 0),
            fee: Decimal::new(1, 0),
            executed_at: now,
        }];
        let episode = TradeEpisodeDraft {
            instrument: plan.instrument.clone(),
            direction: EpisodeDirection::Long,
            allocations: allocations.clone(),
            opened_at: now,
            closed_at: None,
            current_quantity: Decimal::new(12, 0),
            fingerprint: "x".into(),
        };
        let reconciliation = reconcile_tranches(&plan.tranches, &allocations);
        let review = calculate_review(&plan, &episode, &reconciliation).unwrap();
        assert_eq!(review.planned_risk, Decimal::new(50, 0));
        assert_eq!(review.actual_risk, Decimal::new(84, 0));
        assert_eq!(review.entry_slippage, Decimal::new(2, 0));
        assert!(review.flags.contains(&ReviewFlag::ExcessRisk));
        assert!(review.flags.contains(&ReviewFlag::UnplannedSize));
    }
}
