//! `POST /webhooks/paddle` — verify, persist, acknowledge.
//!
//! Paddle expects a 2xx within 5 seconds and retries up to 60 times otherwise,
//! so this handler does no plan work at all: it writes the delivery to
//! `paddle_webhook_events` and returns. `billing::worker` applies it.
//!
//! The route sits outside the Clerk middleware (it is absent from the protected
//! list in `main.rs`) — Paddle has no session. The signature is the auth.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use chrono::Utc;

use crate::service::billing::paddle;
use crate::service::db::Db;
use crate::service::db::schema::tables::billing_table;

const SIGNATURE_HEADER: &str = "Paddle-Signature";

pub async fn paddle_webhook_handler(
    req: HttpRequest,
    body: web::Bytes,
    db: web::Data<Arc<Db>>,
) -> HttpResponse {
    let secret = match std::env::var("PADDLE_WEBHOOK_SECRET") {
        Ok(secret) if !secret.is_empty() => secret,
        _ => {
            log::error!("[paddle] PADDLE_WEBHOOK_SECRET is not set; refusing webhook");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let header = req
        .headers()
        .get(SIGNATURE_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();

    // `body` is the untouched request bytes — re-serialising JSON here would
    // change the digest and reject every legitimate delivery.
    if let Err(error) = paddle::verify_signature(&secret, header, &body, Utc::now()) {
        log::warn!("[paddle] rejected webhook: {error}");
        return HttpResponse::Unauthorized().finish();
    }

    // Metadata only — never the typed subscription shape. This route must be
    // able to store any signed event, whatever its `data` looks like.
    let envelope = match paddle::parse_meta(&body) {
        Ok(envelope) => envelope,
        Err(error) => {
            // No event_id means nothing to store or dedupe on. Acknowledge
            // rather than make Paddle retry an unparseable body 60 times.
            log::warn!("[paddle] signed payload had no usable envelope: {error:#}");
            return HttpResponse::Ok().finish();
        }
    };

    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(error) => {
            log::error!("[paddle] payload is not JSON after parsing succeeded: {error}");
            return HttpResponse::Ok().finish();
        }
    };

    match billing_table::record_webhook_event(
        db.pool(),
        &envelope.event_id,
        &envelope.event_type,
        envelope.occurred_at,
        &payload,
    )
    .await
    {
        Ok(true) => log::info!(
            "[paddle] queued {} ({})",
            envelope.event_type,
            envelope.event_id
        ),
        Ok(false) => log::info!("[paddle] redelivery ignored ({})", envelope.event_id),
        Err(error) => {
            // Returning 500 makes Paddle retry, which is what we want — the
            // event is not yet durable.
            log::error!(
                "[paddle] failed to persist {}: {error:#}",
                envelope.event_id
            );
            return HttpResponse::InternalServerError().finish();
        }
    }

    HttpResponse::Ok().finish()
}
