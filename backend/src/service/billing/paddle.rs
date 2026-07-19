//! Paddle webhook verification and event → plan-change mapping.
//!
//! Pure functions only: nothing here touches the database or the network, so the
//! mapping rules are testable against recorded payload shapes. The route
//! persists, the worker applies.
//!
//! Two rules worth stating outright:
//!
//! 1. **An unrecognised `price_id` is a hard error.** Silently defaulting to a
//!    tier would either give away a paid plan or strip one from a paying user.
//! 2. **`users.plan` holds the *subscribed* tier, not the tier in force.** A
//!    canceled subscription keeps `plan = "pro"` until its period ends;
//!    `entitlements::effective_plan` decides what that currently grants.

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

use super::entitlements::{PRO, PRO_PLUS};
use crate::service::db::schema::tables::billing_table::SubscriptionUpdate;

/// Days a past_due subscription keeps its plan while payment is retried.
pub const GRACE_PERIOD_DAYS: i64 = 2;

/// Replay window for the signature timestamp.
///
/// Paddle's own SDKs default to 5 seconds. That is tight enough that ordinary
/// clock drift on a self-hosted box rejects live events, and replay is already
/// closed off by `paddle_webhook_events.event_id` being the primary key — a
/// replayed body cannot be processed twice regardless of its age.
const TIMESTAMP_TOLERANCE_SECS: i64 = 60;

type HmacSha256 = Hmac<Sha256>;

/// Signature rejection is deliberately a distinct type: the route answers it
/// with 401 and persists nothing, whereas every other failure is a 500.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureError {
    MalformedHeader,
    StaleTimestamp,
    Mismatch,
}

impl std::fmt::Display for SignatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignatureError::MalformedHeader => write!(f, "malformed Paddle-Signature header"),
            SignatureError::StaleTimestamp => write!(f, "signature timestamp outside tolerance"),
            SignatureError::Mismatch => write!(f, "signature mismatch"),
        }
    }
}

impl std::error::Error for SignatureError {}

/// Verify a `Paddle-Signature` header of the form `ts=<unix>;h1=<hex>`.
///
/// The signed payload is `<ts>:<raw body>` — the body must be the exact bytes
/// received, since any re-serialisation changes the digest.
pub fn verify_signature(
    secret: &str,
    header: &str,
    raw_body: &[u8],
    now: DateTime<Utc>,
) -> std::result::Result<(), SignatureError> {
    let mut ts: Option<&str> = None;
    let mut h1: Option<&str> = None;

    for part in header.split(';') {
        match part.trim().split_once('=') {
            Some(("ts", v)) => ts = Some(v),
            Some(("h1", v)) => h1 = Some(v),
            _ => {}
        }
    }

    let (ts, h1) = match (ts, h1) {
        (Some(ts), Some(h1)) if !ts.is_empty() && !h1.is_empty() => (ts, h1),
        _ => return Err(SignatureError::MalformedHeader),
    };

    let ts_secs: i64 = ts.parse().map_err(|_| SignatureError::MalformedHeader)?;
    if (now.timestamp() - ts_secs).abs() > TIMESTAMP_TOLERANCE_SECS {
        return Err(SignatureError::StaleTimestamp);
    }

    let expected = hex::decode(h1).map_err(|_| SignatureError::MalformedHeader)?;

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| SignatureError::Mismatch)?;
    mac.update(ts.as_bytes());
    mac.update(b":");
    mac.update(raw_body);

    // `verify_slice` is constant-time.
    mac.verify_slice(&expected)
        .map_err(|_| SignatureError::Mismatch)
}

// ── payload shapes ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Envelope {
    pub event_id: String,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
    pub data: SubscriptionData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionData {
    pub id: String,
    pub customer_id: String,
    pub status: String,
    #[serde(default)]
    pub items: Vec<SubscriptionItem>,
    #[serde(default)]
    pub current_billing_period: Option<BillingPeriod>,
    #[serde(default)]
    pub scheduled_change: Option<ScheduledChange>,
    #[serde(default)]
    pub custom_data: Option<CustomData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionItem {
    pub price: Price,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Price {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BillingPeriod {
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScheduledChange {
    pub action: String,
    pub effective_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CustomData {
    #[serde(default)]
    pub user_id: Option<String>,
}

/// Just the delivery metadata, with `data` left untouched.
///
/// The route uses only this. A destination is subscribed to dozens of event
/// types whose `data` shapes differ wildly, and parsing them all against
/// `SubscriptionData` would drop anything that didn't fit — including, one day,
/// a subscription event with an unexpected null. Store first, interpret later.
#[derive(Debug, Clone, Deserialize)]
pub struct EventMeta {
    pub event_id: String,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
}

pub fn parse_meta(raw_body: &[u8]) -> Result<EventMeta> {
    serde_json::from_slice(raw_body).context("Failed to parse Paddle webhook envelope")
}

/// The full typed parse, used by the worker once it knows the event is a
/// subscription event. A failure here is retried, not silently dropped.
pub fn parse_envelope(raw_body: &[u8]) -> Result<Envelope> {
    serde_json::from_slice(raw_body).context("Failed to parse Paddle webhook payload")
}

/// True for the events the worker knows how to apply. Anything else is stored
/// and marked processed without effect, so an accidental subscription in the
/// Paddle dashboard cannot wedge the queue.
pub fn is_subscription_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "subscription.created"
            | "subscription.activated"
            | "subscription.updated"
            | "subscription.canceled"
            | "subscription.past_due"
            | "subscription.paused"
            | "subscription.resumed"
            // Subscribed on the Paddle side too. `trialing` carries a real
            // status we honour, and `imported` is how a migrated subscription
            // first appears — skipping either would leave the plan unset.
            | "subscription.trialing"
            | "subscription.imported"
    )
}

// ── event → row change ──────────────────────────────────────────────────────

/// The row change an event implies, plus the two ways of identifying its owner.
#[derive(Debug, Clone)]
pub struct PlanMutation {
    /// From `custom_data.user_id`, set at checkout. Authoritative when present.
    pub user_id: Option<String>,
    /// Fallback lookup key when the checkout carried no `custom_data`.
    pub paddle_customer_id: String,
    pub update: SubscriptionUpdate,
}

/// Map a `price_id` to a tier using the configured catalog.
///
/// Env rather than a constant because the ids differ between the sandbox and
/// live Paddle accounts, which are entirely separate catalogs.
pub fn price_to_plan(price_id: &str) -> Result<&'static str> {
    let pro = std::env::var("PADDLE_PRICE_PRO").unwrap_or_default();
    let pro_plus = std::env::var("PADDLE_PRICE_PRO_PLUS").unwrap_or_default();

    if !pro.is_empty() && price_id == pro {
        return Ok(PRO);
    }
    if !pro_plus.is_empty() && price_id == pro_plus {
        return Ok(PRO_PLUS);
    }

    Err(anyhow!(
        "Unrecognised Paddle price_id {price_id}; refusing to guess a tier \
         (check PADDLE_PRICE_PRO / PADDLE_PRICE_PRO_PLUS)"
    ))
}

/// Translate a subscription event into the row change it implies.
///
/// The plan always comes from the priced item, for every status. Downgrades are
/// a read-time decision in `entitlements::effective_plan` — writing `free` here
/// on cancellation would cut off a user who has already paid for the rest of
/// the period.
pub fn apply_event(envelope: &Envelope) -> Result<PlanMutation> {
    if !is_subscription_event(&envelope.event_type) {
        bail!("{} is not a subscription event", envelope.event_type);
    }

    let data = &envelope.data;

    let plan = data
        .items
        .iter()
        .find_map(|item| price_to_plan(&item.price.id).ok())
        .ok_or_else(|| match data.items.first() {
            Some(item) => price_to_plan(&item.price.id).unwrap_err(),
            None => anyhow!("subscription {} carries no items", data.id),
        })?;

    let grace_until = (data.status == "past_due")
        .then(|| envelope.occurred_at + Duration::days(GRACE_PERIOD_DAYS));

    let (period_start, mut period_end) = match &data.current_billing_period {
        Some(period) => (Some(period.starts_at), Some(period.ends_at)),
        None => (None, None),
    };

    // A cancellation is scheduled while the subscription is still `active`; the
    // effective date, not the billing period, is when access actually ends.
    if let Some(change) = &data.scheduled_change
        && change.action == "cancel"
    {
        period_end = Some(change.effective_at);
    }

    Ok(PlanMutation {
        user_id: data
            .custom_data
            .as_ref()
            .and_then(|c| c.user_id.clone())
            .filter(|id| !id.is_empty()),
        paddle_customer_id: data.customer_id.clone(),
        update: SubscriptionUpdate {
            plan: plan.to_string(),
            paddle_customer_id: Some(data.customer_id.clone()),
            paddle_subscription_id: Some(data.id.clone()),
            subscription_status: data.status.clone(),
            subscription_updated_at: envelope.occurred_at,
            current_period_start: period_start,
            current_period_end: period_end,
            grace_until,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    const SECRET: &str = "pdl_ntfset_test_secret";
    const PRICE_PRO: &str = "pri_pro_monthly";
    const PRICE_PRO_PLUS: &str = "pri_pro_plus_monthly";

    /// Env is process-global and Rust runs tests in threads, so every test that
    /// depends on the price catalog goes through this one guarded setup.
    fn with_prices<T>(f: impl FnOnce() -> T) -> T {
        use std::sync::Mutex;
        static LOCK: Mutex<()> = Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var("PADDLE_PRICE_PRO", PRICE_PRO);
            std::env::set_var("PADDLE_PRICE_PRO_PLUS", PRICE_PRO_PLUS);
        }
        f()
    }

    fn sign(body: &[u8], ts: i64) -> String {
        let mut mac = HmacSha256::new_from_slice(SECRET.as_bytes()).unwrap();
        mac.update(ts.to_string().as_bytes());
        mac.update(b":");
        mac.update(body);
        format!("ts={ts};h1={}", hex::encode(mac.finalize().into_bytes()))
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap()
    }

    fn payload(event_type: &str, status: &str, extra: &str) -> String {
        format!(
            r#"{{
              "event_id": "evt_01",
              "event_type": "{event_type}",
              "occurred_at": "2026-07-18T12:00:00.000000Z",
              "data": {{
                "id": "sub_01",
                "customer_id": "ctm_01",
                "status": "{status}",
                "items": [{{ "price": {{ "id": "{PRICE_PRO}" }} }}],
                "current_billing_period": {{
                  "starts_at": "2026-07-01T00:00:00.000000Z",
                  "ends_at": "2026-08-01T00:00:00.000000Z"
                }},
                "custom_data": {{ "user_id": "user_abc" }}
                {extra}
              }}
            }}"#
        )
    }

    // ── signature ───────────────────────────────────────────────────────────

    #[test]
    fn a_correct_signature_verifies() {
        let body = br#"{"event_id":"evt_01"}"#;
        let header = sign(body, now().timestamp());
        assert!(verify_signature(SECRET, &header, body, now()).is_ok());
    }

    #[test]
    fn a_tampered_body_fails() {
        let body = br#"{"event_id":"evt_01"}"#;
        let header = sign(body, now().timestamp());
        let tampered = br#"{"event_id":"evt_02"}"#;
        assert_eq!(
            verify_signature(SECRET, &header, tampered, now()),
            Err(SignatureError::Mismatch)
        );
    }

    #[test]
    fn the_wrong_secret_fails() {
        let body = br#"{"event_id":"evt_01"}"#;
        let header = sign(body, now().timestamp());
        assert_eq!(
            verify_signature("other_secret", &header, body, now()),
            Err(SignatureError::Mismatch)
        );
    }

    #[test]
    fn a_stale_timestamp_fails_even_with_a_valid_digest() {
        let body = br#"{"event_id":"evt_01"}"#;
        let old = now().timestamp() - TIMESTAMP_TOLERANCE_SECS - 1;
        let header = sign(body, old);
        assert_eq!(
            verify_signature(SECRET, &header, body, now()),
            Err(SignatureError::StaleTimestamp)
        );
    }

    #[test]
    fn a_timestamp_inside_the_window_passes() {
        let body = br#"{"event_id":"evt_01"}"#;
        let recent = now().timestamp() - (TIMESTAMP_TOLERANCE_SECS - 1);
        let header = sign(body, recent);
        assert!(verify_signature(SECRET, &header, body, now()).is_ok());
    }

    #[test]
    fn malformed_headers_are_rejected() {
        let body = br#"{}"#;
        for header in ["", "ts=123", "h1=abcd", "garbage", "ts=abc;h1=dead"] {
            assert!(
                matches!(
                    verify_signature(SECRET, header, body, now()),
                    Err(SignatureError::MalformedHeader)
                ),
                "expected malformed header for {header:?}"
            );
        }
    }

    // ── event mapping ───────────────────────────────────────────────────────

    #[test]
    fn an_active_subscription_grants_its_tier() {
        with_prices(|| {
            let env = parse_envelope(payload("subscription.activated", "active", "").as_bytes())
                .expect("parse");
            let mutation = apply_event(&env).expect("apply");

            assert_eq!(mutation.update.plan, PRO);
            assert_eq!(mutation.update.subscription_status, "active");
            assert_eq!(mutation.user_id.as_deref(), Some("user_abc"));
            assert_eq!(mutation.paddle_customer_id, "ctm_01");
            assert_eq!(
                mutation.update.current_period_end,
                Some(Utc.with_ymd_and_hms(2026, 8, 1, 0, 0, 0).unwrap())
            );
            assert!(mutation.update.grace_until.is_none());
        });
    }

    #[test]
    fn past_due_keeps_the_tier_and_opens_a_grace_window() {
        with_prices(|| {
            let env = parse_envelope(payload("subscription.past_due", "past_due", "").as_bytes())
                .expect("parse");
            let mutation = apply_event(&env).expect("apply");

            assert_eq!(mutation.update.plan, PRO, "plan is kept while retrying");
            assert_eq!(
                mutation.update.grace_until,
                Some(now() + Duration::days(GRACE_PERIOD_DAYS))
            );
        });
    }

    #[test]
    fn recovering_from_past_due_clears_the_grace_window() {
        with_prices(|| {
            let env = parse_envelope(payload("subscription.updated", "active", "").as_bytes())
                .expect("parse");
            let mutation = apply_event(&env).expect("apply");
            assert!(mutation.update.grace_until.is_none());
        });
    }

    #[test]
    fn a_scheduled_cancel_keeps_the_tier_and_ends_at_the_effective_date() {
        with_prices(|| {
            let extra = r#", "scheduled_change": {
                "action": "cancel",
                "effective_at": "2026-09-15T00:00:00.000000Z"
            }"#;
            let env = parse_envelope(payload("subscription.updated", "active", extra).as_bytes())
                .expect("parse");
            let mutation = apply_event(&env).expect("apply");

            assert_eq!(mutation.update.plan, PRO);
            assert_eq!(
                mutation.update.current_period_end,
                Some(Utc.with_ymd_and_hms(2026, 9, 15, 0, 0, 0).unwrap()),
                "the scheduled effective date wins over the billing period"
            );
        });
    }

    #[test]
    fn a_canceled_subscription_still_records_its_tier() {
        with_prices(|| {
            let env = parse_envelope(payload("subscription.canceled", "canceled", "").as_bytes())
                .expect("parse");
            let mutation = apply_event(&env).expect("apply");

            assert_eq!(
                mutation.update.plan, PRO,
                "effective_plan decides access from the period end, so the tier must survive"
            );
            assert_eq!(mutation.update.subscription_status, "canceled");
        });
    }

    #[test]
    fn an_unknown_price_is_an_error_not_a_guess() {
        with_prices(|| {
            let body = payload("subscription.updated", "active", "")
                .replace(PRICE_PRO, "pri_something_unrecognised");
            let env = parse_envelope(body.as_bytes()).expect("parse");

            let err = apply_event(&env).expect_err("unknown price must fail");
            assert!(
                err.to_string().contains("Unrecognised Paddle price_id"),
                "got: {err}"
            );
        });
    }

    #[test]
    fn the_pro_plus_price_maps_to_its_own_tier() {
        with_prices(|| {
            let body =
                payload("subscription.updated", "active", "").replace(PRICE_PRO, PRICE_PRO_PLUS);
            let env = parse_envelope(body.as_bytes()).expect("parse");
            assert_eq!(apply_event(&env).expect("apply").update.plan, PRO_PLUS);
        });
    }

    #[test]
    fn a_payload_without_custom_data_falls_back_to_the_customer_id() {
        with_prices(|| {
            let body = payload("subscription.updated", "active", "").replace(
                r#""custom_data": { "user_id": "user_abc" }"#,
                r#""custom_data": null"#,
            );
            let env = parse_envelope(body.as_bytes()).expect("parse");
            let mutation = apply_event(&env).expect("apply");

            assert!(mutation.user_id.is_none());
            assert_eq!(mutation.paddle_customer_id, "ctm_01");
        });
    }

    #[test]
    fn non_subscription_events_are_recognised_as_out_of_scope() {
        assert!(is_subscription_event("subscription.past_due"));
        assert!(!is_subscription_event("transaction.completed"));
    }

    /// Regression: the route used to run the full typed parse, so a real
    /// `transaction.*` delivery (no `customer_id`, nulls in `data`) was
    /// acknowledged and dropped instead of stored.
    #[test]
    fn the_envelope_parses_for_event_shapes_we_do_not_model() {
        let body = br#"{
          "event_id": "evt_txn_01",
          "event_type": "transaction.completed",
          "occurred_at": "2026-07-19T20:24:13.000000Z",
          "data": {
            "id": "txn_01",
            "status": "completed",
            "customer_id": null,
            "items": [{ "price": { "id": "pri_x" }, "proration": null }]
          }
        }"#;

        let meta = parse_meta(body).expect("metadata must parse regardless of data shape");
        assert_eq!(meta.event_id, "evt_txn_01");
        assert_eq!(meta.event_type, "transaction.completed");

        // The typed parse still refuses it — which is correct, and why only the
        // worker (for subscription events) is allowed to run it.
        assert!(parse_envelope(body).is_err());
    }

    #[test]
    fn a_body_without_an_event_id_is_rejected() {
        assert!(parse_meta(br#"{"event_type":"subscription.updated"}"#).is_err());
    }
}
