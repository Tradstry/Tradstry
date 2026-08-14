use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::types::{FillAllocation, PlanTranche};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrancheAllocation {
    pub tranche_id: String,
    pub transaction_id: String,
    pub quantity: Decimal,
    pub planned_price: Decimal,
    pub actual_price: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrancheReconciliation {
    pub allocations: Vec<TrancheAllocation>,
    pub missing_planned_quantity: Decimal,
    pub unplanned_actual_quantity: Decimal,
}

pub fn reconcile_tranches(
    tranches: &[PlanTranche],
    entry_fills: &[FillAllocation],
) -> TrancheReconciliation {
    let mut planned: Vec<(usize, Decimal)> = tranches
        .iter()
        .enumerate()
        .map(|(index, tranche)| (index, tranche.quantity.max(Decimal::ZERO)))
        .collect();
    let mut actual: Vec<(usize, Decimal)> = entry_fills
        .iter()
        .enumerate()
        .map(|(index, fill)| (index, fill.quantity.max(Decimal::ZERO)))
        .collect();
    let mut candidates = Vec::new();
    for (tranche_index, tranche) in tranches.iter().enumerate() {
        for (fill_index, fill) in entry_fills.iter().enumerate() {
            candidates.push((
                (tranche.entry_price - fill.price).abs(),
                fill.executed_at,
                fill.transaction_id.as_str(),
                tranche.order,
                tranche_index,
                fill_index,
            ));
        }
    }
    candidates.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(right.2))
            .then_with(|| left.3.cmp(&right.3))
    });

    let mut allocations = Vec::new();
    for (_, _, _, _, tranche_index, fill_index) in candidates {
        let quantity = planned[tranche_index].1.min(actual[fill_index].1);
        if quantity <= Decimal::ZERO {
            continue;
        }
        planned[tranche_index].1 -= quantity;
        actual[fill_index].1 -= quantity;
        allocations.push(TrancheAllocation {
            tranche_id: tranches[tranche_index].id.clone(),
            transaction_id: entry_fills[fill_index].transaction_id.clone(),
            quantity,
            planned_price: tranches[tranche_index].entry_price,
            actual_price: entry_fills[fill_index].price,
        });
    }

    TrancheReconciliation {
        allocations,
        missing_planned_quantity: planned.into_iter().map(|(_, quantity)| quantity).sum(),
        unplanned_actual_quantity: actual.into_iter().map(|(_, quantity)| quantity).sum(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::service::trade_review::types::FillRole;

    #[test]
    fn conserves_quantity_and_uses_closest_price() {
        let tranches = vec![
            PlanTranche {
                id: "low".into(),
                order: 0,
                quantity: Decimal::new(5, 0),
                entry_price: Decimal::new(100, 0),
            },
            PlanTranche {
                id: "high".into(),
                order: 1,
                quantity: Decimal::new(5, 0),
                entry_price: Decimal::new(110, 0),
            },
        ];
        let fills = vec![FillAllocation {
            transaction_id: "fill".into(),
            role: FillRole::Entry,
            quantity: Decimal::new(7, 0),
            price: Decimal::new(109, 0),
            fee: Decimal::ZERO,
            executed_at: Utc.with_ymd_and_hms(2026, 8, 14, 12, 0, 0).unwrap(),
        }];
        let result = reconcile_tranches(&tranches, &fills);
        assert_eq!(result.allocations[0].tranche_id, "high");
        assert_eq!(
            result
                .allocations
                .iter()
                .map(|a| a.quantity)
                .sum::<Decimal>(),
            Decimal::new(7, 0)
        );
        assert_eq!(result.missing_planned_quantity, Decimal::new(3, 0));
        assert_eq!(result.unplanned_actual_quantity, Decimal::ZERO);
    }
}
