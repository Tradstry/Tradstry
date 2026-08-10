use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use anyhow::{Context, Result};
use chrono::Utc;

use crate::service::brokerage::webhook;
use crate::service::db::Db;

#[derive(Clone)]
pub struct SnapTradeWebhookConfig {
    consumer_key: Arc<str>,
}

impl SnapTradeWebhookConfig {
    pub fn from_env() -> Result<Self> {
        let consumer_key = std::env::var("SNAPTRADE_CONSUMER_KEY")
            .context("SNAPTRADE_CONSUMER_KEY not set for webhook verification")?;
        anyhow::ensure!(!consumer_key.is_empty(), "SNAPTRADE_CONSUMER_KEY is empty");
        Ok(Self {
            consumer_key: Arc::from(consumer_key),
        })
    }
}

pub async fn ingest(
    request: HttpRequest,
    body: web::Bytes,
    db: web::Data<Arc<Db>>,
    config: web::Data<SnapTradeWebhookConfig>,
) -> HttpResponse {
    let signature = request
        .headers()
        .get("Signature")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let event =
        match webhook::verify_and_normalize(&body, signature, &config.consumer_key, Utc::now()) {
            Ok(event) => event,
            Err(error) => {
                log::warn!("rejected SnapTrade webhook: {error}");
                return HttpResponse::Unauthorized().finish();
            }
        };
    match webhook::ingest(db.pool(), &event).await {
        Ok(_) => HttpResponse::Accepted().finish(),
        Err(error) => {
            log::error!("failed to persist SnapTrade webhook: {error}");
            HttpResponse::ServiceUnavailable().finish()
        }
    }
}
