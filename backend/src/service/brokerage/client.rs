use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use hmac::{Hmac, Mac};
use hyper_util::rt::TokioIo;
use prost::Message;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint, Uri};
use tonic::{Request, Response, Status};
use tower::service_fn;

const CONTRACT_VERSION: &str = "2026-08-09";
const DEFAULT_SOCKET_PATH: &str = "/tmp/tradstry-snaptrade.sock";
const MAX_RATE_LIMIT_RETRIES: usize = 3;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
const REGISTER_USER_RPC: &str = "/tradstry.snaptrade.v1.SnapTradeAdapterService/RegisterUser";
const DELETE_USER_RPC: &str = "/tradstry.snaptrade.v1.SnapTradeAdapterService/DeleteUser";
const INITIATE_CONNECTION_RPC: &str =
    "/tradstry.snaptrade.v1.SnapTradeAdapterService/InitiateConnection";
const GET_CONNECTION_RPC: &str = "/tradstry.snaptrade.v1.SnapTradeAdapterService/GetConnection";
const REFRESH_CONNECTION_RPC: &str =
    "/tradstry.snaptrade.v1.SnapTradeAdapterService/RefreshConnection";
const DELETE_CONNECTION_RPC: &str =
    "/tradstry.snaptrade.v1.SnapTradeAdapterService/DeleteConnection";
const LIST_ACCOUNTS_RPC: &str = "/tradstry.snaptrade.v1.SnapTradeAdapterService/ListAccounts";
const GET_PORTFOLIO_SNAPSHOT_RPC: &str =
    "/tradstry.snaptrade.v1.SnapTradeAdapterService/GetPortfolioSnapshot";
const GET_ACTIVITIES_RPC: &str = "/tradstry.snaptrade.v1.SnapTradeAdapterService/GetActivities";

pub mod proto {
    tonic::include_proto!("tradstry.snaptrade.v1");
}

#[derive(Debug)]
pub enum SnapTradeError {
    StaleCredentials,
    RateLimited {
        retry_after_seconds: u64,
    },
    Upstream {
        code: String,
        message: String,
        retryable: bool,
        status: u16,
        upstream_code: Option<String>,
    },
}

impl std::fmt::Display for SnapTradeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleCredentials => {
                write!(formatter, "SnapTrade credentials need reauthorization")
            }
            Self::RateLimited {
                retry_after_seconds,
            } => {
                write!(
                    formatter,
                    "SnapTrade rate limited; retry after {retry_after_seconds}s"
                )
            }
            Self::Upstream {
                code,
                message,
                status,
                ..
            } => {
                write!(
                    formatter,
                    "SnapTrade adapter error {code} ({status}): {message}"
                )
            }
        }
    }
}

impl std::error::Error for SnapTradeError {}

#[derive(Clone)]
pub struct BrokerageClient {
    channel: Channel,
    internal_secret: Arc<str>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradePagination {
    pub offset: Option<i32>,
    pub limit: Option<i32>,
    pub total: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeTransactionsResponse {
    #[serde(rename = "activities")]
    pub data: Vec<SnapTradeActivity>,
    pub pagination: Option<SnapTradePagination>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeActivity {
    pub id: Option<String>,
    pub symbol: Option<SnapTradeSymbol>,
    pub option_symbol: Option<SnapTradeOptionSymbol>,
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
    pub id: Option<String>,
    pub symbol: Option<String>,
    pub raw_symbol: Option<String>,
    pub description: Option<String>,
    pub currency: Option<SnapTradeCurrency>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeCurrency {
    pub id: Option<String>,
    pub code: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeOptionSymbol {
    pub id: Option<String>,
    pub ticker: Option<String>,
    pub option_type: Option<String>,
    pub strike_price: Option<f64>,
    pub expiration_date: Option<String>,
    pub is_mini_option: Option<bool>,
    pub underlying_symbol: Option<SnapTradeSymbol>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeHoldingsResponse {
    pub account_id: String,
    pub as_of: Option<String>,
    pub complete: bool,
    pub holdings_unavailable: bool,
    pub positions: Vec<SnapTradePosition>,
    pub balances: Vec<SnapTradeBalance>,
    pub orders: Vec<SnapTradeOrder>,
    pub total_value: Option<SnapTradeTotalValue>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeTotalValue {
    pub amount: Option<f64>,
    pub currency: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeBalance {
    pub currency: String,
    pub cash: Option<f64>,
    pub buying_power: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradePosition {
    pub instrument_id: String,
    pub kind: String,
    pub symbol: String,
    pub raw_symbol: Option<String>,
    pub description: Option<String>,
    pub currency: Option<String>,
    pub units: Option<f64>,
    pub price: Option<f64>,
    pub average_purchase_price: Option<f64>,
    pub option: Option<SnapTradeOptionDetails>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeOptionDetails {
    pub option_type: String,
    pub strike_price: f64,
    pub expiration_date: String,
    pub multiplier: f64,
    pub underlying_symbol: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeOrder {
    pub brokerage_order_id: String,
    pub symbol: Option<String>,
    pub option_symbol: Option<String>,
    pub status: Option<String>,
    pub action: Option<String>,
    pub order_type: Option<String>,
    pub units: Option<f64>,
    pub price: Option<f64>,
    pub time_placed: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeAccount {
    pub id: Option<String>,
    pub brokerage_authorization: Option<String>,
    pub name: Option<String>,
    pub number: Option<String>,
    pub institution_name: Option<String>,
    pub sync_status: Option<SnapTradeSyncStatus>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SnapTradeSyncStatus {
    pub transactions: Option<TransactionsSyncStatus>,
    pub holdings: Option<HoldingsSyncStatus>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TransactionsSyncStatus {
    pub initial_sync_completed: Option<bool>,
    pub last_successful_sync: Option<String>,
    pub first_transaction_date: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HoldingsSyncStatus {
    pub initial_sync_completed: Option<bool>,
    pub last_successful_sync: Option<String>,
    #[serde(default)]
    pub holdings_unavailable: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConnectionStatus {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub connection_type: Option<String>,
    pub disabled: Option<bool>,
    pub disabled_date: Option<String>,
    pub data_freshness_mode: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateUserResponse {
    pub user_id: String,
    pub user_secret: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InitiateConnectionResponse {
    pub redirect_url: String,
    pub session_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RefreshConnectionResponse {
    pub connection_id: String,
    pub status: String,
}

trait AuthenticatedMessage: Message + Clone {
    fn set_auth(&mut self, auth: proto::RequestAuth);
}

macro_rules! authenticated_messages {
    ($($request:ty),+ $(,)?) => {
        $(impl AuthenticatedMessage for $request {
            fn set_auth(&mut self, auth: proto::RequestAuth) {
                self.auth = Some(auth);
            }
        })+
    };
}

authenticated_messages!(
    proto::RegisterUserRequest,
    proto::DeleteUserRequest,
    proto::InitiateConnectionRequest,
    proto::GetConnectionRequest,
    proto::ListConnectionsRequest,
    proto::RefreshConnectionRequest,
    proto::DeleteConnectionRequest,
    proto::ListAccountsRequest,
    proto::GetAccountRequest,
    proto::GetPortfolioSnapshotRequest,
    proto::GetActivitiesRequest,
);

impl BrokerageClient {
    pub fn from_env() -> Result<Self> {
        let socket_path = std::env::var("SNAPTRADE_GRPC_SOCKET")
            .unwrap_or_else(|_| DEFAULT_SOCKET_PATH.to_string());
        anyhow::ensure!(
            PathBuf::from(&socket_path).is_absolute(),
            "SNAPTRADE_GRPC_SOCKET must be an absolute path"
        );
        let internal_secret = std::env::var("SNAPTRADE_INTERNAL_SECRET")
            .context("SNAPTRADE_INTERNAL_SECRET not set")?;
        anyhow::ensure!(
            internal_secret.len() >= 32,
            "SNAPTRADE_INTERNAL_SECRET must contain at least 32 bytes"
        );
        let socket_path = Arc::new(PathBuf::from(socket_path));
        let channel = Endpoint::try_from("http://[::]:50051")?
            .connect_timeout(Duration::from_secs(5))
            .timeout(REQUEST_TIMEOUT)
            .tcp_nodelay(true)
            .connect_with_connector_lazy(service_fn(move |_: Uri| {
                let socket_path = socket_path.clone();
                async move {
                    UnixStream::connect(socket_path.as_path())
                        .await
                        .map(TokioIo::new)
                }
            }));
        Ok(Self {
            channel,
            internal_secret: Arc::from(internal_secret),
        })
    }

    pub async fn register_user(&self, user_id: &str) -> Result<CreateUserResponse> {
        let response = self
            .call(
                REGISTER_USER_RPC,
                proto::RegisterUserRequest {
                    auth: None,
                    user_id: user_id.to_string(),
                },
                |mut client, request| async move { client.register_user(request).await },
            )
            .await?;
        validate_meta(response.meta.as_ref())?;
        let value = response
            .user
            .context("SnapTrade adapter omitted registered user")?;
        Ok(CreateUserResponse {
            user_id: value.user_id,
            user_secret: value.user_secret,
        })
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<()> {
        let response = self
            .call(
                DELETE_USER_RPC,
                proto::DeleteUserRequest {
                    auth: None,
                    user_id: user_id.to_string(),
                },
                |mut client, request| async move { client.delete_user(request).await },
            )
            .await?;
        validate_meta(response.meta.as_ref())?;
        anyhow::ensure!(
            response.accepted,
            "SnapTrade adapter did not accept user deletion"
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn initiate_connection(
        &self,
        user_id: &str,
        user_secret: &str,
        brokerage_id: &str,
        connection_type: Option<&str>,
        reconnect: Option<&str>,
        custom_redirect: Option<&str>,
    ) -> Result<InitiateConnectionResponse> {
        let response = self
            .call(
                INITIATE_CONNECTION_RPC,
                proto::InitiateConnectionRequest {
                    auth: None,
                    user_id: user_id.to_string(),
                    user_secret: user_secret.to_string(),
                    brokerage_id: optional_string(brokerage_id),
                    connection_type: connection_type.unwrap_or("read").to_string(),
                    reconnect: reconnect.map(str::to_string),
                    custom_redirect: custom_redirect.map(str::to_string),
                },
                |mut client, request| async move { client.initiate_connection(request).await },
            )
            .await?;
        validate_meta(response.meta.as_ref())?;
        let value = response
            .portal
            .context("SnapTrade adapter omitted connection portal")?;
        Ok(InitiateConnectionResponse {
            redirect_url: value.redirect_url,
            session_id: value.session_id,
        })
    }

    pub async fn list_snaptrade_accounts(
        &self,
        user_id: &str,
        user_secret: &str,
    ) -> Result<Vec<SnapTradeAccount>> {
        let response = self
            .call(
                LIST_ACCOUNTS_RPC,
                proto::ListAccountsRequest {
                    auth: None,
                    credentials: Some(credentials(user_id, user_secret)),
                },
                |mut client, request| async move { client.list_accounts(request).await },
            )
            .await?;
        validate_meta(response.meta.as_ref())?;
        Ok(response
            .accounts
            .into_iter()
            .map(account_from_proto)
            .collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn fetch_transactions(
        &self,
        user_id: &str,
        user_secret: &str,
        snaptrade_account_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
        transaction_type: Option<&str>,
        offset: Option<i32>,
        limit: Option<i32>,
    ) -> Result<SnapTradeTransactionsResponse> {
        let response = self
            .call(
                GET_ACTIVITIES_RPC,
                proto::GetActivitiesRequest {
                    auth: None,
                    credentials: Some(credentials(user_id, user_secret)),
                    account_id: snaptrade_account_id.to_string(),
                    start_date: start_date.map(str::to_string),
                    end_date: end_date.map(str::to_string),
                    activity_type: transaction_type.map(str::to_string),
                    offset,
                    limit,
                },
                |mut client, request| async move { client.get_activities(request).await },
            )
            .await?;
        validate_meta(response.meta.as_ref())?;
        let page = response
            .page
            .context("SnapTrade adapter omitted activities page")?;
        Ok(SnapTradeTransactionsResponse {
            data: page
                .activities
                .into_iter()
                .map(activity_from_proto)
                .collect(),
            pagination: page.pagination.map(|value| SnapTradePagination {
                offset: value.offset,
                limit: value.limit,
                total: value.total,
            }),
        })
    }

    pub async fn fetch_holdings(
        &self,
        user_id: &str,
        user_secret: &str,
        snaptrade_account_id: &str,
    ) -> Result<SnapTradeHoldingsResponse> {
        let response = self
            .call(
                GET_PORTFOLIO_SNAPSHOT_RPC,
                proto::GetPortfolioSnapshotRequest {
                    auth: None,
                    credentials: Some(credentials(user_id, user_secret)),
                    account_id: snaptrade_account_id.to_string(),
                },
                |mut client, request| async move { client.get_portfolio_snapshot(request).await },
            )
            .await?;
        validate_meta(response.meta.as_ref())?;
        let value = response
            .snapshot
            .context("SnapTrade adapter omitted portfolio snapshot")?;
        Ok(portfolio_from_proto(value))
    }

    pub async fn get_connection_status(
        &self,
        user_id: &str,
        user_secret: &str,
        connection_id: &str,
    ) -> Result<ConnectionStatus> {
        let response = self
            .call(
                GET_CONNECTION_RPC,
                proto::GetConnectionRequest {
                    auth: None,
                    credentials: Some(credentials(user_id, user_secret)),
                    connection_id: connection_id.to_string(),
                },
                |mut client, request| async move { client.get_connection(request).await },
            )
            .await?;
        validate_meta(response.meta.as_ref())?;
        let value = response
            .connection
            .context("SnapTrade adapter omitted connection")?;
        Ok(ConnectionStatus {
            id: optional_owned(value.id),
            name: value.name,
            connection_type: value.connection_type,
            disabled: Some(value.disabled),
            disabled_date: value.disabled_date,
            data_freshness_mode: value.data_freshness_mode,
        })
    }

    pub async fn refresh_connection(
        &self,
        user_id: &str,
        user_secret: &str,
        connection_id: &str,
    ) -> Result<RefreshConnectionResponse> {
        let response = self
            .call(
                REFRESH_CONNECTION_RPC,
                proto::RefreshConnectionRequest {
                    auth: None,
                    credentials: Some(credentials(user_id, user_secret)),
                    connection_id: connection_id.to_string(),
                },
                |mut client, request| async move { client.refresh_connection(request).await },
            )
            .await?;
        validate_meta(response.meta.as_ref())?;
        let value = response
            .result
            .context("SnapTrade adapter omitted refresh result")?;
        Ok(RefreshConnectionResponse {
            connection_id: value.connection_id,
            status: value.status,
        })
    }

    pub async fn delete_connection(
        &self,
        user_id: &str,
        user_secret: &str,
        connection_id: &str,
    ) -> Result<()> {
        let response = self
            .call(
                DELETE_CONNECTION_RPC,
                proto::DeleteConnectionRequest {
                    auth: None,
                    credentials: Some(credentials(user_id, user_secret)),
                    connection_id: connection_id.to_string(),
                },
                |mut client, request| async move { client.delete_connection(request).await },
            )
            .await?;
        validate_meta(response.meta.as_ref())?;
        anyhow::ensure!(
            response.deleted,
            "SnapTrade adapter did not confirm connection deletion"
        );
        Ok(())
    }

    async fn call<M, T, F, Fut>(&self, method: &'static str, message: M, mut rpc: F) -> Result<T>
    where
        M: AuthenticatedMessage + Send + Sync + 'static,
        T: Send + 'static,
        F: FnMut(
            proto::snap_trade_adapter_service_client::SnapTradeAdapterServiceClient<Channel>,
            Request<M>,
        ) -> Fut,
        Fut: Future<Output = std::result::Result<Response<T>, Status>>,
    {
        for attempt in 0..=MAX_RATE_LIMIT_RETRIES {
            let mut message = message.clone();
            message.set_auth(self.auth(method, &message.encode_to_vec())?);
            let mut request = Request::new(message);
            request.set_timeout(REQUEST_TIMEOUT);
            let client =
                proto::snap_trade_adapter_service_client::SnapTradeAdapterServiceClient::new(
                    self.channel.clone(),
                )
                .max_decoding_message_size(8 << 20)
                .max_encoding_message_size(1 << 20);
            match rpc(client, request).await {
                Ok(response) => return Ok(response.into_inner()),
                Err(status) => {
                    let error = parse_adapter_error(&status);
                    if let SnapTradeError::RateLimited {
                        retry_after_seconds,
                    } = &error
                        && attempt < MAX_RATE_LIMIT_RETRIES
                    {
                        let jitter_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .subsec_millis() as u64
                            % 250;
                        tokio::time::sleep(
                            Duration::from_secs((*retry_after_seconds).clamp(1, 30))
                                + Duration::from_millis(jitter_ms),
                        )
                        .await;
                        continue;
                    }
                    return Err(anyhow!(error));
                }
            }
        }
        unreachable!("rate-limit retry loop always returns")
    }

    fn auth(&self, method: &str, payload: &[u8]) -> Result<proto::RequestAuth> {
        let unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before Unix epoch")?
            .as_secs();
        let nonce = uuid::Uuid::new_v4().to_string();
        Ok(proto::RequestAuth {
            unix_seconds: unix_seconds
                .try_into()
                .context("internal timestamp overflow")?,
            signature: sign_request(&self.internal_secret, unix_seconds, method, &nonce, payload)?,
            nonce,
        })
    }
}

fn sign_request(
    secret: &str,
    unix_seconds: u64,
    method: &str,
    nonce: &str,
    payload: &[u8],
) -> Result<String> {
    let payload_hash = hex::encode(Sha256::digest(payload));
    let message = format!("{unix_seconds}\n{method}\n{nonce}\n{payload_hash}");
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| anyhow!("invalid internal signing key"))?;
    mac.update(message.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn credentials(user_id: &str, user_secret: &str) -> proto::Credentials {
    proto::Credentials {
        user_id: user_id.to_string(),
        user_secret: user_secret.to_string(),
    }
}

fn optional_string(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

fn optional_owned(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn validate_meta(meta: Option<&proto::ResponseMeta>) -> Result<()> {
    let meta = meta.context("SnapTrade adapter omitted response metadata")?;
    anyhow::ensure!(
        meta.contract_version == CONTRACT_VERSION,
        "unsupported SnapTrade adapter contract {}; expected {}",
        meta.contract_version,
        CONTRACT_VERSION
    );
    if let Some(rate) = &meta.rate_limit {
        tracing::debug!(
            request_id = meta.request_id.as_deref().unwrap_or_default(),
            remaining = rate.remaining,
            account_remaining = rate.account_remaining,
            reset_seconds = rate.reset_seconds,
            account_reset_seconds = rate.account_reset_seconds,
            "SnapTrade adapter rate-limit state"
        );
    }
    Ok(())
}

fn metadata<'a>(status: &'a Status, key: &'static str) -> Option<&'a str> {
    status
        .metadata()
        .get(key)
        .and_then(|value| value.to_str().ok())
}

fn parse_adapter_error(status: &Status) -> SnapTradeError {
    let upstream_code = metadata(status, "x-upstream-code").map(str::to_string);
    if upstream_code.as_deref() == Some("1083") {
        return SnapTradeError::StaleCredentials;
    }
    let adapter_code = metadata(status, "x-adapter-code").unwrap_or("GRPC_FAILURE");
    if status.code() == tonic::Code::ResourceExhausted || adapter_code == "RATE_LIMITED" {
        return SnapTradeError::RateLimited {
            retry_after_seconds: metadata(status, "x-retry-after-seconds")
                .and_then(|value| value.parse().ok())
                .unwrap_or(1),
        };
    }
    SnapTradeError::Upstream {
        code: adapter_code.to_string(),
        message: status.message().to_string(),
        retryable: metadata(status, "x-retryable").is_some_and(|value| value == "true")
            || matches!(
                status.code(),
                tonic::Code::Unavailable | tonic::Code::DeadlineExceeded
            ),
        status: metadata(status, "x-upstream-status")
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| grpc_status_code(status.code())),
        upstream_code,
    }
}

fn grpc_status_code(code: tonic::Code) -> u16 {
    match code {
        tonic::Code::InvalidArgument => 400,
        tonic::Code::Unauthenticated => 401,
        tonic::Code::PermissionDenied => 403,
        tonic::Code::NotFound => 404,
        tonic::Code::Aborted | tonic::Code::AlreadyExists => 409,
        tonic::Code::ResourceExhausted => 429,
        tonic::Code::DeadlineExceeded => 504,
        tonic::Code::Unavailable => 503,
        _ => 502,
    }
}

fn account_from_proto(value: proto::Account) -> SnapTradeAccount {
    SnapTradeAccount {
        id: optional_owned(value.id),
        brokerage_authorization: value.brokerage_authorization,
        name: value.name,
        number: value.number,
        institution_name: value.institution_name,
        sync_status: value.sync_status.map(|status| SnapTradeSyncStatus {
            transactions: status.transactions.map(|item| TransactionsSyncStatus {
                initial_sync_completed: item.initial_sync_completed,
                last_successful_sync: item.last_successful_sync,
                first_transaction_date: item.first_transaction_date,
            }),
            holdings: status.holdings.map(|item| HoldingsSyncStatus {
                initial_sync_completed: item.initial_sync_completed,
                last_successful_sync: item.last_successful_sync,
                holdings_unavailable: item.holdings_unavailable,
            }),
        }),
    }
}

fn portfolio_from_proto(value: proto::PortfolioSnapshot) -> SnapTradeHoldingsResponse {
    SnapTradeHoldingsResponse {
        account_id: value.account_id,
        as_of: value.as_of,
        complete: value.complete,
        holdings_unavailable: value.holdings_unavailable,
        positions: value
            .positions
            .into_iter()
            .map(|item| SnapTradePosition {
                instrument_id: item.instrument_id,
                kind: item.kind,
                symbol: item.symbol,
                raw_symbol: item.raw_symbol,
                description: item.description,
                currency: item.currency,
                units: item.units,
                price: item.price,
                average_purchase_price: item.average_purchase_price,
                option: item.option.map(|option| SnapTradeOptionDetails {
                    option_type: option.option_type,
                    strike_price: option.strike_price,
                    expiration_date: option.expiration_date,
                    multiplier: option.multiplier,
                    underlying_symbol: option.underlying_symbol,
                }),
            })
            .collect(),
        balances: value
            .balances
            .into_iter()
            .map(|item| SnapTradeBalance {
                currency: item.currency,
                cash: item.cash,
                buying_power: item.buying_power,
            })
            .collect(),
        orders: value
            .orders
            .into_iter()
            .map(|item| SnapTradeOrder {
                brokerage_order_id: item.brokerage_order_id,
                symbol: item.symbol,
                option_symbol: item.option_symbol,
                status: item.status,
                action: item.action,
                order_type: item.order_type,
                units: item.units,
                price: item.price,
                time_placed: item.time_placed,
            })
            .collect(),
        total_value: value.total_value.map(|money| SnapTradeTotalValue {
            amount: money.amount,
            currency: money.currency,
        }),
    }
}

fn activity_from_proto(value: proto::Activity) -> SnapTradeActivity {
    SnapTradeActivity {
        id: value.id,
        symbol: value.symbol.map(symbol_from_proto),
        option_symbol: value.option_symbol.map(|item| SnapTradeOptionSymbol {
            id: item.id,
            ticker: item.ticker,
            option_type: item.option_type,
            strike_price: item.strike_price,
            expiration_date: item.expiration_date,
            is_mini_option: item.is_mini_option,
            underlying_symbol: item.underlying_symbol.map(symbol_from_proto),
        }),
        price: value.price,
        units: value.units,
        amount: value.amount,
        currency: value.currency.map(currency_from_proto),
        activity_type: value.activity_type,
        option_type: value.option_type,
        description: value.description,
        trade_date: value.trade_date,
        settlement_date: value.settlement_date,
        fee: value.fee,
        fx_rate: value.fx_rate,
        institution: value.institution,
        external_reference_id: value.external_reference_id,
    }
}

fn symbol_from_proto(value: proto::ActivitySymbol) -> SnapTradeSymbol {
    SnapTradeSymbol {
        id: value.id,
        symbol: value.symbol,
        raw_symbol: value.raw_symbol,
        description: value.description,
        currency: value.currency.map(currency_from_proto),
    }
}

fn currency_from_proto(value: proto::Currency) -> SnapTradeCurrency {
    SnapTradeCurrency {
        id: value.id,
        code: value.code,
        name: value.name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_credentials_require_structured_upstream_signal() {
        let mut status = Status::unauthenticated("rejected");
        status
            .metadata_mut()
            .insert("x-upstream-code", "1083".parse().unwrap());
        assert!(matches!(
            parse_adapter_error(&status),
            SnapTradeError::StaleCredentials
        ));

        assert!(!matches!(
            parse_adapter_error(&Status::unauthenticated("rejected")),
            SnapTradeError::StaleCredentials
        ));
    }

    #[test]
    fn authentication_covers_rpc_and_payload() {
        let first = sign_request(
            "test-only-secret-that-is-long-enough",
            100,
            "/service/one",
            "nonce",
            b"payload",
        )
        .unwrap();
        let second = sign_request(
            "test-only-secret-that-is-long-enough",
            100,
            "/service/two",
            "nonce",
            b"payload",
        )
        .unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn authentication_matches_cross_language_vector() {
        let request = proto::RegisterUserRequest {
            auth: None,
            user_id: "user-1".to_string(),
        };
        let payload = request.encode_to_vec();
        assert_eq!(hex::encode(&payload), "1206757365722d31");
        assert_eq!(
            sign_request(
                "test-only-secret-that-is-at-least-32-bytes",
                1_800_000_000,
                REGISTER_USER_RPC,
                "fixed-nonce",
                &payload,
            )
            .unwrap(),
            "ff913ae344665be19c6fd7de649a8420ea4f0bb5264d4cf7fff9703ab8237a86"
        );
    }
}
