use anyhow::{Context, Result, anyhow};

/// Typed errors surfaced from the SnapTrade microservice. Returned wrapped in
/// `anyhow::Error` — callers downcast via `e.downcast_ref::<SnapTradeError>()`
/// to detect specific recovery cases.
#[derive(Debug)]
pub enum SnapTradeError {
    /// SnapTrade rejected the stored userID/userSecret with code 1083. Caller
    /// should clear the stored credentials and re-register before retrying.
    StaleCredentials,
}

impl std::fmt::Display for SnapTradeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleCredentials => write!(
                f,
                "SnapTrade credentials are stale (code 1083) — re-registration required"
            ),
        }
    }
}

impl std::error::Error for SnapTradeError {}

/// Envelope returned by the Go microservice when it forwards a SnapTrade error.
/// All fields optional so this also matches the legacy `{ "error": "..." }`
/// shape — we only act on `snaptrade_code` when it's present.
#[derive(Debug, Deserialize)]
struct GoErrorEnvelope {
    #[allow(dead_code)]
    error: Option<String>,
    #[allow(dead_code)]
    snaptrade_status: Option<u16>,
    snaptrade_code: Option<String>,
    #[allow(dead_code)]
    snaptrade_detail: Option<String>,
}
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct BrokerageClient {
    http: reqwest::Client,
    base_url: String,
}

// ── SnapTrade response types

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradePagination {
    pub offset: Option<i32>,
    pub limit: Option<i32>,
    pub total: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeTransactionsResponse {
    pub data: Vec<SnapTradeActivity>,
    pub pagination: Option<SnapTradePagination>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeActivity {
    pub id: Option<String>,
    pub symbol: Option<SnapTradeSymbol>,
    pub option_symbol: Option<serde_json::Value>,
    pub price: Option<f64>,
    pub units: Option<f64>,
    pub amount: Option<f64>,
    pub currency: Option<SnapTradeCurrency>,
    #[serde(rename = "type")]
    pub activity_type: Option<String>,
    pub option_type: Option<String>,
    pub description: Option<String>,
    pub trade_date: Option<String>,
    pub settlement_date: Option<String>,
    pub fee: Option<f64>,
    pub fx_rate: Option<f64>,
    pub institution: Option<String>,
    pub external_reference_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeSymbol {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub raw_symbol: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub currency: Option<SnapTradeCurrency>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeCurrency {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// Holdings response types

// All holdings structs use #[serde(default)] and a catch-all #[serde(flatten)]
// to handle unexpected fields/types from SnapTrade's evolving API.

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeHoldingsResponse {
    #[serde(default)]
    pub account: Option<serde_json::Value>,
    #[serde(default)]
    pub balances: Option<Vec<SnapTradeBalance>>,
    #[serde(default)]
    pub positions: Option<Vec<SnapTradePosition>>,
    #[serde(default)]
    pub option_positions: Option<Vec<SnapTradeOptionPosition>>,
    #[serde(default)]
    pub orders: Option<Vec<serde_json::Value>>,
    /// SnapTrade's authoritative total market value (`account.balance.total`),
    /// forwarded by the Go service as `total_value`. `amount` may be null.
    #[serde(default)]
    pub total_value: Option<SnapTradeTotalValue>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeTotalValue {
    #[serde(default)]
    pub amount: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeBalance {
    #[serde(default)]
    pub currency: Option<SnapTradeCurrency>,
    #[serde(default)]
    pub cash: Option<f64>,
    #[serde(default)]
    pub buying_power: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradePosition {
    #[serde(default)]
    pub symbol: Option<SnapTradeSymbol>,
    #[serde(default)]
    pub units: Option<f64>,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub open_pnl: Option<f64>,
    #[serde(default)]
    pub average_purchase_price: Option<f64>,
    #[serde(default)]
    pub currency: Option<SnapTradeCurrency>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeOptionPosition {
    #[serde(default)]
    pub option_symbol: Option<SnapTradeOptionSymbol>,
    #[serde(default)]
    pub units: Option<f64>,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub average_purchase_price: Option<f64>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeOptionSymbol {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub ticker: Option<String>,
    #[serde(default)]
    pub option_type: Option<String>,
    #[serde(default)]
    pub strike_price: Option<f64>,
    #[serde(default)]
    pub expiration_date: Option<String>,
    #[serde(default)]
    pub underlying_symbol: Option<SnapTradeSymbol>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ── SnapTrade account types ────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeAccount {
    pub id: Option<String>,
    pub name: Option<String>,
    pub number: Option<String>,
    #[serde(rename = "institutionName")]
    pub institution_name: Option<String>,
    // Catch any other fields
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

// ── Connection flow types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateUserResponse {
    pub user_id: String,
    pub user_secret: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InitiateConnectionResponse {
    pub redirect_url: String,
    pub connection_id: String,
}

// ── Client implementation ───────────────────────────────────────────────────

impl BrokerageClient {
    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var("SNAPTRADE_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:9056".to_string());

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
        })
    }

    pub async fn register_user(&self, user_id: &str) -> Result<CreateUserResponse> {
        let result = self.try_register_user(user_id).await;

        match result {
            Ok(resp) => Ok(resp),
            Err(e) if e.to_string().contains("400") || e.to_string().contains("already exist") => {
                // User exists but we lost the secret — delete and re-register
                log::info!(
                    "SnapTrade user {} already exists, deleting and re-registering",
                    user_id
                );
                let _ = self.delete_user(user_id).await;
                self.try_register_user(user_id).await
            }
            Err(e) => Err(e),
        }
    }

    async fn try_register_user(&self, user_id: &str) -> Result<CreateUserResponse> {
        let url = format!("{}/api/v1/users", self.base_url);

        let response = self
            .http
            .post(&url)
            .header("X-User-Id", user_id)
            .json(&serde_json::json!({ "user_id": user_id }))
            .send()
            .await
            .context("Failed to call snaptrade service to register user")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "SnapTrade register user error {}: {}",
                status,
                body
            ));
        }

        response
            .json::<CreateUserResponse>()
            .await
            .context("Failed to parse register user response")
    }

    async fn delete_user(&self, user_id: &str) -> Result<()> {
        let url = format!("{}/api/v1/users/{}", self.base_url, user_id);

        let response = self
            .http
            .delete(&url)
            .header("X-User-Id", user_id)
            .send()
            .await
            .context("Failed to call snaptrade service to delete user")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("SnapTrade delete user error {}: {}", status, body));
        }

        Ok(())
    }

    pub async fn initiate_connection(
        &self,
        user_id: &str,
        user_secret: &str,
        brokerage_id: &str,
        connection_type: Option<&str>,
        custom_redirect: Option<&str>,
    ) -> Result<InitiateConnectionResponse> {
        let url = format!("{}/api/v1/connections/initiate", self.base_url);

        let mut body = serde_json::json!({
            "brokerage_id": brokerage_id,
            "user_secret": user_secret,
            "connection_type": connection_type.unwrap_or("read"),
        });
        if let Some(redirect) = custom_redirect {
            body["custom_redirect"] = serde_json::Value::String(redirect.to_string());
        }

        let response = self
            .http
            .post(&url)
            .header("X-User-Id", user_id)
            .json(&body)
            .send()
            .await
            .context("Failed to call snaptrade service to initiate connection")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            // If the Go service forwarded a SnapTrade error envelope, pull the
            // structured fields out and surface known recovery cases as typed
            // errors so callers can react without string-matching.
            if status.as_u16() == 401
                && let Ok(env) = serde_json::from_str::<GoErrorEnvelope>(&body)
                && env.snaptrade_code.as_deref() == Some("1083")
            {
                return Err(anyhow!(SnapTradeError::StaleCredentials));
            }

            return Err(anyhow!(
                "SnapTrade initiate connection error {}: {}",
                status,
                body
            ));
        }

        response
            .json::<InitiateConnectionResponse>()
            .await
            .context("Failed to parse initiate connection response")
    }

    /// Lists all SnapTrade brokerage accounts for a user.
    /// Returns SnapTrade account IDs (not internal Tradstry account IDs).
    pub async fn list_snaptrade_accounts(
        &self,
        user_id: &str,
        user_secret: &str,
    ) -> Result<Vec<SnapTradeAccount>> {
        let url = format!("{}/api/v1/accounts", self.base_url);

        log::info!("Listing SnapTrade accounts for user_id={}", user_id);

        let response = self
            .http
            .get(&url)
            .header("X-User-Id", user_id)
            .header("X-User-Secret", user_secret)
            .send()
            .await
            .context("Failed to call snaptrade service to list accounts")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "SnapTrade list accounts error {}: {}",
                status,
                body
            ));
        }

        let accounts: Vec<SnapTradeAccount> = response
            .json()
            .await
            .context("Failed to parse list accounts response")?;

        log::info!(
            "Found {} SnapTrade accounts for user_id={}",
            accounts.len(),
            user_id
        );
        for acc in &accounts {
            log::info!(
                "  SnapTrade account: id={:?} name={:?} institution={:?}",
                acc.id,
                acc.name,
                acc.institution_name
            );
        }

        Ok(accounts)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn fetch_transactions(
        &self,
        user_id: &str,
        user_secret: &str,
        account_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
        tx_type: Option<&str>,
        offset: Option<i32>,
        limit: Option<i32>,
    ) -> Result<SnapTradeTransactionsResponse> {
        let url = format!(
            "{}/api/v1/accounts/{}/transactions",
            self.base_url, account_id
        );

        let mut req = self
            .http
            .get(&url)
            .header("X-User-Id", user_id)
            .header("X-User-Secret", user_secret);

        if let Some(sd) = start_date {
            req = req.query(&[("start_date", sd)]);
        }
        if let Some(ed) = end_date {
            req = req.query(&[("end_date", ed)]);
        }
        if let Some(t) = tx_type {
            req = req.query(&[("type", t)]);
        }
        if let Some(o) = offset {
            req = req.query(&[("offset", o.to_string())]);
        }
        if let Some(l) = limit {
            req = req.query(&[("limit", l.to_string())]);
        }

        log::info!(
            "Fetching transactions: account_id={} offset={:?} limit={:?}",
            account_id,
            offset,
            limit
        );

        let response = req
            .send()
            .await
            .context("Failed to call snaptrade service")?;

        let status = response.status();
        log::info!(
            "Transactions response status: {} for account_id={}",
            status,
            account_id
        );

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            log::error!(
                "Transactions fetch failed: status={} account_id={} body={}",
                status,
                account_id,
                body
            );
            return Err(anyhow!("SnapTrade service error {}: {}", status, body));
        }

        let body_text = response
            .text()
            .await
            .context("Failed to read transactions response body")?;
        log::debug!(
            "Transactions raw response for account_id={}: {}",
            account_id,
            &body_text[..body_text.len().min(500)]
        );

        serde_json::from_str::<SnapTradeTransactionsResponse>(&body_text).context(format!(
            "Failed to parse transactions response for account_id={}: {}",
            account_id,
            &body_text[..body_text.len().min(200)]
        ))
    }

    pub async fn fetch_holdings(
        &self,
        user_id: &str,
        user_secret: &str,
        account_id: &str,
    ) -> Result<SnapTradeHoldingsResponse> {
        let url = format!("{}/api/v1/accounts/{}/holdings", self.base_url, account_id);

        log::info!(
            "Fetching holdings: url={} user_id={} account_id={}",
            url,
            user_id,
            account_id
        );

        let response = self
            .http
            .get(&url)
            .header("X-User-Id", user_id)
            .header("X-User-Secret", user_secret)
            .send()
            .await
            .context("Failed to call snaptrade service for holdings")?;

        let status = response.status();
        log::info!(
            "Holdings response status: {} for account_id={}",
            status,
            account_id
        );

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            log::error!(
                "Holdings fetch failed: status={} account_id={} body={}",
                status,
                account_id,
                body
            );
            return Err(anyhow!(
                "SnapTrade service holdings error {}: {}",
                status,
                body
            ));
        }

        let body_text = response
            .text()
            .await
            .context("Failed to read holdings response body")?;
        log::info!(
            "Holdings raw response for account_id={}: {}",
            account_id,
            &body_text[..body_text.len().min(2000)]
        );

        // Parse as raw JSON first, then extract what we need — SnapTrade's response
        // shape varies and strict typed parsing breaks on unknown nested objects.
        let raw: serde_json::Value = serde_json::from_str(&body_text)
            .context("Failed to parse holdings response as JSON")?;

        let balances: Option<Vec<SnapTradeBalance>> = raw
            .get("balances")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let positions: Option<Vec<SnapTradePosition>> =
            raw.get("positions").and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .map(|p| SnapTradePosition {
                        symbol: p
                            .get("symbol")
                            .and_then(|s| serde_json::from_value(s.clone()).ok()),
                        units: p
                            .get("units")
                            .and_then(|v| v.as_f64())
                            .or_else(|| p.get("fractional_units").and_then(|v| v.as_f64())),
                        price: p.get("price").and_then(|v| v.as_f64()),
                        open_pnl: p.get("open_pnl").and_then(|v| v.as_f64()),
                        average_purchase_price: p
                            .get("average_purchase_price")
                            .and_then(|v| v.as_f64()),
                        currency: p
                            .get("currency")
                            .and_then(|c| serde_json::from_value(c.clone()).ok()),
                        extra: serde_json::Value::Null,
                    })
                    .collect()
            });

        let option_positions: Option<Vec<SnapTradeOptionPosition>> = raw
            .get("option_positions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|p| SnapTradeOptionPosition {
                        option_symbol: p
                            .get("option_symbol")
                            .and_then(|s| serde_json::from_value(s.clone()).ok()),
                        units: p.get("units").and_then(|v| v.as_f64()),
                        price: p.get("price").and_then(|v| v.as_f64()),
                        average_purchase_price: p
                            .get("average_purchase_price")
                            .and_then(|v| v.as_f64()),
                        extra: serde_json::Value::Null,
                    })
                    .collect()
            });

        Ok(SnapTradeHoldingsResponse {
            account: raw.get("account").cloned(),
            balances,
            positions,
            option_positions,
            orders: raw
                .get("orders")
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
            total_value: raw
                .get("total_value")
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
            extra: serde_json::Value::Null,
        })
    }
}
