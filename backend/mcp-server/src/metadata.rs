//! OAuth 2.0 Protected Resource Metadata (RFC 9728)
//!
//! Serves the `/.well-known/oauth-protected-resource` document that MCP
//! clients (e.g. claude.ai) fetch to discover which authorization server
//! (Clerk) they must authenticate against before calling `/mcp`.

use serde_json::{Value, json};

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

/// Axum handler — returns the metadata document as JSON.
///
/// This handler must be mounted on a **public** (un-authenticated) branch of
/// the router so that clients can always reach it regardless of whether they
/// hold a valid token.
pub async fn handler(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::app_state::AppState>>,
) -> axum::Json<Value> {
    axum::Json(protected_resource_metadata(
        &state.public_url,
        &state.clerk_issuer,
    ))
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
}
