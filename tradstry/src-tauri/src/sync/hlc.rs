//! Hybrid Logical Clock.
//!
//! `updated_at` comes from a Postgres trigger — the *server's* wall clock. It is
//! correct as a pull cursor and useless for ordering two clients' writes: client
//! clocks are skewed and can run backwards. An HLC advances a logical counter
//! instead of the clock, so stamps are monotonic regardless.
//!
//! Monotonicity is not uniqueness: two clients can produce the same (millis,
//! counter). `client_id` is the tiebreak, giving a total order.

const COUNTER_MAX: u16 = u16::MAX;

pub struct Hlc {
    millis: u64,
    counter: u16,
    client_id: String,
    #[cfg(test)]
    fixed_physical: Option<u64>,
}

impl Hlc {
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            millis: 0,
            counter: 0,
            client_id: client_id.into(),
            #[cfg(test)]
            fixed_physical: None,
        }
    }

    fn physical(&self) -> u64 {
        #[cfg(test)]
        if let Some(p) = self.fixed_physical {
            return p;
        }
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Next stamp. Advances the counter when the physical clock did not move
    /// forward, which covers both same-millisecond calls and backwards jumps.
    pub fn now(&mut self) -> String {
        let physical = self.physical();
        if physical > self.millis {
            self.millis = physical;
            self.counter = 0;
        } else {
            self.counter = self.counter.saturating_add(1);
            debug_assert!(self.counter < COUNTER_MAX, "HLC counter saturated");
        }
        format!("{:015}:{:05}:{}", self.millis, self.counter, self.client_id)
    }

    /// Merge a remote stamp so a peer with an advanced clock cannot make our
    /// operations look perpetually older.
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

    #[cfg(test)]
    pub fn with_physical(client_id: &str, physical: u64) -> Self {
        let mut c = Self::new(client_id);
        c.fixed_physical = Some(physical);
        c
    }

    #[cfg(test)]
    pub fn set_physical(&mut self, physical: u64) {
        self.fixed_physical = Some(physical);
    }
}

fn parse(stamp: &str) -> Option<(u64, u16)> {
    let mut parts = stamp.splitn(3, ':');
    let millis = parts.next()?.parse().ok()?;
    let counter = parts.next()?.parse().ok()?;
    Some((millis, counter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_under_backwards_clock() {
        let mut clock = Hlc::with_physical("c1", 1_000);
        let a = clock.now();
        clock.set_physical(900); // NTP correction / VM suspend
        let b = clock.now();
        assert!(b > a, "HLC must not go backwards: {a} then {b}");
    }

    #[test]
    fn counter_advances_within_same_millisecond() {
        let mut clock = Hlc::with_physical("c1", 1_000);
        let a = clock.now();
        let b = clock.now();
        assert!(
            b > a,
            "two stamps in the same ms must still order: {a} then {b}"
        );
    }

    #[test]
    fn observing_a_future_remote_pulls_us_forward() {
        let mut clock = Hlc::with_physical("c1", 1_000);
        clock.observe("000000000009999:00000:c2");
        let a = clock.now();
        assert!(
            a.as_str() > "000000000009999:00000:c2",
            "must exceed observed remote: {a}"
        );
    }

    #[test]
    fn encoding_is_lexicographically_sortable() {
        let mut clock = Hlc::with_physical("c1", 2);
        let small = clock.now();
        clock.set_physical(10);
        let big = clock.now();
        assert!(small < big, "{small} should sort before {big}");
    }

    #[test]
    fn ties_break_on_client_id() {
        let mut a = Hlc::with_physical("aaa", 5);
        let mut b = Hlc::with_physical("bbb", 5);
        assert!(
            a.now() < b.now(),
            "equal clocks must break ties deterministically"
        );
    }
}
