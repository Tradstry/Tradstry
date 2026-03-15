use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::types::RetryPolicy;

pub fn retry_sleep_duration(policy: &RetryPolicy, attempts: u32) -> Duration {
    let scaled = policy.initial_interval_secs * policy.backoff_factor.powf((attempts - 1) as f64);
    let interval = scaled.min(policy.max_interval_secs).max(0.0);
    let jitter = if policy.jitter {
        jitter_fraction()
    } else {
        0.0
    };
    Duration::from_secs_f64((interval + jitter).max(0.0))
}

fn jitter_fraction() -> f64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos as f64) / 1_000_000_000.0
}

#[cfg(test)]
mod tests {
    use super::retry_sleep_duration;
    use crate::langgraph_rs::runtime::runner::RetryPolicy;

    #[test]
    fn clamps_backoff_to_max_interval() {
        let policy = RetryPolicy::new()
            .with_initial_interval_secs(1.0)
            .with_backoff_factor(4.0)
            .with_max_interval_secs(2.0)
            .with_jitter(false);
        let sleep = retry_sleep_duration(&policy, 3);
        assert_eq!(sleep.as_secs_f64(), 2.0);
    }
}
