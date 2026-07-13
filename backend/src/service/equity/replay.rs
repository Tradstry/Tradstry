//! Pure equity replay: equity(t) = cash(t) + Σ shares(t) × close(t). No DB, no network.
//!
//! Cash is driven by `amount` alone — SnapTrade's cash impact of the transaction — never
//! re-derived from price × units per type, since brokers pass their own type strings
//! through and we can't confirm their signs. `transaction_type` only decides whether to
//! count shares and whether it's funding; unrecognized types still hit cash and land in
//! `ReplayHealth::unclassified_types`.

use std::collections::HashMap;

use chrono::{Duration, NaiveDate};

#[derive(Debug, Clone)]
pub struct Txn {
    pub settlement_date: NaiveDate,
    pub transaction_type: String,
    pub symbol: Option<String>,
    pub units: f64,
    pub amount: f64,
    pub is_option: bool,
    pub is_foreign_currency: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxnKind {
    Buy,
    Sell,
    Funding,
    Other,
}

/// `DeriveFromType` is the escape hatch for a broker that reports `amount` as an
/// unsigned magnitude; the drift check surfaces that case loudly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CashSignConvention {
    #[default]
    AmountIsSigned,
    DeriveFromType,
}

#[derive(Debug, Clone, Copy)]
pub struct SplitEvent {
    pub date: NaiveDate,
    pub ratio: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EquityPoint {
    pub date: NaiveDate,
    pub cash: f64,
    pub positions_value: f64,
    pub equity: f64,
    pub net_contributions: f64,
    pub funding_adjusted_equity: f64,
}

/// Everything that could make the curve wrong; surfaced to the UI, because a silently
/// wrong equity chart is worse than no chart.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReplayHealth {
    pub unclassified_types: Vec<String>,
    pub excluded_option_txns: usize,
    pub foreign_currency_txns: usize,
    pub missing_price_days: usize,
}

pub struct PriceLookup<'a>(pub &'a HashMap<(String, NaiveDate), f64>);

impl PriceLookup<'_> {
    /// Carries the last close forward across weekends and holidays.
    pub fn close_on_or_before(&self, symbol: &str, date: NaiveDate) -> Option<f64> {
        let mut d = date;
        for _ in 0..366 {
            if let Some(close) = self.0.get(&(symbol.to_string(), d)) {
                return Some(*close);
            }
            d -= Duration::days(1);
        }
        None
    }
}

/// Substring match: brokers pass their own type strings through.
pub fn classify(transaction_type: &str) -> TxnKind {
    let t = transaction_type.trim().to_ascii_uppercase();
    if t.contains("BUY") {
        return TxnKind::Buy;
    }
    if t.contains("SELL") {
        return TxnKind::Sell;
    }
    if t.contains("DEPOSIT")
        || t.contains("WITHDRAW")
        || t.contains("CONTRIBUTION")
        || t.contains("TRANSFER")
    {
        return TxnKind::Funding;
    }
    TxnKind::Other
}

pub fn cash_delta(t: &Txn, conv: CashSignConvention) -> f64 {
    match conv {
        CashSignConvention::AmountIsSigned => t.amount,
        CashSignConvention::DeriveFromType => match classify(&t.transaction_type) {
            TxnKind::Buy => -t.amount.abs(),
            _ => t.amount.abs(),
        },
    }
}

/// Restates broker share counts — which are in the terms of their trade date — into the
/// *current* terms the price series uses, by applying every split since the trade.
///
/// The price feed is split-adjusted, so a 1:100 reverse split makes old closes look 100x
/// bigger (WOK shows $912,000). Valuing raw shares against those prices double-counts the
/// split; capping at the valuation date leaves shares and prices in different bases.
pub fn apply_splits(shares: f64, trade_date: NaiveDate, splits: &[SplitEvent]) -> f64 {
    let mut out = shares;
    for s in splits {
        if s.date > trade_date {
            out *= s.ratio;
        }
    }
    out
}

/// Per-lot rather than a running total: split adjustment depends on the acquisition date.
#[derive(Debug, Clone, Copy)]
struct Lot {
    date: NaiveDate,
    units: f64,
}

pub fn replay(
    txns: &[Txn],
    splits: &HashMap<String, Vec<SplitEvent>>,
    prices: &PriceLookup,
    conv: CashSignConvention,
) -> (Vec<EquityPoint>, ReplayHealth) {
    let mut health = ReplayHealth::default();
    if txns.is_empty() {
        return (Vec::new(), health);
    }

    let mut sorted: Vec<&Txn> = txns.iter().collect();
    sorted.sort_by_key(|t| t.settlement_date);

    let start = sorted[0].settlement_date;
    let end = sorted
        .last()
        .map(|t| t.settlement_date)
        .unwrap_or(start)
        .max(chrono::Utc::now().date_naive());

    let mut cash = 0.0_f64;
    let mut net_contributions = 0.0_f64;
    let mut lots: HashMap<String, Vec<Lot>> = HashMap::new();
    let mut seen_unclassified: Vec<String> = Vec::new();

    let mut points = Vec::new();
    let mut idx = 0usize;
    let mut day = start;

    while day <= end {
        while idx < sorted.len() && sorted[idx].settlement_date == day {
            let t = sorted[idx];
            let kind = classify(&t.transaction_type);

            cash += cash_delta(t, conv);

            if kind == TxnKind::Funding {
                net_contributions += cash_delta(t, conv);
            }

            if t.is_foreign_currency {
                health.foreign_currency_txns += 1;
            }

            match kind {
                TxnKind::Buy | TxnKind::Sell => {
                    if t.is_option {
                        health.excluded_option_txns += 1;
                    } else if let Some(sym) = &t.symbol {
                        // Take the magnitude and re-sign from the type: SnapTrade already
                        // signs `units` (a sell arrives as -8), so negating a sell would
                        // *add* shares and the position would never close out.
                        let signed = if kind == TxnKind::Buy {
                            t.units.abs()
                        } else {
                            -t.units.abs()
                        };
                        lots.entry(sym.clone()).or_default().push(Lot {
                            date: day,
                            units: signed,
                        });
                    }
                }
                TxnKind::Other => {
                    let ty = t.transaction_type.trim().to_string();
                    if !ty.is_empty() && !seen_unclassified.contains(&ty) {
                        seen_unclassified.push(ty);
                    }
                }
                TxnKind::Funding => {}
            }

            idx += 1;
        }

        let mut positions_value = 0.0_f64;
        for (symbol, sym_lots) in &lots {
            let empty: Vec<SplitEvent> = Vec::new();
            let sym_splits = splits.get(symbol).unwrap_or(&empty);

            let shares: f64 = sym_lots
                .iter()
                .filter(|l| l.date <= day)
                .map(|l| apply_splits(l.units, l.date, sym_splits))
                .sum();

            if shares.abs() < f64::EPSILON {
                continue;
            }

            match prices.close_on_or_before(symbol, day) {
                Some(close) => positions_value += shares * close,
                None => health.missing_price_days += 1,
            }
        }

        let equity = cash + positions_value;
        points.push(EquityPoint {
            date: day,
            cash,
            positions_value,
            equity,
            net_contributions,
            funding_adjusted_equity: equity - net_contributions,
        });

        day += Duration::days(1);
    }

    health.unclassified_types = seen_unclassified;
    (points, health)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn base() -> Txn {
        Txn {
            settlement_date: d("2026-01-01"),
            transaction_type: "BUY".into(),
            symbol: None,
            units: 0.0,
            amount: 0.0,
            is_option: false,
            is_foreign_currency: false,
        }
    }

    fn txn_funding(date: &str, amount: f64) -> Txn {
        Txn {
            settlement_date: d(date),
            transaction_type: "DEPOSIT".into(),
            amount,
            ..base()
        }
    }

    fn txn_buy(date: &str, symbol: &str, units: f64, amount: f64) -> Txn {
        Txn {
            settlement_date: d(date),
            transaction_type: "BUY".into(),
            symbol: Some(symbol.into()),
            units,
            amount,
            ..base()
        }
    }

    fn txn_other(date: &str, ty: &str, amount: f64) -> Txn {
        Txn {
            settlement_date: d(date),
            transaction_type: ty.into(),
            amount,
            ..base()
        }
    }

    fn txn_option_buy(date: &str, symbol: &str, units: f64, amount: f64) -> Txn {
        Txn {
            settlement_date: d(date),
            transaction_type: "BUY".into(),
            symbol: Some(symbol.into()),
            units,
            amount,
            is_option: true,
            ..base()
        }
    }

    fn prices(rows: &[(&str, &str, f64)]) -> HashMap<(String, NaiveDate), f64> {
        rows.iter()
            .map(|(s, date, c)| ((s.to_string(), d(date)), *c))
            .collect()
    }

    fn empty_prices() -> HashMap<(String, NaiveDate), f64> {
        HashMap::new()
    }

    fn no_splits() -> HashMap<String, Vec<SplitEvent>> {
        HashMap::new()
    }

    fn splits_for(rows: &[(&str, &str, f64)]) -> HashMap<String, Vec<SplitEvent>> {
        let mut m: HashMap<String, Vec<SplitEvent>> = HashMap::new();
        for (s, date, ratio) in rows {
            m.entry(s.to_string()).or_default().push(SplitEvent {
                date: d(date),
                ratio: *ratio,
            });
        }
        m
    }

    #[test]
    fn classify_maps_broker_strings_to_kinds() {
        assert_eq!(classify("BUY"), TxnKind::Buy);
        assert_eq!(classify("Buy to Open"), TxnKind::Buy);
        assert_eq!(classify("SELL"), TxnKind::Sell);
        assert_eq!(classify("DEPOSIT"), TxnKind::Funding);
        assert_eq!(classify("Withdrawal"), TxnKind::Funding);
        assert_eq!(classify("CONTRIBUTION"), TxnKind::Funding);
        assert_eq!(classify("DIVIDEND"), TxnKind::Other);
        assert_eq!(classify("SOME_WEIRD_BROKER_TYPE"), TxnKind::Other);
    }

    #[test]
    fn cash_delta_uses_signed_amount_by_default() {
        let t = txn_buy("2026-01-05", "AAPL", 10.0, -500.0);
        assert_eq!(cash_delta(&t, CashSignConvention::AmountIsSigned), -500.0);
        let m = txn_buy("2026-01-05", "AAPL", 10.0, 500.0);
        assert_eq!(cash_delta(&m, CashSignConvention::DeriveFromType), -500.0);
    }

    #[test]
    fn shares_are_restated_into_current_terms_by_every_split_since_the_trade() {
        let s = [SplitEvent {
            date: d("2024-06-01"),
            ratio: 2.0,
        }];
        // Bought before the split: 10 old shares are 20 in today's terms — and the price
        // series is quoted in today's terms too, including for dates before the split.
        assert_eq!(apply_splits(10.0, d("2024-01-01"), &s), 20.0);
        assert_eq!(apply_splits(10.0, d("2024-05-01"), &s), 20.0);
        // Bought after: the broker already reported post-split units.
        assert_eq!(apply_splits(10.0, d("2024-06-02"), &s), 10.0);
    }

    /// A reverse split scales shares DOWN while the feed scales the matching historical
    /// closes UP, so position value must stay sane instead of exploding.
    #[test]
    fn a_reverse_split_does_not_inflate_position_value() {
        let txns = vec![txn_buy("2025-09-01", "WOK", 100.0, -200.0)];
        let sp = splits_for(&[("WOK", "2025-10-01", 0.01)]);
        // Split-adjusted close: a pre-split $2 share shows as $200 in current terms.
        let p = prices(&[("WOK", "2025-09-15", 200.0)]);
        let (pts, _) = replay(&txns, &sp, &PriceLookup(&p), Default::default());
        let sep = pts.iter().find(|p| p.date == d("2025-09-15")).unwrap();
        // 100 raw shares -> 1 in current terms, at $200 = $200. Not $20,000.
        assert_eq!(sep.positions_value, 200.0);
    }

    #[test]
    fn deposit_then_buy_then_price_move() {
        let txns = vec![
            txn_funding("2026-01-01", 1000.0),
            txn_buy("2026-01-05", "AAPL", 10.0, -500.0),
        ];
        let p = prices(&[("AAPL", "2026-01-05", 50.0), ("AAPL", "2026-01-06", 55.0)]);
        let (pts, _h) = replay(&txns, &no_splits(), &PriceLookup(&p), Default::default());

        let jan6 = pts.iter().find(|p| p.date == d("2026-01-06")).unwrap();
        assert_eq!(jan6.cash, 500.0);
        assert_eq!(jan6.positions_value, 550.0);
        assert_eq!(jan6.equity, 1050.0);
        assert_eq!(jan6.net_contributions, 1000.0);
        assert_eq!(jan6.funding_adjusted_equity, 50.0);
    }

    #[test]
    fn a_deposit_does_not_look_like_profit() {
        let txns = vec![
            txn_funding("2026-01-01", 1000.0),
            txn_funding("2026-02-01", 5000.0),
        ];
        let ep = empty_prices();
        let (pts, _) = replay(&txns, &no_splits(), &PriceLookup(&ep), Default::default());
        let feb = pts.iter().find(|p| p.date == d("2026-02-01")).unwrap();
        assert_eq!(feb.equity, 6000.0);
        assert_eq!(feb.funding_adjusted_equity, 0.0);
    }

    #[test]
    fn split_doubles_historical_share_count() {
        let txns = vec![txn_buy("2026-01-02", "NVDA", 10.0, -1000.0)];
        let sp = splits_for(&[("NVDA", "2026-06-01", 2.0)]);
        let p = prices(&[("NVDA", "2026-07-01", 60.0)]);
        let (pts, _) = replay(&txns, &sp, &PriceLookup(&p), Default::default());
        let jul = pts.iter().find(|p| p.date == d("2026-07-01")).unwrap();
        assert_eq!(jul.positions_value, 1200.0);
    }

    #[test]
    fn unknown_types_still_hit_cash_and_are_reported() {
        let txns = vec![txn_other("2026-01-01", "SOME_BROKER_THING", -25.0)];
        let ep = empty_prices();
        let (pts, h) = replay(&txns, &no_splits(), &PriceLookup(&ep), Default::default());
        assert_eq!(pts.first().unwrap().cash, -25.0);
        assert!(
            h.unclassified_types
                .contains(&"SOME_BROKER_THING".to_string())
        );
    }

    fn txn_sell(date: &str, symbol: &str, units: f64, amount: f64) -> Txn {
        Txn {
            settlement_date: d(date),
            transaction_type: "SELL".into(),
            symbol: Some(symbol.into()),
            units,
            amount,
            ..base()
        }
    }

    /// SnapTrade signs `units` (a sell arrives as -8). Whichever convention the broker
    /// uses, selling everything you bought must leave zero shares — not double them.
    #[test]
    fn selling_closes_the_position_under_either_units_convention() {
        let p = prices(&[("PRCH", "2026-01-10", 15.0)]);

        for sell_units in [-8.0, 8.0] {
            let txns = vec![
                txn_buy("2026-01-05", "PRCH", 8.0, -121.84),
                txn_sell("2026-01-06", "PRCH", sell_units, 116.16),
            ];
            let (pts, _) = replay(&txns, &no_splits(), &PriceLookup(&p), Default::default());
            let last = pts.last().unwrap();
            assert_eq!(
                last.positions_value, 0.0,
                "sell_units={sell_units} left an open position"
            );
            assert!((last.equity - (116.16 - 121.84)).abs() < 1e-9);
        }
    }

    #[test]
    fn option_txns_do_not_create_share_positions_but_still_move_cash() {
        let txns = vec![txn_option_buy("2026-01-05", "AAPL", 1.0, -300.0)];
        let ep = empty_prices();
        let (pts, h) = replay(&txns, &no_splits(), &PriceLookup(&ep), Default::default());
        let first = pts.first().unwrap();
        assert_eq!(first.cash, -300.0);
        assert_eq!(first.positions_value, 0.0);
        assert_eq!(h.excluded_option_txns, 1);
    }
}
