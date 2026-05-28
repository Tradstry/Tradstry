use std::sync::Arc;

use clerk_rs::validators::jwks::MemoryCacheJwksProvider;
use tradstry_backend::service::ai::vector_database::client::VectorDatabaseClient;
use tradstry_backend::service::r2::R2Client;
use tradstry_backend::service::turso::TursoClient;

/// Shared application state for the MCP server.
///
/// Constructed once at startup and cloned (cheaply via Arc) into middleware
/// and, later, tool handlers.
pub struct AppState {
    /// Cached JWKS provider used to validate Clerk JWTs.
    pub jwks: Arc<MemoryCacheJwksProvider>,
    /// Turso database client.  Call `turso.get_connection()` to obtain a
    /// per-request `libsql::Connection`.
    pub turso: Arc<TursoClient>,
    /// Hybrid (dense + sparse) vector search client backing semantic search.
    /// Constructed once at startup via `VectorDatabaseClient::from_env()`,
    /// mirroring the main backend.
    pub vector_db: Arc<VectorDatabaseClient>,
    /// Cloudflare R2 client for fetching raw media bytes on behalf of tools.
    /// Constructed once at startup via `R2Client::from_env()`.
    pub r2: Arc<R2Client>,
    /// Base URL of this MCP server (e.g. `https://mcp.tradstry.com`).
    /// Read once from `MCP_PUBLIC_URL` at startup and used to build the
    /// `WWW-Authenticate` resource metadata URL on 401 responses.
    pub public_url: String,
    /// Clerk issuer URL (e.g. `https://clerk.tradstry.com`).
    /// Read once from `CLERK_ISSUER` at startup and advertised in the
    /// OAuth Protected Resource Metadata document.
    pub clerk_issuer: String,
}
