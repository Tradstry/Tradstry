use std::collections::BTreeMap;

use rust_decimal::{Decimal, prelude::Signed};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::types::{
    EpisodeDirection, ExecutionFill, ExecutionSide, FillAllocation, FillRole, TradeEpisodeDraft,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EpisodeBuildError {
    #[error("fill {transaction_id} has a non-positive quantity")]
    InvalidQuantity { transaction_id: String },
    #[error("fill {transaction_id} has a non-positive price")]
    InvalidPrice { transaction_id: String },
}

pub fn build_episodes(
    fills: impl IntoIterator<Item = ExecutionFill>,
) -> Result<Vec<TradeEpisodeDraft>, EpisodeBuildError> {
    let mut by_instrument = BTreeMap::<String, Vec<ExecutionFill>>::new();
    for mut fill in fills {
        if fill.quantity <= Decimal::ZERO {
            return Err(EpisodeBuildError::InvalidQuantity {
                transaction_id: fill.transaction_id,
            });
        }
        if fill.price <= Decimal::ZERO {
            return Err(EpisodeBuildError::InvalidPrice {
                transaction_id: fill.transaction_id,
            });
        }
        fill.instrument = fill.instrument.normalized();
        by_instrument
            .entry(fill.instrument.key())
            .or_default()
            .push(fill);
    }

    let mut episodes = Vec::new();
    for (_, mut instrument_fills) in by_instrument {
        instrument_fills.sort_by(|left, right| {
            left.executed_at
                .cmp(&right.executed_at)
                .then_with(|| left.transaction_id.cmp(&right.transaction_id))
        });
        build_instrument_episodes(instrument_fills, &mut episodes);
    }
    episodes.sort_by(|left, right| {
        left.opened_at
            .cmp(&right.opened_at)
            .then_with(|| left.fingerprint.cmp(&right.fingerprint))
    });
    Ok(episodes)
}

fn build_instrument_episodes(fills: Vec<ExecutionFill>, episodes: &mut Vec<TradeEpisodeDraft>) {
    let mut active: Option<TradeEpisodeDraft> = None;
    let mut signed_position = Decimal::ZERO;

    for fill in fills {
        let fill_sign = match fill.side {
            ExecutionSide::Buy => Decimal::ONE,
            ExecutionSide::Sell => -Decimal::ONE,
        };
        let mut remaining = fill.quantity;
        let total = fill.quantity;

        if signed_position != Decimal::ZERO && signed_position.signum() != fill_sign {
            let exit_quantity = remaining.min(signed_position.abs());
            let fee = prorated_fee(fill.fee, exit_quantity, total);
            if let Some(episode) = active.as_mut() {
                episode
                    .allocations
                    .push(allocation(&fill, FillRole::Exit, exit_quantity, fee));
            }
            signed_position += fill_sign * exit_quantity;
            remaining -= exit_quantity;

            if signed_position == Decimal::ZERO {
                if let Some(mut completed) = active.take() {
                    completed.closed_at = Some(fill.executed_at);
                    completed.current_quantity = Decimal::ZERO;
                    completed.fingerprint = fingerprint(&completed);
                    episodes.push(completed);
                }
            }
        }

        if remaining > Decimal::ZERO {
            if active.is_none() {
                active = Some(TradeEpisodeDraft {
                    instrument: fill.instrument.clone(),
                    direction: if fill_sign > Decimal::ZERO {
                        EpisodeDirection::Long
                    } else {
                        EpisodeDirection::Short
                    },
                    allocations: Vec::new(),
                    opened_at: fill.executed_at,
                    closed_at: None,
                    current_quantity: Decimal::ZERO,
                    fingerprint: String::new(),
                });
            }
            let allocated_before = total - remaining;
            let fee = fill.fee - prorated_fee(fill.fee, allocated_before, total);
            if let Some(episode) = active.as_mut() {
                episode
                    .allocations
                    .push(allocation(&fill, FillRole::Entry, remaining, fee));
            }
            signed_position += fill_sign * remaining;
        }

        if let Some(episode) = active.as_mut() {
            episode.current_quantity = signed_position.abs();
        }
    }

    if let Some(mut episode) = active {
        episode.fingerprint = fingerprint(&episode);
        episodes.push(episode);
    }
}

fn allocation(
    fill: &ExecutionFill,
    role: FillRole,
    quantity: Decimal,
    fee: Decimal,
) -> FillAllocation {
    FillAllocation {
        transaction_id: fill.transaction_id.clone(),
        role,
        quantity,
        price: fill.price,
        fee,
        executed_at: fill.executed_at,
    }
}

fn prorated_fee(fee: Decimal, quantity: Decimal, total: Decimal) -> Decimal {
    if quantity == Decimal::ZERO {
        Decimal::ZERO
    } else {
        fee * quantity / total
    }
}

fn fingerprint(episode: &TradeEpisodeDraft) -> String {
    let mut digest = Sha256::new();
    digest.update(episode.instrument.key());
    digest.update(format!("|{:?}", episode.direction));
    for allocation in &episode.allocations {
        digest.update(format!(
            "|{}:{:?}:{}:{}:{}",
            allocation.transaction_id,
            allocation.role,
            allocation.quantity.normalize(),
            allocation.price.normalize(),
            allocation.fee.normalize()
        ));
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::service::trade_review::types::ExecutionInstrument;

    fn fill(
        id: &str,
        minute: u32,
        side: ExecutionSide,
        quantity: i64,
        price: i64,
    ) -> ExecutionFill {
        ExecutionFill {
            transaction_id: id.into(),
            instrument: ExecutionInstrument::Equity {
                symbol: " avgo ".into(),
            },
            side,
            price: Decimal::new(price, 0),
            quantity: Decimal::new(quantity, 0),
            fee: Decimal::new(1, 0),
            executed_at: Utc.with_ymd_and_hms(2026, 8, 14, 12, minute, 0).unwrap(),
        }
    }

    #[test]
    fn builds_long_episode_with_partial_exits() {
        let episodes = build_episodes([
            fill("b1", 0, ExecutionSide::Buy, 10, 100),
            fill("b2", 1, ExecutionSide::Buy, 5, 102),
            fill("s1", 2, ExecutionSide::Sell, 6, 103),
            fill("s2", 3, ExecutionSide::Sell, 9, 104),
        ])
        .unwrap();
        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].direction, EpisodeDirection::Long);
        assert_eq!(episodes[0].entry_quantity(), Decimal::new(15, 0));
        assert_eq!(episodes[0].current_quantity, Decimal::ZERO);
        assert!(episodes[0].closed_at.is_some());
    }

    #[test]
    fn reversal_splits_one_fill_across_adjacent_episodes() {
        let episodes = build_episodes([
            fill("b1", 0, ExecutionSide::Buy, 4, 100),
            fill("reverse", 1, ExecutionSide::Sell, 10, 99),
        ])
        .unwrap();
        assert_eq!(episodes.len(), 2);
        assert_eq!(episodes[0].current_quantity, Decimal::ZERO);
        assert_eq!(episodes[1].direction, EpisodeDirection::Short);
        assert_eq!(episodes[1].current_quantity, Decimal::new(6, 0));
        let allocated: Decimal = episodes
            .iter()
            .flat_map(|episode| episode.allocations.iter())
            .filter(|allocation| allocation.transaction_id == "reverse")
            .map(|allocation| allocation.quantity)
            .sum();
        assert_eq!(allocated, Decimal::new(10, 0));
    }

    #[test]
    fn stable_ordering_produces_stable_fingerprints() {
        let first = build_episodes([
            fill("b", 0, ExecutionSide::Buy, 1, 100),
            fill("a", 0, ExecutionSide::Buy, 1, 100),
        ])
        .unwrap();
        let second = build_episodes([
            fill("a", 0, ExecutionSide::Buy, 1, 100),
            fill("b", 0, ExecutionSide::Buy, 1, 100),
        ])
        .unwrap();
        assert_eq!(first[0].fingerprint, second[0].fingerprint);
    }
}
