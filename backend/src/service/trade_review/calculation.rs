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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopBoundaryPosition {
    BeforePlannedStop,
    AtOrNearPlannedStop,
    BeyondPlannedStop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewUnavailableReason {
    InvalidPlannedStop,
    PositionStillOpen,
    IncompleteExitQuantity,
    NoExitFills,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitStopComparison {
    pub transaction_id: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    pub executed_at: String,
    /// Positive means the exit remained on the position's safe side of the
    /// planned stop; negative means it printed beyond the planned boundary.
    pub distance_from_stop: Decimal,
    pub distance_from_stop_r: Decimal,
    pub boundary_position: StopBoundaryPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopOutcome {
    pub planned_stop: Decimal,
    /// Exits within this fraction of planned per-unit risk are described as
    /// at-or-near the stop. Persisting the tolerance keeps the label auditable.
    pub near_tolerance_r: Decimal,
    pub before_count: usize,
    pub near_count: usize,
    pub beyond_count: usize,
    pub exits: Vec<ExitStopComparison>,
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
    pub entry_fees: Decimal,
    pub exit_fees: Decimal,
    pub total_fees: Decimal,
    pub gross_realized_pnl: Option<Decimal>,
    pub realized_pnl: Option<Decimal>,
    pub realized_r: Option<Decimal>,
    pub realized_r_unavailable_reason: Option<ReviewUnavailableReason>,
    pub planned_r_multiple: Option<Decimal>,
    pub actual_r_multiple: Option<Decimal>,
    pub stop_outcome: Option<StopOutcome>,
    pub stop_outcome_unavailable_reason: Option<ReviewUnavailableReason>,
    pub flags: Vec<ReviewFlag>,
}

const NEAR_STOP_TOLERANCE_R: Decimal = Decimal::from_parts(5, 0, 0, false, 2);

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
    let valid_stop = plan.stop_loss > Decimal::ZERO
        && match plan.direction {
            EpisodeDirection::Long => plan.stop_loss < planned_entry,
            EpisodeDirection::Short => plan.stop_loss > planned_entry,
        };
    let planned_risk = if valid_stop {
        (planned_entry - plan.stop_loss).abs() * planned_quantity * multiplier
    } else {
        Decimal::ZERO
    };
    let actual_stop_valid = plan.stop_loss > Decimal::ZERO
        && match plan.direction {
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
    let exit_quantity: Decimal = episode.exit_allocations().map(|fill| fill.quantity).sum();
    let entry_fees: Decimal = episode.entry_allocations().map(|fill| fill.fee).sum();
    let exit_fees: Decimal = episode.exit_allocations().map(|fill| fill.fee).sum();
    let total_fees = entry_fees + exit_fees;
    let is_closed = episode.closed_at.is_some();
    let has_complete_exits =
        is_closed && exit_quantity == actual_quantity && episode.current_quantity == Decimal::ZERO;
    let realized_unavailable_reason = if !is_closed {
        Some(ReviewUnavailableReason::PositionStillOpen)
    } else if exit_quantity <= Decimal::ZERO {
        Some(ReviewUnavailableReason::NoExitFills)
    } else if !has_complete_exits {
        Some(ReviewUnavailableReason::IncompleteExitQuantity)
    } else if !valid_stop || planned_risk <= Decimal::ZERO {
        Some(ReviewUnavailableReason::InvalidPlannedStop)
    } else {
        None
    };
    let gross_realized_pnl = has_complete_exits.then(|| {
        let exit_value: Decimal = episode
            .exit_allocations()
            .map(|fill| fill.price * fill.quantity)
            .sum();
        let exit_entry_basis = actual_entry * exit_quantity;
        (exit_value - exit_entry_basis) * direction_sign * multiplier
    });
    let realized_pnl = gross_realized_pnl.map(|gross| gross - total_fees);
    let realized_r = realized_pnl
        .filter(|_| realized_unavailable_reason.is_none())
        .map(|pnl| pnl / planned_risk);
    let stop_outcome_unavailable_reason = if !valid_stop || planned_risk <= Decimal::ZERO {
        Some(ReviewUnavailableReason::InvalidPlannedStop)
    } else if !is_closed {
        Some(ReviewUnavailableReason::PositionStillOpen)
    } else if exit_quantity <= Decimal::ZERO {
        Some(ReviewUnavailableReason::NoExitFills)
    } else if !has_complete_exits {
        Some(ReviewUnavailableReason::IncompleteExitQuantity)
    } else {
        None
    };
    let stop_outcome = stop_outcome_unavailable_reason.is_none().then(|| {
        let risk_price_distance = (planned_entry - plan.stop_loss).abs();
        let exits = episode
            .exit_allocations()
            .map(|fill| {
                let distance_from_stop = (fill.price - plan.stop_loss) * direction_sign;
                let distance_from_stop_r = distance_from_stop / risk_price_distance;
                let boundary_position = if distance_from_stop_r.abs() <= NEAR_STOP_TOLERANCE_R {
                    StopBoundaryPosition::AtOrNearPlannedStop
                } else if distance_from_stop_r > Decimal::ZERO {
                    StopBoundaryPosition::BeforePlannedStop
                } else {
                    StopBoundaryPosition::BeyondPlannedStop
                };
                ExitStopComparison {
                    transaction_id: fill.transaction_id.clone(),
                    quantity: fill.quantity,
                    price: fill.price,
                    fee: fill.fee,
                    executed_at: fill.executed_at.to_rfc3339(),
                    distance_from_stop,
                    distance_from_stop_r,
                    boundary_position,
                }
            })
            .collect::<Vec<_>>();
        StopOutcome {
            planned_stop: plan.stop_loss,
            near_tolerance_r: NEAR_STOP_TOLERANCE_R,
            before_count: exits
                .iter()
                .filter(|exit| exit.boundary_position == StopBoundaryPosition::BeforePlannedStop)
                .count(),
            near_count: exits
                .iter()
                .filter(|exit| exit.boundary_position == StopBoundaryPosition::AtOrNearPlannedStop)
                .count(),
            beyond_count: exits
                .iter()
                .filter(|exit| exit.boundary_position == StopBoundaryPosition::BeyondPlannedStop)
                .count(),
            exits,
        }
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
        entry_fees,
        exit_fees,
        total_fees,
        gross_realized_pnl,
        realized_pnl,
        realized_r,
        realized_r_unavailable_reason: realized_unavailable_reason,
        planned_r_multiple: realized_r,
        actual_r_multiple: realized_pnl
            .filter(|_| actual_risk > Decimal::ZERO)
            .map(|pnl| pnl / actual_risk),
        stop_outcome,
        stop_outcome_unavailable_reason,
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
        assert_eq!(
            review.realized_r_unavailable_reason,
            Some(ReviewUnavailableReason::PositionStillOpen)
        );
        assert_eq!(
            review.stop_outcome_unavailable_reason,
            Some(ReviewUnavailableReason::PositionStillOpen)
        );
    }

    #[test]
    fn calculates_fee_aware_realized_r_and_exit_stop_evidence() {
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
        let allocations = vec![
            FillAllocation {
                transaction_id: "entry".into(),
                role: FillRole::Entry,
                quantity: Decimal::new(10, 0),
                price: Decimal::new(100, 0),
                fee: Decimal::new(1, 0),
                executed_at: now,
            },
            FillAllocation {
                transaction_id: "exit-before".into(),
                role: FillRole::Exit,
                quantity: Decimal::new(4, 0),
                price: Decimal::new(96, 0),
                fee: Decimal::new(40, 2),
                executed_at: now + chrono::Duration::minutes(1),
            },
            FillAllocation {
                transaction_id: "exit-near".into(),
                role: FillRole::Exit,
                quantity: Decimal::new(3, 0),
                price: Decimal::new(952, 1),
                fee: Decimal::new(30, 2),
                executed_at: now + chrono::Duration::minutes(2),
            },
            FillAllocation {
                transaction_id: "exit-beyond".into(),
                role: FillRole::Exit,
                quantity: Decimal::new(3, 0),
                price: Decimal::new(94, 0),
                fee: Decimal::new(30, 2),
                executed_at: now + chrono::Duration::minutes(3),
            },
        ];
        let episode = TradeEpisodeDraft {
            instrument: plan.instrument.clone(),
            direction: EpisodeDirection::Long,
            allocations: allocations.clone(),
            opened_at: now,
            closed_at: Some(now + chrono::Duration::minutes(3)),
            current_quantity: Decimal::ZERO,
            fingerprint: "x".into(),
        };
        let reconciliation = reconcile_tranches(&plan.tranches, &allocations);
        let review = calculate_review(&plan, &episode, &reconciliation).unwrap();

        assert_eq!(review.planned_risk, Decimal::new(50, 0));
        assert_eq!(review.gross_realized_pnl, Some(Decimal::new(-484, 1)));
        assert_eq!(review.total_fees, Decimal::new(2, 0));
        assert_eq!(review.realized_pnl, Some(Decimal::new(-504, 1)));
        assert_eq!(review.realized_r, Some(Decimal::new(-1008, 3)));
        assert_eq!(review.realized_r_unavailable_reason, None);
        let outcome = review.stop_outcome.expect("closed trade has stop evidence");
        assert_eq!(outcome.before_count, 1);
        assert_eq!(outcome.near_count, 1);
        assert_eq!(outcome.beyond_count, 1);
        assert_eq!(
            outcome.exits[2].boundary_position,
            StopBoundaryPosition::BeyondPlannedStop
        );
        assert_eq!(outcome.exits[2].distance_from_stop_r, Decimal::new(-2, 1));
    }

    #[test]
    fn classifies_short_exits_direction_aware() {
        let now = Utc.with_ymd_and_hms(2026, 8, 14, 12, 0, 0).unwrap();
        let plan = PlanSnapshot {
            plan_id: "p".into(),
            workspace_id: "w".into(),
            instrument: ExecutionInstrument::Equity {
                symbol: "AVGO".into(),
            },
            direction: EpisodeDirection::Short,
            stop_loss: Decimal::new(105, 0),
            created_at: now,
            active_at_episode_open: true,
            tranches: vec![PlanTranche {
                id: "t".into(),
                order: 0,
                quantity: Decimal::new(2, 0),
                entry_price: Decimal::new(100, 0),
            }],
        };
        let allocations = vec![
            FillAllocation {
                transaction_id: "entry".into(),
                role: FillRole::Entry,
                quantity: Decimal::new(2, 0),
                price: Decimal::new(100, 0),
                fee: Decimal::ZERO,
                executed_at: now,
            },
            FillAllocation {
                transaction_id: "cover".into(),
                role: FillRole::Exit,
                quantity: Decimal::new(2, 0),
                price: Decimal::new(106, 0),
                fee: Decimal::ZERO,
                executed_at: now + chrono::Duration::minutes(1),
            },
        ];
        let episode = TradeEpisodeDraft {
            instrument: plan.instrument.clone(),
            direction: EpisodeDirection::Short,
            allocations: allocations.clone(),
            opened_at: now,
            closed_at: Some(now + chrono::Duration::minutes(1)),
            current_quantity: Decimal::ZERO,
            fingerprint: "x".into(),
        };
        let reconciliation = reconcile_tranches(&plan.tranches, &allocations);
        let review = calculate_review(&plan, &episode, &reconciliation).unwrap();
        let exit = &review.stop_outcome.expect("stop evidence").exits[0];
        assert_eq!(
            exit.boundary_position,
            StopBoundaryPosition::BeyondPlannedStop
        );
        assert_eq!(exit.distance_from_stop, Decimal::NEGATIVE_ONE);
        assert_eq!(review.realized_r, Some(Decimal::new(-12, 1)));
    }

    #[test]
    fn refuses_realized_r_when_the_planned_stop_is_invalid() {
        let now = Utc.with_ymd_and_hms(2026, 8, 14, 12, 0, 0).unwrap();
        let plan = PlanSnapshot {
            plan_id: "p".into(),
            workspace_id: "w".into(),
            instrument: ExecutionInstrument::Equity {
                symbol: "AVGO".into(),
            },
            direction: EpisodeDirection::Long,
            stop_loss: Decimal::ZERO,
            created_at: now,
            active_at_episode_open: true,
            tranches: vec![PlanTranche {
                id: "t".into(),
                order: 0,
                quantity: Decimal::ONE,
                entry_price: Decimal::new(100, 0),
            }],
        };
        let allocations = vec![
            FillAllocation {
                transaction_id: "entry".into(),
                role: FillRole::Entry,
                quantity: Decimal::ONE,
                price: Decimal::new(100, 0),
                fee: Decimal::ZERO,
                executed_at: now,
            },
            FillAllocation {
                transaction_id: "exit".into(),
                role: FillRole::Exit,
                quantity: Decimal::ONE,
                price: Decimal::new(110, 0),
                fee: Decimal::ZERO,
                executed_at: now + chrono::Duration::minutes(1),
            },
        ];
        let episode = TradeEpisodeDraft {
            instrument: plan.instrument.clone(),
            direction: EpisodeDirection::Long,
            allocations: allocations.clone(),
            opened_at: now,
            closed_at: Some(now + chrono::Duration::minutes(1)),
            current_quantity: Decimal::ZERO,
            fingerprint: "x".into(),
        };
        let reconciliation = reconcile_tranches(&plan.tranches, &allocations);
        let review = calculate_review(&plan, &episode, &reconciliation).unwrap();
        assert_eq!(review.realized_pnl, Some(Decimal::new(10, 0)));
        assert_eq!(review.realized_r, None);
        assert_eq!(
            review.realized_r_unavailable_reason,
            Some(ReviewUnavailableReason::InvalidPlannedStop)
        );
        assert_eq!(review.stop_outcome, None);
    }
}
