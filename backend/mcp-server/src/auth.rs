use std::sync::Arc;

use anyhow::anyhow;
use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use clerk_rs::validators::{
    authorizer::{ClerkJwt, validate_jwt},
    jwks::MemoryCacheJwksProvider,
};
use tradstry_backend::service::read_service::users::ensure_user;

use crate::app_state::AppState;
use crate::user_context::UserContext;

// ---------------------------------------------------------------------------
// 401 helper
// ---------------------------------------------------------------------------

/// Return a 401 response that includes the RFC 9728 `WWW-Authenticate` hint so
/// MCP clients can discover the OAuth resource metadata endpoint.
///
/// `public_url` is the pre-validated base URL read once at startup from
/// `MCP_PUBLIC_URL` (stored in `AppState`).
fn unauthorized(public_url: &str) -> Response {
    let resource_metadata = format!("{public_url}/.well-known/oauth-protected-resource");

    let www_auth = format!(r#"Bearer resource_metadata="{resource_metadata}""#);

    let header_value =
        HeaderValue::from_str(&www_auth).unwrap_or_else(|_| HeaderValue::from_static("Bearer"));

    let mut response = StatusCode::UNAUTHORIZED.into_response();
    response
        .headers_mut()
        .insert(header::WWW_AUTHENTICATE, header_value);
    response
}

// ---------------------------------------------------------------------------
// Token extraction / validation
// ---------------------------------------------------------------------------

/// Extract the raw Bearer token from an `Authorization` header value.
///
/// Returns the token string (the part after `"Bearer "`) or an error whose
/// message contains "missing" if the header is absent or lacks the prefix.
pub fn extract_bearer(header: Option<&str>) -> anyhow::Result<&str> {
    match header {
        None => Err(anyhow!("missing Authorization header")),
        Some(h) => h
            .strip_prefix("Bearer ")
            .ok_or_else(|| anyhow!("missing Bearer prefix in Authorization header")),
    }
}

/// Validate a Clerk JWT and return the full [`ClerkJwt`] claims on success.
///
/// Returning the full claims (rather than just `sub`) lets callers extract
/// `full_name` and `email` from `jwt.other` — mirroring how the backend's
/// GraphQL resolvers populate the user profile on first sign-in.
pub async fn validate(token: &str, jwks: Arc<MemoryCacheJwksProvider>) -> anyhow::Result<ClerkJwt> {
    validate_jwt(token, jwks)
        .await
        .map_err(|e| anyhow!("{}", e))
}

// ---------------------------------------------------------------------------
// Axum middleware
// ---------------------------------------------------------------------------

/// Axum middleware that validates the Clerk Bearer JWT and resolves the
/// internal Tradstry user, threading a [`UserContext`] into the request
/// extensions for downstream tool handlers to consume.
///
/// On authentication failure returns 401 + `WWW-Authenticate` header.
/// On internal errors (DB failure) returns 500.
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    // 1. Extract the Bearer token from the Authorization header.
    let token_str = {
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        match extract_bearer(auth_header) {
            Ok(t) => t.to_owned(),
            Err(e) => {
                tracing::warn!("auth: missing or malformed Authorization header: {e}");
                return unauthorized(&state.public_url);
            }
        }
    };

    // 2. Validate the JWT with Clerk JWKS and obtain the full claims.
    let jwt = match validate(&token_str, state.jwks.clone()).await {
        Ok(jwt) => jwt,
        Err(e) => {
            tracing::warn!("auth: JWT validation failed: {e}");
            return unauthorized(&state.public_url);
        }
    };

    // 3. Extract identity fields from the JWT claims (mirrors accounts.rs).
    let sub = jwt.sub.clone();
    let full_name = jwt
        .other
        .get("full_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let email = jwt
        .other
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // 4. Resolve (or create) the internal Tradstry user.
    let user = match ensure_user(state.db.pool(), &sub, full_name, email).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("ensure_user failed for sub={sub}: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // 5. Thread the resolved identity into the request extensions so rmcp
    //    tool handlers can read it via `ctx.extensions`.
    req.extensions_mut()
        .insert(UserContext { user_id: user.id });

    // 6. Pass the (now annotated) request down the middleware chain.
    next.run(req).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn rejects_missing_bearer() {
        assert!(
            extract_bearer(None)
                .unwrap_err()
                .to_string()
                .contains("missing")
        );
    }

    #[test]
    fn extracts_token() {
        assert_eq!(
            extract_bearer(Some("Bearer abc.def.ghi")).unwrap(),
            "abc.def.ghi"
        );
    }

    #[test]
    fn rejects_no_bearer_prefix() {
        assert!(
            extract_bearer(Some("Token abc.def.ghi"))
                .unwrap_err()
                .to_string()
                .contains("missing")
        );
    }

    #[test]
    fn unauthorized_returns_401_with_www_authenticate_header() {
        let base_url = "https://mcp.tradstry.com";
        let response = unauthorized(base_url);

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let www_auth = response
            .headers()
            .get(header::WWW_AUTHENTICATE)
            .expect("WWW-Authenticate header must be present")
            .to_str()
            .expect("WWW-Authenticate header must be valid UTF-8");

        assert!(
            www_auth.contains("resource_metadata=\""),
            "header should contain resource_metadata= but got: {www_auth}"
        );
        assert!(
            www_auth.contains("/.well-known/oauth-protected-resource"),
            "header should contain the well-known path but got: {www_auth}"
        );
    }
}
