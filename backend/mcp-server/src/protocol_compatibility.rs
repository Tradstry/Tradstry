use rmcp::{
    ServerHandler,
    model::{Implementation, ServerCapabilities, ServerInfo},
    transport::streamable_http_server::{
        StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::{Value, json};

use crate::mcp_server_config;

#[derive(Clone, Default)]
struct CompatibilityServer;

impl ServerHandler for CompatibilityServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("tradstry-mcp-test", "1.0.0"))
    }
}

async fn spawn_server() -> (reqwest::Client, String, tokio::task::JoinHandle<()>) {
    let service: StreamableHttpService<CompatibilityServer, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(CompatibilityServer),
            Default::default(),
            mcp_server_config(),
        );
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener should bind");
    let address = listener
        .local_addr()
        .expect("test listener should have an address");
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("test MCP server should run");
    });

    (
        reqwest::Client::new(),
        format!("http://{address}/mcp"),
        server,
    )
}

#[tokio::test]
async fn modern_clients_can_discover_before_initialize() {
    let (client, url, server) = spawn_server().await;
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "server/discover")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": "server-discover-probe-1",
            "method": "server/discover",
            "params": {
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientInfo": {
                        "name": "modern-mcp-client",
                        "version": "2.0.0"
                    },
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }
        }))
        .send()
        .await
        .expect("discovery request should complete");

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response
        .json()
        .await
        .expect("discovery response should be JSON");
    assert_eq!(body["id"], "server-discover-probe-1");
    assert!(
        body["result"]["supportedVersions"]
            .as_array()
            .is_some_and(|versions| versions.iter().any(|version| version == "2026-07-28")),
        "modern protocol version missing from discovery response: {body}"
    );

    let tools_response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "tools/list")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": "tools-list-1",
            "method": "tools/list",
            "params": {
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientInfo": {
                        "name": "modern-mcp-client",
                        "version": "2.0.0"
                    },
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            }
        }))
        .send()
        .await
        .expect("tools/list request should complete");

    assert_eq!(tools_response.status(), reqwest::StatusCode::OK);
    let tools_body: Value = tools_response
        .json()
        .await
        .expect("tools/list response should be JSON");
    assert!(
        tools_body["result"]["tools"].is_array(),
        "modern tools/list should return a tools array: {tools_body}"
    );

    server.abort();
}

#[tokio::test]
async fn claude_legacy_initialize_remains_supported() {
    let (client, url, server) = spawn_server().await;
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {
                    "name": "ClaudeAI",
                    "version": "1.0.0"
                }
            }
        }))
        .send()
        .await
        .expect("legacy initialize request should complete");

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        content_type.contains("text/event-stream"),
        "legacy session response should remain SSE, got {content_type}"
    );
    let body = response
        .text()
        .await
        .expect("legacy response should have a body");
    assert!(
        body.contains("2025-11-25"),
        "legacy initialize response should negotiate Claude's protocol: {body}"
    );

    server.abort();
}
