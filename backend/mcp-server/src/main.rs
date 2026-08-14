//! Tradstry MCP server.
//!
//! Streamable-HTTP rmcp 1.7 server fronted by Axum middleware that validates
//! Clerk Bearer JWTs and threads a `UserContext` into every request.
//!
//! ## Identity threading (resolved in Task 1)
//!
//! A value inserted by Axum middleware via `req.extensions_mut().insert(..)`
//! IS readable inside an rmcp tool handler:
//!
//!   1. Tool handlers may take a `RequestContext<RoleServer>` argument (the
//!      `#[tool_router]` macro injects it).
//!   2. The Streamable-HTTP transport calls `request.into_parts()` and inserts
//!      the full `axum::http::request::Parts` into the MCP request extensions.
//!   3. Inside a handler:
//!      let parts = ctx.extensions.get::<axum::http::request::Parts>();
//!      let user  = parts.and_then(|p| p.extensions.get::<UserContext>());
//!
//! ## Middleware state threading
//!
//! `require_auth` uses `axum::extract::State<Arc<AppState>>`.  To make
//! `from_fn_with_state` work while the same state is also available to tool
//! handlers added later, the router is structured as:
//!
//!   Router::new()
//!     .nest_service("/mcp", mcp_service)
//!     .route_layer(middleware::from_fn_with_state(state.clone(), require_auth))
//!     .with_state(state)

mod app_state;
mod auth;
mod metadata;
mod rate_limit;
mod server;
mod tools;
mod user_context;
mod views;

use std::sync::Arc;

use axum::{Router, middleware};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tradstry_backend::service::ai::vector_database::client::VectorDatabaseClient;
use tradstry_backend::service::auth::create_jwks_provider;
use tradstry_backend::service::db::Db;
use tradstry_backend::service::r2::R2Client;
use tradstry_backend::service::redis::RedisClient;

use app_state::AppState;
use rate_limit::RateLimiter;
use server::TradstryMcp;

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present (mirrors backend bootstrap).
    dotenvy::dotenv().ok();

    // Initialise structured logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // ---------------------------------------------------------------------------
    // Build shared AppState
    // ---------------------------------------------------------------------------

    let clerk_secret = std::env::var("CLERK_SECRET_KEY")
        .expect("CLERK_SECRET_KEY environment variable must be set");

    let public_url =
        std::env::var("MCP_PUBLIC_URL").expect("MCP_PUBLIC_URL environment variable must be set");

    let clerk_issuer = std::env::var("CLERK_ISSUER").expect("CLERK_ISSUER must be set");

    let jwks = Arc::new(create_jwks_provider(&clerk_secret));

    let db = Arc::new(Db::new().await?);

    // Construct the vector search client exactly like the main backend
    // (reads POSTGRES_* and VOYAGE_* env vars). `from_env` is synchronous.
    let vector_db = Arc::new(VectorDatabaseClient::from_env()?);

    // Construct the R2 client (reads R2_ACCOUNT_ID, R2_ACCESS_KEY_ID,
    // R2_SECRET_ACCESS_KEY, R2_BUCKET env vars — same vars as the main backend).
    let r2 = Arc::new(R2Client::from_env()?);

    let redis = Arc::new(RedisClient::from_env().await?);
    let rate_limiter = Arc::new(RateLimiter::new(redis));

    let state = Arc::new(AppState {
        jwks,
        db,
        vector_db,
        r2,
        public_url,
        clerk_issuer,
        rate_limiter,
    });

    // ---------------------------------------------------------------------------
    // Build the rmcp service
    // ---------------------------------------------------------------------------

    let mcp_service = {
        let state = state.clone();
        StreamableHttpService::new(
            move || Ok(TradstryMcp::new(state.clone())),
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default().with_allowed_hosts([
                "mcp.tradstry.com",
                "localhost",
                "127.0.0.1",
                "::1",
            ]),
        )
    };

    // ---------------------------------------------------------------------------
    // Compose the Axum router
    //
    // Two separate routers are merged so that `route_layer` only gates the
    // protected routes.  In Axum 0.8, `route_layer` applies to routes declared
    // on the *same* router before it is called; merging a fresh router keeps
    // public routes completely outside that layer.
    //
    // Protected branch: /mcp — requires a valid Clerk Bearer JWT.
    // Public branch   : OAuth discovery endpoints and /health — no auth.
    // ---------------------------------------------------------------------------

    let protected = Router::new().nest_service("/mcp", mcp_service).route_layer(
        middleware::from_fn_with_state(state.clone(), auth::require_auth),
    );

    let discovery = axum::routing::get(metadata::handler).options(metadata::options_handler);
    let public = Router::new()
        .route(metadata::ROOT_METADATA_PATH, discovery.clone())
        .route(metadata::MCP_METADATA_PATH, discovery)
        .route("/health", axum::routing::get(|| async { "ok" }));

    let app = public.merge(protected).with_state(state);

    let bind_addr = std::env::var("MCP_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:7900".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("mcp-server listening on {bind_addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
