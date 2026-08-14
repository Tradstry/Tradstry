//! OAuth 2.0 Protected Resource Metadata (RFC 9728)
//!
//! Serves the RFC 9728 protected-resource document at both the origin-level
//! and path-specific well-known URLs. MCP clients differ on which discovery
//! form they probe, so both routes intentionally return the same document.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::app_state::AppState;

pub const ROOT_METADATA_PATH: &str = "/.well-known/oauth-protected-resource";
pub const MCP_METADATA_PATH: &str = "/.well-known/oauth-protected-resource/mcp";

/// Build the RFC 9728 Protected Resource Metadata document.
///
/// - `resource`    — the canonical URL of this resource server (from `MCP_PUBLIC_URL`).
/// - `auth_server` — the Clerk issuer URL (from `CLERK_ISSUER`).
pub fn protected_resource_metadata(resource: &str, auth_server: &str) -> Value {
    json!({
        "resource": resource,
        "authorization_servers": [auth_server],
        "bearer_methods_supported": ["header"]
    })
}

fn discovery_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Authorization, Content-Type, MCP-Protocol-Version"),
    );
    headers
}

/// Public discovery handler. CORS is included because some MCP clients fetch
/// well-known metadata from a browser context before starting OAuth.
pub async fn handler(State(state): State<Arc<AppState>>) -> Response {
    (
        discovery_headers(),
        Json(protected_resource_metadata(
            &state.public_url,
            &state.clerk_issuer,
        )),
    )
        .into_response()
}

/// CORS preflight for both protected-resource discovery URLs.
pub async fn options_handler() -> Response {
    (StatusCode::NO_CONTENT, discovery_headers()).into_response()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertises_clerk_as_auth_server() {
        let doc =
            protected_resource_metadata("https://mcp.tradstry.com", "https://clerk.tradstry.com");
        assert_eq!(doc["resource"], "https://mcp.tradstry.com");
        assert_eq!(
            doc["authorization_servers"][0],
            "https://clerk.tradstry.com"
        );
        assert_eq!(doc["bearer_methods_supported"][0], "header");
    }

    #[test]
    fn exposes_origin_and_path_specific_discovery_locations() {
        assert_eq!(ROOT_METADATA_PATH, "/.well-known/oauth-protected-resource");
        assert_eq!(
            MCP_METADATA_PATH,
            "/.well-known/oauth-protected-resource/mcp"
        );
    }

    #[test]
    fn discovery_allows_browser_preflight() {
        let headers = discovery_headers();
        assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert_eq!(
            headers[header::ACCESS_CONTROL_ALLOW_METHODS],
            "GET, OPTIONS"
        );
        assert!(
            headers[header::ACCESS_CONTROL_ALLOW_HEADERS]
                .to_str()
                .unwrap()
                .contains("MCP-Protocol-Version")
        );
    }
}
