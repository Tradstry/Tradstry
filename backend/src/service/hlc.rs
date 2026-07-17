//! The server's Hybrid Logical Clock.
//!
//! Every synced table resolves conflicts with last-writer-wins on `hlc`, and the desktop
//! compares stamps as plain strings: `CASE WHEN pulled_hlc > local_hlc THEN pulled ELSE local`.
//!
//! Server-authored writes (the web app's GraphQL mutations, and now the MCP write tools)
//! used to leave `hlc` at its `''` default. `'' > anything` is false, so the desktop pulled
//! the row and then kept every one of its own values — a server-side *edit* was silently
//! dropped on the desktop while looking perfectly applied in the browser. Creates survived
//! only because the desktop had no local row to prefer.
//!
//! So the server is a peer like any other: it carries a clock, stamps its writes, and
//! observes the stamps it receives so a client with a fast wall clock cannot make the
//! server's writes look perpetually older.
//!
//! The format is byte-for-byte the desktop's (`{:015}:{:05}:{client_id}`): zero-padded so
//! lexicographic order is numeric order, with `client_id` breaking ties.

use std::sync::{LazyLock, Mutex};

const COUNTER_MAX: u16 = u16::MAX;
const SERVER_CLIENT_ID: &str = "server";

pub struct Hlc {
    millis: u64,
    counter: u16,
    client_id: String,
}

impl Hlc {
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            millis: 0,
            counter: 0,
            client_id: client_id.into(),
        }
    }

    fn physical() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Next stamp. Advances the counter when the physical clock did not move forward,
    /// which covers both same-millisecond calls and a clock that jumped backwards.
    pub fn now(&mut self) -> String {
        let physical = Self::physical();
        if physical > self.millis {
            self.millis = physical;
            self.counter = 0;
        } else {
            self.counter = self.counter.saturating_add(1);
            debug_assert!(self.counter < COUNTER_MAX, "HLC counter saturated");
        }
        format!("{:015}:{:05}:{}", self.millis, self.counter, self.client_id)
    }

    /// Merge a stamp we received, so a peer with an advanced clock cannot make our writes
    /// look older than theirs forever.
    pub fn observe(&mut self, remote: &str) {
        let Some((millis, counter)) = parse(remote) else {
            return;
        };
        if millis > self.millis {
            self.millis = millis;
            self.counter = counter;
        } else if millis == self.millis && counter > self.counter {
            self.counter = counter;
        }
    }
}

fn parse(stamp: &str) -> Option<(u64, u16)> {
    let mut parts = stamp.splitn(3, ':');
    let millis = parts.next()?.parse().ok()?;
    let counter = parts.next()?.parse().ok()?;
    Some((millis, counter))
}

static SERVER: LazyLock<Mutex<Hlc>> = LazyLock::new(|| Mutex::new(Hlc::new(SERVER_CLIENT_ID)));

/// Stamp a server-authored write. Use this everywhere `""` used to be passed.
pub fn stamp() -> String {
    SERVER.lock().unwrap_or_else(|e| e.into_inner()).now()
}

/// Fold a client's stamp into the server clock. Called as client mutations arrive.
pub fn observe(remote: &str) {
    SERVER
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .observe(remote);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_are_monotonic_and_lexicographically_ordered() {
        let mut c = Hlc::new("server");
        let a = c.now();
        let b = c.now();
        assert!(b > a, "{b} must sort after {a}");
    }

    /// The whole point: a real stamp must beat the `''` that server writes used to carry,
    /// because the desktop's merge is a plain `>` on the string.
    #[test]
    fn any_stamp_beats_the_empty_default() {
        let mut c = Hlc::new("server");
        assert!(c.now().as_str() > "");
    }

    /// Zero-padding is load-bearing: the desktop compares stamps as strings, so unpadded
    /// "9…" would sort above "10…" and a later write would lose to an earlier one.
    #[test]
    fn a_later_millisecond_sorts_above_an_earlier_one_as_a_string() {
        let at_9 = format!("{:015}:{:05}:{}", 9u64, 0u16, "server");
        let at_10 = format!("{:015}:{:05}:{}", 10u64, 0u16, "server");
        assert!(at_10 > at_9, "{at_10} must sort after {at_9}");
        assert!(at_10 > format!("{:015}:{:05}:{}", 9u64, u16::MAX, "server"));
    }

    #[test]
    fn observing_a_faster_peer_lifts_our_clock_above_it() {
        let mut c = Hlc::new("server");
        let remote = "999999999999999:00007:desktop";
        c.observe(remote);
        // Our next write must win against the peer we just heard from.
        assert!(c.now().as_str() > remote);
    }

    #[test]
    fn a_malformed_stamp_is_ignored_rather_than_poisoning_the_clock() {
        let mut c = Hlc::new("server");
        c.observe("not-an-hlc");
        c.observe("");
        assert!(!c.now().is_empty());
    }
}
