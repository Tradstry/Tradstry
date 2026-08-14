use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpisodeDirection {
    Long,
    Short,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillRole {
    Entry,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "asset_class", rename_all = "snake_case")]
pub enum ExecutionInstrument {
    Equity {
        symbol: String,
    },
    Option {
        underlying: String,
        expiration: NaiveDate,
        strike: Decimal,
        option_kind: String,
        multiplier: Decimal,
    },
}

impl ExecutionInstrument {
    pub fn normalized(self) -> Self {
        match self {
            Self::Equity { symbol } => Self::Equity {
                symbol: symbol.trim().to_ascii_uppercase(),
            },
            Self::Option {
                underlying,
                expiration,
                strike,
                option_kind,
                multiplier,
            } => Self::Option {
                underlying: underlying.trim().to_ascii_uppercase(),
                expiration,
                strike: strike.normalize(),
                option_kind: option_kind.trim().to_ascii_lowercase(),
                multiplier: multiplier.normalize(),
            },
        }
    }

    pub fn key(&self) -> String {
        match self {
            Self::Equity { symbol } => format!("equity:{}", symbol.trim().to_ascii_uppercase()),
            Self::Option {
                underlying,
                expiration,
                strike,
                option_kind,
                multiplier,
            } => format!(
                "option:{}:{}:{}:{}:{}",
                underlying.trim().to_ascii_uppercase(),
                expiration,
                strike.normalize(),
                option_kind.trim().to_ascii_lowercase(),
                multiplier.normalize()
            ),
        }
    }

    pub fn multiplier(&self) -> Decimal {
        match self {
            Self::Equity { .. } => Decimal::ONE,
            Self::Option { multiplier, .. } => *multiplier,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionFill {
    pub transaction_id: String,
    pub instrument: ExecutionInstrument,
    pub side: ExecutionSide,
    pub price: Decimal,
    pub quantity: Decimal,
    pub fee: Decimal,
    pub executed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FillAllocation {
    pub transaction_id: String,
    pub role: FillRole,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    pub executed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeEpisodeDraft {
    pub instrument: ExecutionInstrument,
    pub direction: EpisodeDirection,
    pub allocations: Vec<FillAllocation>,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub current_quantity: Decimal,
    pub fingerprint: String,
}

impl TradeEpisodeDraft {
    pub fn entry_quantity(&self) -> Decimal {
        self.allocations
            .iter()
            .filter(|allocation| allocation.role == FillRole::Entry)
            .map(|allocation| allocation.quantity)
            .sum()
    }

    pub fn entry_allocations(&self) -> impl Iterator<Item = &FillAllocation> {
        self.allocations
            .iter()
            .filter(|allocation| allocation.role == FillRole::Entry)
    }

    pub fn exit_allocations(&self) -> impl Iterator<Item = &FillAllocation> {
        self.allocations
            .iter()
            .filter(|allocation| allocation.role == FillRole::Exit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanTranche {
    pub id: String,
    pub order: usize,
    pub quantity: Decimal,
    pub entry_price: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanSnapshot {
    pub plan_id: String,
    pub workspace_id: String,
    pub instrument: ExecutionInstrument,
    pub direction: EpisodeDirection,
    pub stop_loss: Decimal,
    pub created_at: DateTime<Utc>,
    pub active_at_episode_open: bool,
    pub tranches: Vec<PlanTranche>,
}

impl PlanSnapshot {
    pub fn quantity(&self) -> Decimal {
        self.tranches.iter().map(|tranche| tranche.quantity).sum()
    }

    pub fn weighted_entry(&self) -> Option<Decimal> {
        let quantity = self.quantity();
        if quantity <= Decimal::ZERO {
            return None;
        }
        Some(
            self.tranches
                .iter()
                .map(|tranche| tranche.entry_price * tranche.quantity)
                .sum::<Decimal>()
                / quantity,
        )
    }
}
