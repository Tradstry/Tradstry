use chrono::Duration;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::types::{PlanSnapshot, TradeEpisodeDraft};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchEvidence {
    pub time_delta_minutes: i64,
    pub planned_quantity: Decimal,
    pub actual_quantity: Decimal,
    pub quantity_delta: Decimal,
    pub planned_entry: Option<Decimal>,
    pub actual_entry: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanMatchCandidate {
    pub plan_id: String,
    pub score: Decimal,
    pub evidence: MatchEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanMatchSuggestion {
    pub candidates: Vec<PlanMatchCandidate>,
    pub ambiguous: bool,
}

pub fn suggest_plan_match(
    episode: &TradeEpisodeDraft,
    plans: &[PlanSnapshot],
) -> PlanMatchSuggestion {
    let actual_quantity = episode.entry_quantity();
    let actual_entry = weighted_actual_entry(episode);
    let mut candidates: Vec<_> = plans
        .iter()
        .filter(|plan| {
            plan.active_at_episode_open
                && plan.instrument.key() == episode.instrument.key()
                && plan.direction == episode.direction
                && plan.created_at <= episode.opened_at
                && episode.opened_at <= plan.created_at + Duration::days(7)
        })
        .map(|plan| {
            let minutes = (episode.opened_at - plan.created_at).num_minutes();
            let planned_quantity = plan.quantity();
            let quantity_delta = (planned_quantity - actual_quantity).abs();
            let planned_entry = plan.weighted_entry();
            let price_delta = planned_entry
                .map(|entry| (entry - actual_entry).abs())
                .unwrap_or(Decimal::MAX);
            let time_cost = Decimal::from(minutes.max(0)) / Decimal::from(10_080_i64);
            let quantity_cost = if planned_quantity > Decimal::ZERO {
                quantity_delta / planned_quantity
            } else {
                Decimal::MAX
            };
            let price_cost = planned_entry
                .filter(|entry| *entry > Decimal::ZERO)
                .map(|entry| price_delta / entry)
                .unwrap_or(Decimal::MAX);
            PlanMatchCandidate {
                plan_id: plan.plan_id.clone(),
                score: time_cost + quantity_cost + price_cost,
                evidence: MatchEvidence {
                    time_delta_minutes: minutes,
                    planned_quantity,
                    actual_quantity,
                    quantity_delta,
                    planned_entry,
                    actual_entry,
                },
            }
        })
        .collect();
    candidates.sort_by(|left, right| {
        left.score
            .cmp(&right.score)
            .then_with(|| left.plan_id.cmp(&right.plan_id))
    });
    let ambiguous = candidates
        .get(1)
        .zip(candidates.first())
        .map(|(second, first)| (second.score - first.score).abs() <= Decimal::new(5, 2))
        .unwrap_or(false);
    PlanMatchSuggestion {
        candidates,
        ambiguous,
    }
}

fn weighted_actual_entry(episode: &TradeEpisodeDraft) -> Decimal {
    let quantity = episode.entry_quantity();
    if quantity <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    episode
        .entry_allocations()
        .map(|allocation| allocation.price * allocation.quantity)
        .sum::<Decimal>()
        / quantity
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::service::trade_review::types::{
        EpisodeDirection, ExecutionInstrument, FillAllocation, FillRole, PlanTranche,
    };

    #[test]
    fn filters_then_ranks_candidates_deterministically() {
        let opened = Utc.with_ymd_and_hms(2026, 8, 14, 12, 0, 0).unwrap();
        let instrument = ExecutionInstrument::Equity {
            symbol: "AVGO".into(),
        };
        let episode = TradeEpisodeDraft {
            instrument: instrument.clone(),
            direction: EpisodeDirection::Long,
            allocations: vec![FillAllocation {
                transaction_id: "f".into(),
                role: FillRole::Entry,
                quantity: Decimal::new(10, 0),
                price: Decimal::new(101, 0),
                fee: Decimal::ZERO,
                executed_at: opened,
            }],
            opened_at: opened,
            closed_at: None,
            current_quantity: Decimal::new(10, 0),
            fingerprint: "x".into(),
        };
        let plan = |id: &str, symbol: &str, qty: i64| PlanSnapshot {
            plan_id: id.into(),
            workspace_id: "w".into(),
            instrument: ExecutionInstrument::Equity {
                symbol: symbol.into(),
            },
            direction: EpisodeDirection::Long,
            stop_loss: Decimal::new(95, 0),
            created_at: opened - Duration::hours(1),
            active_at_episode_open: true,
            tranches: vec![PlanTranche {
                id: "t".into(),
                order: 0,
                quantity: Decimal::new(qty, 0),
                entry_price: Decimal::new(100, 0),
            }],
        };
        let result = suggest_plan_match(
            &episode,
            &[
                plan("wrong-symbol", "MU", 10),
                plan("worse", "AVGO", 30),
                plan("best", "AVGO", 10),
            ],
        );
        assert_eq!(result.candidates.len(), 2);
        assert_eq!(result.candidates[0].plan_id, "best");
    }

    #[test]
    fn option_matching_requires_the_exact_single_leg_contract() {
        let opened = Utc.with_ymd_and_hms(2026, 8, 14, 12, 0, 0).unwrap();
        let expiration = chrono::NaiveDate::from_ymd_opt(2026, 9, 18).unwrap();
        let option = |strike| ExecutionInstrument::Option {
            underlying: "IONQ".into(),
            expiration,
            strike: Decimal::new(strike, 0),
            option_kind: "call".into(),
            multiplier: Decimal::new(100, 0),
        };
        let episode = TradeEpisodeDraft {
            instrument: option(50),
            direction: EpisodeDirection::Long,
            allocations: vec![FillAllocation {
                transaction_id: "option-fill".into(),
                role: FillRole::Entry,
                quantity: Decimal::ONE,
                price: Decimal::new(4, 0),
                fee: Decimal::ZERO,
                executed_at: opened,
            }],
            opened_at: opened,
            closed_at: None,
            current_quantity: Decimal::ONE,
            fingerprint: "option".into(),
        };
        let plan = |id: &str, instrument| PlanSnapshot {
            plan_id: id.into(),
            workspace_id: "w".into(),
            instrument,
            direction: EpisodeDirection::Long,
            stop_loss: Decimal::new(3, 0),
            created_at: opened - Duration::minutes(30),
            active_at_episode_open: true,
            tranches: vec![PlanTranche {
                id: "t".into(),
                order: 0,
                quantity: Decimal::ONE,
                entry_price: Decimal::new(4, 0),
            }],
        };
        let result = suggest_plan_match(
            &episode,
            &[plan("wrong-strike", option(55)), plan("exact", option(50))],
        );
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].plan_id, "exact");
    }
}
