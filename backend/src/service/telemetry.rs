//! Logging, tracing, and error reporting setup.
//!
//! `tracing` replaces `env_logger` as the sink, but the ~160 existing `log::`
//! call sites keep working: `SubscriberInitExt::init` installs a `log`
//! compatibility bridge, so those records arrive here as tracing events. New
//! code should reach for `tracing::` and, more importantly, for spans — a field
//! set once on a span is attached to every event inside it, which is what makes
//! "show me everything that happened for this user's sync" answerable.

use sentry::ClientInitGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

/// Initialises error reporting and the tracing subscriber.
///
/// The returned guard flushes buffered events to Sentry when dropped, so it has
/// to live for the whole process — bind it in `main`, don't discard it with `_`.
/// Returns `None` when `SENTRY_DSN` is unset, which is the normal local case:
/// logging still works, nothing is shipped anywhere.
pub fn init() -> Option<ClientInitGuard> {
    let guard = init_sentry();
    init_subscriber();

    match guard.as_ref() {
        Some(_) => tracing::info!(environment = %environment(), "Sentry error reporting enabled"),
        None => tracing::info!("SENTRY_DSN not set — error reporting disabled"),
    }

    guard
}

fn environment() -> String {
    std::env::var("SENTRY_ENVIRONMENT")
        .or_else(|_| std::env::var("POSTGRES_DATABASE"))
        .unwrap_or_else(|_| "local".to_string())
}

fn init_sentry() -> Option<ClientInitGuard> {
    let dsn = std::env::var("SENTRY_DSN").ok().filter(|d| !d.is_empty())?;

    Some(sentry::init((
        dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(environment().into()),
            // Traces are sampled; errors are not. A trace per request is far
            // more volume than the free tier tolerates, and the value here is
            // in the errors.
            traces_sample_rate: sample_rate("SENTRY_TRACES_SAMPLE_RATE", 0.0),
            attach_stacktrace: true,
            send_default_pii: false,
            ..Default::default()
        },
    )))
}

fn sample_rate(var: &str, default: f32) -> f32 {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| (0.0..=1.0).contains(v))
        .unwrap_or(default)
}

fn init_subscriber() {
    // `info` for us, `warn` for dependencies: sqlx logs every statement at
    // info, which drowns everything else in query text.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,hyper=warn,h2=warn,rustls=warn"));

    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(sentry::integrations::tracing::layer());

    // JSON in a container so a collector can parse it; human-readable when a
    // person is watching a terminal.
    if json_logs() {
        registry
            .with(fmt::layer().json().flatten_event(true))
            .init();
    } else {
        registry.with(fmt::layer().compact()).init();
    }
}

fn json_logs() -> bool {
    match std::env::var("LOG_FORMAT") {
        Ok(v) => v.eq_ignore_ascii_case("json"),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_rate_rejects_values_outside_the_unit_interval() {
        unsafe { std::env::set_var("TEST_RATE_HIGH", "1.5") };
        unsafe { std::env::set_var("TEST_RATE_NEG", "-0.2") };
        unsafe { std::env::set_var("TEST_RATE_JUNK", "yes") };
        assert_eq!(sample_rate("TEST_RATE_HIGH", 0.1), 0.1);
        assert_eq!(sample_rate("TEST_RATE_NEG", 0.1), 0.1);
        assert_eq!(sample_rate("TEST_RATE_JUNK", 0.1), 0.1);
    }

    #[test]
    fn sample_rate_accepts_the_boundaries() {
        unsafe { std::env::set_var("TEST_RATE_ZERO", "0.0") };
        unsafe { std::env::set_var("TEST_RATE_ONE", "1.0") };
        assert_eq!(sample_rate("TEST_RATE_ZERO", 0.5), 0.0);
        assert_eq!(sample_rate("TEST_RATE_ONE", 0.5), 1.0);
    }

    #[test]
    fn json_logs_is_opt_in_and_case_insensitive() {
        unsafe { std::env::remove_var("LOG_FORMAT") };
        assert!(!json_logs());
        unsafe { std::env::set_var("LOG_FORMAT", "JSON") };
        assert!(json_logs());
        unsafe { std::env::set_var("LOG_FORMAT", "pretty") };
        assert!(!json_logs());
    }
}
