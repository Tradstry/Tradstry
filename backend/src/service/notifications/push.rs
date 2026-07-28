use anyhow::{Context, Result};
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

use super::deliveries::DueDelivery;

#[derive(Debug, Clone, PartialEq)]
pub enum PushOutcome {
    Sent,
    /// The endpoint is permanently dead (404/410). Delete the subscription.
    Gone,
    Retry(String),
}

pub trait PushSender: Send + Sync {
    fn send<'a>(
        &'a self,
        target: &'a DueDelivery,
    ) -> Pin<Box<dyn Future<Output = PushOutcome> + Send + 'a>>;
}

/// Real sender. `skip_all` because `DueDelivery` carries the subscription's
/// encryption keys, which must never reach a log line.
pub struct WebPushSender {
    client: reqwest::Client,
    private_key: String,
    subject: String,
}

impl WebPushSender {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            private_key: std::env::var("VAPID_PRIVATE_KEY").context("VAPID_PRIVATE_KEY not set")?,
            subject: std::env::var("VAPID_SUBJECT").context("VAPID_SUBJECT not set")?,
        })
    }
}

impl PushSender for WebPushSender {
    fn send<'a>(
        &'a self,
        target: &'a DueDelivery,
    ) -> Pin<Box<dyn Future<Output = PushOutcome> + Send + 'a>> {
        Box::pin(async move {
            match self.send_inner(target).await {
                Ok(outcome) => outcome,
                Err(e) => PushOutcome::Retry(e.to_string()),
            }
        })
    }
}

impl WebPushSender {
    #[tracing::instrument(skip_all, fields(notification = %target.notification_id))]
    async fn send_inner(&self, target: &DueDelivery) -> Result<PushOutcome> {
        use web_push::{
            ContentEncoding, SubscriptionInfo, VapidSignatureBuilder, WebPushMessageBuilder,
        };

        let info = SubscriptionInfo::new(
            target.endpoint.clone(),
            target.p256dh.clone(),
            target.auth.clone(),
        );

        let payload = serde_json::json!({
            "title": target.title,
            "body": target.body,
            "deep_link": target.deep_link,
            "notification_id": target.notification_id,
        })
        .to_string();

        let mut sig_builder = VapidSignatureBuilder::from_base64(&self.private_key, &info)?;
        sig_builder.add_claim("sub", self.subject.as_str());
        let signature = sig_builder.build()?;

        let mut builder = WebPushMessageBuilder::new(&info);
        builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_bytes());
        builder.set_vapid_signature(signature);

        let message = builder.build()?;

        let mut request = self
            .client
            .post(message.endpoint.to_string())
            .header("TTL", message.ttl.to_string());

        if let Some(payload) = message.payload {
            request = request
                .header("Content-Encoding", payload.content_encoding.to_str())
                .header("Content-Type", "application/octet-stream");
            for (name, value) in payload.crypto_headers {
                request = request.header(name, value);
            }
            request = request.body(payload.content);
        }

        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => return Ok(PushOutcome::Retry(e.to_string())),
        };

        let status = response.status();
        if status.is_success() {
            Ok(PushOutcome::Sent)
        } else if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::GONE {
            Ok(PushOutcome::Gone)
        } else {
            Ok(PushOutcome::Retry(status.to_string()))
        }
    }
}

/// Scripted sender for tests. Outcomes are returned in order; once exhausted it
/// keeps returning the last one, so a test only scripts what it cares about.
pub struct FakePushSender {
    outcomes: Mutex<std::collections::VecDeque<PushOutcome>>,
    last: Mutex<PushOutcome>,
}

impl FakePushSender {
    pub fn new(outcomes: Vec<PushOutcome>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes.into()),
            last: Mutex::new(PushOutcome::Sent),
        }
    }
}

impl PushSender for FakePushSender {
    fn send<'a>(
        &'a self,
        _target: &'a DueDelivery,
    ) -> Pin<Box<dyn Future<Output = PushOutcome> + Send + 'a>> {
        let next = {
            let mut queue = self.outcomes.lock().unwrap();
            match queue.pop_front() {
                Some(o) => {
                    *self.last.lock().unwrap() = o.clone();
                    o
                }
                None => self.last.lock().unwrap().clone(),
            }
        };
        Box::pin(async move { next })
    }
}
