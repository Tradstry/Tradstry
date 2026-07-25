//! Sends one error through the real telemetry stack and exits.
//!
//! Verifies the whole chain end to end — subscriber, `log` bridge, Sentry
//! layer, DSN, network — without booting the server or needing a database.
//!
//!     SENTRY_DSN=... SENTRY_ENVIRONMENT=smoke cargo run --example sentry_smoke
//!
//! Set `SENTRY_ENVIRONMENT` to something other than `prod` so a test event
//! doesn't land in the same bucket as real production errors.

use tradstry_backend::service::telemetry;

#[tracing::instrument(fields(account = "acct-demo"))]
fn simulate_sync_failure(st_account: &str) {
    // Emitted via `log`, not `tracing`, to prove the compatibility bridge
    // carries the ~160 existing call sites into the new subscriber.
    log::warn!("legacy log:: call site — should appear with the span's fields attached");

    tracing::error!(
        error = "simulated failure from sentry_smoke",
        "brokerage sync failed"
    );
}

fn main() {
    let guard = telemetry::init();

    if guard.is_none() {
        eprintln!("\nSENTRY_DSN is unset — logging was exercised, nothing was sent.");
        eprintln!("Re-run with SENTRY_DSN=... to test delivery.\n");
    }

    simulate_sync_failure("st-acct-demo");

    // Dropping the guard flushes queued events. Without this the process can
    // exit before the request leaves, and the event is silently lost.
    drop(guard);
    println!("\nDone. If a DSN was set, check Sentry → Issues for 'brokerage sync failed'.");
}
