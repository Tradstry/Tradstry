use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Error as WebSocketError, Message},
};

const POLYGON_STOCKS_WEBSOCKET_URL: &str = "wss://socket.polygon.io/stocks";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const RECONNECT_DELAY: Duration = Duration::from_secs(3);
const CHANNEL_CAPACITY: usize = 256;

type PolygonSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(Debug, Clone, PartialEq)]
pub struct PriceUpdate {
    pub symbol: String,
    pub price: f64,
    pub timestamp_ms: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum PolygonStreamError {
    #[error("POLYGON_API_KEY is not set")]
    MissingApiKey,
    #[error("Polygon WebSocket connection failed: {0}")]
    Connection(String),
    #[error("Polygon WebSocket authentication failed: {0}")]
    Authentication(String),
    #[error("Polygon WebSocket timed out during authentication")]
    AuthenticationTimeout,
    #[error("Polygon WebSocket subscription failed: {0}")]
    Subscription(String),
}

enum IncomingEvent {
    Price(PriceUpdate),
    Status { status: String, message: String },
}

pub async fn subscribe(
    symbols: Vec<String>,
) -> Result<ReceiverStream<PriceUpdate>, PolygonStreamError> {
    let api_key = std::env::var("POLYGON_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .ok_or(PolygonStreamError::MissingApiKey)?;
    let socket = connect(&api_key, &symbols).await?;
    let (sender, receiver) = mpsc::channel(CHANNEL_CAPACITY);

    tokio::spawn(run(socket, api_key, symbols, sender));
    Ok(ReceiverStream::new(receiver))
}

async fn connect(api_key: &str, symbols: &[String]) -> Result<PolygonSocket, PolygonStreamError> {
    let socket = tokio::time::timeout(CONNECT_TIMEOUT, connect_async(POLYGON_STOCKS_WEBSOCKET_URL))
        .await
        .map_err(|_| PolygonStreamError::Connection("connection timed out".to_string()))?
        .map_err(|error| PolygonStreamError::Connection(error.to_string()))?
        .0;
    authenticate_and_subscribe(socket, api_key, symbols).await
}

async fn authenticate_and_subscribe(
    mut socket: PolygonSocket,
    api_key: &str,
    symbols: &[String],
) -> Result<PolygonSocket, PolygonStreamError> {
    socket
        .send(Message::Text(
            json!({ "action": "auth", "params": api_key })
                .to_string()
                .into(),
        ))
        .await
        .map_err(|error| PolygonStreamError::Authentication(error.to_string()))?;

    tokio::time::timeout(CONNECT_TIMEOUT, async {
        while let Some(message) = socket.next().await {
            let message =
                message.map_err(|error| PolygonStreamError::Authentication(error.to_string()))?;
            if let Message::Text(text) = message {
                for event in parse_events(&text) {
                    if let IncomingEvent::Status { status, message } = event {
                        if status == "auth_success" {
                            return Ok(());
                        }
                        if status == "auth_failed" || status == "not_authorized" {
                            return Err(PolygonStreamError::Authentication(message));
                        }
                    }
                }
            }
        }
        Err(PolygonStreamError::Authentication(
            "connection closed before authentication completed".to_string(),
        ))
    })
    .await
    .map_err(|_| PolygonStreamError::AuthenticationTimeout)??;

    let channels = symbols
        .iter()
        .map(|symbol| format!("A.{symbol}"))
        .collect::<Vec<_>>()
        .join(",");
    socket
        .send(Message::Text(
            json!({ "action": "subscribe", "params": channels })
                .to_string()
                .into(),
        ))
        .await
        .map_err(|error| PolygonStreamError::Subscription(error.to_string()))?;

    tokio::time::timeout(CONNECT_TIMEOUT, async {
        while let Some(message) = socket.next().await {
            let message =
                message.map_err(|error| PolygonStreamError::Subscription(error.to_string()))?;
            match message {
                Message::Text(text) => {
                    for event in parse_events(&text) {
                        if let IncomingEvent::Status { status, message } = event {
                            if status == "success" {
                                log::info!("Polygon WebSocket subscription active");
                                return Ok(());
                            }
                            if status == "error"
                                || status == "auth_failed"
                                || status == "not_authorized"
                            {
                                return Err(PolygonStreamError::Subscription(message));
                            }
                        }
                    }
                }
                Message::Ping(payload) => socket
                    .send(Message::Pong(payload))
                    .await
                    .map_err(|error| PolygonStreamError::Subscription(error.to_string()))?,
                Message::Close(_) => {
                    return Err(PolygonStreamError::Subscription(
                        "connection closed before subscription completed".to_string(),
                    ));
                }
                _ => {}
            }
        }
        Err(PolygonStreamError::Subscription(
            "connection closed before subscription completed".to_string(),
        ))
    })
    .await
    .map_err(|_| {
        PolygonStreamError::Subscription("subscription confirmation timed out".to_string())
    })??;
    Ok(socket)
}

async fn run(
    mut socket: PolygonSocket,
    api_key: String,
    symbols: Vec<String>,
    sender: mpsc::Sender<PriceUpdate>,
) {
    loop {
        loop {
            tokio::select! {
                _ = sender.closed() => return,
                message = socket.next() => {
                    match message {
                        Some(Ok(Message::Text(text))) => {
                            for event in parse_events(&text) {
                                match event {
                                    IncomingEvent::Price(update) => {
                                        if sender.send(update).await.is_err() {
                                            return;
                                        }
                                    }
                                    IncomingEvent::Status { status, message }
                                        if status == "error"
                                            || status == "auth_failed"
                                            || status == "not_authorized" =>
                                    {
                                        log::error!("Polygon WebSocket authorization error: {message}");
                                        break;
                                    }
                                    IncomingEvent::Status { .. } => {}
                                }
                            }
                        }
                        Some(Ok(Message::Ping(payload))) => {
                            if socket.send(Message::Pong(payload)).await.is_err() {
                                break;
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Err(error)) => {
                            log_websocket_error(&error);
                            break;
                        }
                        Some(Ok(_)) => {}
                    }
                }
            }
        }

        if sender.is_closed() {
            return;
        }
        log::warn!("Polygon WebSocket disconnected; reconnecting in 3 seconds");
        tokio::time::sleep(RECONNECT_DELAY).await;
        loop {
            if sender.is_closed() {
                return;
            }
            match connect(&api_key, &symbols).await {
                Ok(reconnected) => {
                    socket = reconnected;
                    log::info!("Polygon WebSocket reconnected");
                    break;
                }
                Err(error) => {
                    log::warn!("Polygon WebSocket reconnect failed: {error}");
                    tokio::time::sleep(RECONNECT_DELAY).await;
                }
            }
        }
    }
}

fn log_websocket_error(error: &WebSocketError) {
    log::warn!("Polygon WebSocket read failed: {error}");
}

fn parse_events(text: &str) -> Vec<IncomingEvent> {
    let Ok(events) = serde_json::from_str::<Vec<Value>>(text) else {
        log::warn!("Ignoring invalid Polygon WebSocket message");
        return Vec::new();
    };

    events
        .into_iter()
        .filter_map(|event| match event.get("ev").and_then(Value::as_str) {
            Some("T") => price_event(&event, "p", "t"),
            Some("A" | "AM") => price_event(&event, "c", "e"),
            Some("status") => Some(IncomingEvent::Status {
                status: event
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                message: event
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Polygon returned an unknown status")
                    .to_string(),
            }),
            _ => None,
        })
        .collect()
}

fn price_event(event: &Value, price_field: &str, timestamp_field: &str) -> Option<IncomingEvent> {
    Some(IncomingEvent::Price(PriceUpdate {
        symbol: event.get("sym")?.as_str()?.to_string(),
        price: event.get(price_field)?.as_f64()?,
        timestamp_ms: event.get(timestamp_field)?.as_i64()?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_aggregate_in_a_polygon_batch() {
        let updates = parse_events(
            r#"[{"ev":"A","sym":"AAPL","c":186.19,"e":1705363200000},{"ev":"A","sym":"NVDA","c":590.20,"e":1705363201000}]"#,
        )
        .into_iter()
        .filter_map(|event| match event {
            IncomingEvent::Price(update) => Some(update),
            IncomingEvent::Status { .. } => None,
        })
        .collect::<Vec<_>>();

        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].symbol, "AAPL");
        assert_eq!(updates[1].price, 590.20);
    }

    #[test]
    fn parses_polygon_authentication_status() {
        let events =
            parse_events(r#"[{"ev":"status","status":"auth_success","message":"authenticated"}]"#);
        assert!(matches!(
            events.as_slice(),
            [IncomingEvent::Status { status, .. }] if status == "auth_success"
        ));
    }
}
