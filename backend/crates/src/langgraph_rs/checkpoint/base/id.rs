use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

static LAST_MILLIS: AtomicU64 = AtomicU64::new(0);
static ID_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static LAST_V6_TIMESTAMP: AtomicU64 = AtomicU64::new(0);

const UUID_EPOCH_OFFSET_100NS: u64 = 0x01B21DD213814000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointIdStrategy {
    #[default]
    LegacyMonotonic,
    Uuid6,
}

pub fn now_timestamp_string() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", now.as_secs(), now.subsec_nanos())
}

pub fn next_checkpoint_id(clock_seq: i64) -> String {
    let millis = next_monotonic_millis();
    let seq = ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{millis:020}-{seq:010}-{clock_seq:+06}")
}

pub fn next_checkpoint_id_with_strategy(clock_seq: i64, strategy: CheckpointIdStrategy) -> String {
    match strategy {
        CheckpointIdStrategy::LegacyMonotonic => next_checkpoint_id(clock_seq),
        CheckpointIdStrategy::Uuid6 => next_uuid6_checkpoint_id(clock_seq),
    }
}

pub fn next_uuid6_checkpoint_id(clock_seq: i64) -> String {
    let timestamp = next_uuid6_timestamp_100ns();

    let time_high_and_mid = (timestamp >> 12) & 0xFFFF_FFFF_FFFF;
    let time_low = timestamp & 0x0FFF;

    let mut uuid_int = 0_u128;
    uuid_int |= u128::from(time_high_and_mid) << 80;
    uuid_int |= u128::from(time_low) << 64;
    uuid_int |= u128::from((clock_seq as i128 & 0x3FFF) as u16) << 48;

    // Use random node bits for parity with Python uuid6 fallback behavior.
    let node = Uuid::new_v4().as_u128() & 0xFFFF_FFFF_FFFF;
    uuid_int |= node;

    // Set RFC4122 variant bits (10xx....).
    uuid_int &= !(u128::from(0xC000_u16) << 48);
    uuid_int |= u128::from(0x8000_u16) << 48;
    // Set UUID version bits to v6.
    uuid_int &= !(u128::from(0xF_u8) << 76);
    uuid_int |= u128::from(6_u8) << 76;

    Uuid::from_u128(uuid_int).hyphenated().to_string()
}

fn next_monotonic_millis() -> u64 {
    let now_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    loop {
        let last = LAST_MILLIS.load(Ordering::Relaxed);
        let next = if now_millis > last {
            now_millis
        } else {
            last.saturating_add(1)
        };

        if LAST_MILLIS
            .compare_exchange(last, next, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            return next;
        }
    }
}

fn next_uuid6_timestamp_100ns() -> u64 {
    let now_100ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .saturating_div(100) as u64
        + UUID_EPOCH_OFFSET_100NS;

    loop {
        let last = LAST_V6_TIMESTAMP.load(Ordering::Relaxed);
        let next = if now_100ns > last {
            now_100ns
        } else {
            last.saturating_add(1)
        };

        if LAST_V6_TIMESTAMP
            .compare_exchange(last, next, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            return next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CheckpointIdStrategy, next_checkpoint_id, next_checkpoint_id_with_strategy,
        next_uuid6_checkpoint_id,
    };

    #[test]
    fn checkpoint_ids_are_monotonic() {
        let a = next_checkpoint_id(0);
        let b = next_checkpoint_id(0);
        assert!(b > a);
    }

    #[test]
    fn uuid6_ids_are_uuid_shaped() {
        let id = next_uuid6_checkpoint_id(1);
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|ch| *ch == '-').count(), 4);
    }

    #[test]
    fn id_strategy_routes_generation() {
        let legacy = next_checkpoint_id_with_strategy(0, CheckpointIdStrategy::LegacyMonotonic);
        let uuid6 = next_checkpoint_id_with_strategy(0, CheckpointIdStrategy::Uuid6);
        assert_ne!(legacy, uuid6);
        assert_eq!(uuid6.len(), 36);
    }
}
