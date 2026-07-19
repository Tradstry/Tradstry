//! The one place we call the Paddle API outbound.
//!
//! Everything else in billing is driven by webhooks. This mints a hosted portal
//! session so the user can change their card, read invoices or cancel — screens
//! we deliberately do not rebuild, because doing so would put us in the path of
//! card data and tax rules that Paddle owns as merchant of record.

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const SANDBOX_BASE: &str = "https://sandbox-api.paddle.com";
const LIVE_BASE: &str = "https://api.paddle.com";

/// `PADDLE_ENV=sandbox` selects the sandbox host. Sandbox and live are separate
/// accounts with separate keys and catalogs — crossing them fails confusingly,
/// so the default is sandbox rather than live.
fn api_base() -> &'static str {
    match std::env::var("PADDLE_ENV").unwrap_or_default().as_str() {
        "live" | "production" => LIVE_BASE,
        _ => SANDBOX_BASE,
    }
}

#[derive(Deserialize)]
struct PortalResponse {
    data: PortalData,
}

#[derive(Deserialize)]
struct PortalData {
    urls: PortalUrls,
}

#[derive(Deserialize)]
struct PortalUrls {
    general: PortalGeneral,
}

#[derive(Deserialize)]
struct PortalGeneral {
    overview: String,
}

/// Create a customer portal session and return its overview URL.
///
/// Passing the subscription id makes the deep links inside the portal (update
/// payment method, cancel) resolve to that subscription.
pub async fn create_portal_session(
    paddle_customer_id: &str,
    paddle_subscription_id: Option<&str>,
) -> Result<String> {
    let api_key = std::env::var("PADDLE_API_KEY")
        .ok()
        .filter(|k| !k.is_empty())
        .context("PADDLE_API_KEY is not set")?;

    let body = serde_json::json!({
        "subscription_ids": paddle_subscription_id.into_iter().collect::<Vec<_>>(),
    });

    let response = reqwest::Client::new()
        .post(format!(
            "{}/customers/{paddle_customer_id}/portal-sessions",
            api_base()
        ))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .context("Failed to reach Paddle")?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("Paddle portal-session request failed ({status}): {text}");
    }

    let parsed: PortalResponse =
        serde_json::from_str(&text).context("Unexpected Paddle portal-session response")?;

    Ok(parsed.data.urls.general.overview)
}
