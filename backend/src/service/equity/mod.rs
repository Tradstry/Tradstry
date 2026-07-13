//! Reconstructs account equity history from broker transactions, because
//! `accounts.total_value` is overwritten on every sync and its history is lost.

pub mod prices;
pub mod rebuild;
pub mod replay;
pub mod schedule;

/// Bump whenever the replay math changes. Stored curves carry the version that produced
/// them, so a bump makes every account rebuild on next read rather than serving numbers
/// the current code would never produce.
///
/// 1: initial. 2: sells re-signed from type (SnapTrade signs `units`, so sells were adding
/// shares). 3: shares restated into the price feed's split-adjusted terms.
pub const REPLAY_VERSION: i32 = 3;
