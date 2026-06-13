use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use libsql::{Builder, Connection, Database};
use log::info;

use super::schema::migrate;

/// Periodic background sync cadence for the main backend's embedded replica.
/// The backend is the sole writer (read-your-writes covers its own writes), so
/// this is only insurance against out-of-band changes to the primary.
pub const BACKEND_SYNC_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Periodic background sync cadence for the read-only MCP server's replica,
/// paired with a best-effort on-read sync (see `synced_user_db` in the MCP server).
pub const MCP_SYNC_INTERVAL: Duration = Duration::from_secs(60);

/// Compile-time embedded-replica toggle. Flip to `false` (then rebuild + redeploy)
/// to fall back to remote-only access.
const EMBEDDED_REPLICA: bool = true;

/// Local replica file path inside the container. Both containers use the same
/// container path; their bind mounts map to distinct host directories.
const REPLICA_PATH: &str = "/data/replica.db";

/// Number of libsql connections opened once at startup and handed out
/// round-robin. `Database::connect()` runs libsql's one-time `block_in_place`
/// bootstrap, so we pay it N times at boot instead of per request; N distinct
/// connections also allow that many concurrent local reads (a single connection
/// serializes queries).
const REPLICA_POOL_SIZE: usize = 4;

#[derive(Clone)]
pub struct TursoConfig {
    pub db_url: String,
    pub db_token: String,
    /// Background replica sync cadence (ignored in remote-only mode). Defaults to
    /// `BACKEND_SYNC_INTERVAL`; the MCP binary overrides it with `MCP_SYNC_INTERVAL`.
    pub sync_interval: Duration,
}

impl TursoConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            db_url: std::env::var("TURSO_DB_URL")
                .context("TURSO_DB_URL environment variable not set")?,
            db_token: std::env::var("TURSO_DB_TOKEN")
                .context("TURSO_DB_TOKEN environment variable not set")?,
            sync_interval: BACKEND_SYNC_INTERVAL,
        })
    }
}

pub struct TursoClient {
    db: Database,
    /// Fixed pool of connections opened at startup; handed out round-robin via
    /// `next`. Cloning a pooled `Connection` shares that specific underlying
    /// connection (its inner state is `Arc`-backed).
    pool: Vec<Connection>,
    next: AtomicUsize,
    /// True when running against an embedded replica (mirrors `EMBEDDED_REPLICA`).
    /// Gates `sync()` so remote-only mode is a no-op.
    embedded: bool,
    #[allow(dead_code)]
    config: TursoConfig,
}

/// Scoped database access for a specific user
pub struct UserDb {
    conn: Connection,
    user_id: String,
}

impl UserDb {
    pub fn new(conn: Connection, user_id: String) -> Self {
        Self { conn, user_id }
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }
}

impl TursoClient {
    /// Create a new Turso client and run migrations
    pub async fn new(config: TursoConfig) -> Result<Self> {
        let db = if EMBEDDED_REPLICA {
            // The replica file path is environment-specific infra: the container
            // bind-mounts `/data`, but local dev needs a writable path. Default to
            // REPLICA_PATH; allow TURSO_REPLICA_PATH to override (e.g. ./.turso/replica.db
            // locally). Create the parent dir so libsql can write its db + temp files.
            let replica_path =
                std::env::var("TURSO_REPLICA_PATH").unwrap_or_else(|_| REPLICA_PATH.to_string());
            if let Some(parent) = std::path::Path::new(&replica_path).parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create replica dir {parent:?}"))?;
            }
            let db = Builder::new_remote_replica(
                &replica_path,
                config.db_url.clone(),
                config.db_token.clone(),
            )
            .sync_interval(config.sync_interval)
            .read_your_writes(true)
            .build()
            .await
            .context("Failed to build embedded replica")?;
            // Pull the primary's current state up front so the first reads
            // aren't served from an empty local file.
            db.sync().await.context("Initial replica sync failed")?;
            db
        } else {
            Builder::new_remote(config.db_url.clone(), config.db_token.clone())
                .build()
                .await
                .context("Failed to connect to database")?
        };

        // Open the connection pool once. `foreign_keys` is a per-connection
        // pragma, so set it on each connection here (not per request).
        let mut pool = Vec::with_capacity(REPLICA_POOL_SIZE);
        for _ in 0..REPLICA_POOL_SIZE {
            let conn = db
                .connect()
                .context("Failed to open pooled database connection")?;
            conn.execute("PRAGMA foreign_keys = ON", libsql::params![])
                .await
                .context("Failed to enable foreign keys")?;
            pool.push(conn);
        }

        migrate(&pool[0]).await.context("Schema migration failed")?;

        info!(
            "Database connection pool ({REPLICA_POOL_SIZE}) established and schema migrated (embedded_replica={EMBEDDED_REPLICA})"
        );

        Ok(Self {
            db,
            pool,
            next: AtomicUsize::new(0),
            embedded: EMBEDDED_REPLICA,
            config,
        })
    }

    /// Pull the latest frames from the primary into the local replica. No-op in
    /// remote-only mode so call sites stay mode-agnostic.
    pub async fn sync(&self) -> Result<()> {
        if self.embedded {
            self.db.sync().await.context("Replica sync failed")?;
        }
        Ok(())
    }

    /// Hand out a pooled connection round-robin. Cheap: an `Arc` clone, no
    /// per-request `connect()` (so no per-request `block_in_place` bootstrap).
    pub fn get_connection(&self) -> Result<Connection> {
        let idx = self.next.fetch_add(1, Ordering::Relaxed) % self.pool.len();
        Ok(self.pool[idx].clone())
    }

    pub async fn get_user_db(&self, user_id: &str) -> Result<UserDb> {
        // `foreign_keys` was set on every pooled connection at startup, so there
        // is no per-call pragma round-trip here.
        let conn = self.get_connection()?;
        Ok(UserDb::new(conn, user_id.to_string()))
    }

    pub async fn health_check(&self) -> Result<()> {
        let conn = self.get_connection()?;
        // Use `query` (not `execute`) for a row-returning SELECT: the local SQLite
        // replica strictly rejects a SELECT passed to `execute` ("Execute returned
        // rows"), whereas the remote hrana path tolerated it.
        conn.query("SELECT 1", libsql::params![]).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_defaults_sync_interval_to_backend() {
        // NOTE: mutates process env; these vars are already required by the app.
        unsafe {
            std::env::set_var("TURSO_DB_URL", "libsql://test-db");
            std::env::set_var("TURSO_DB_TOKEN", "test-token");
        }
        let cfg = TursoConfig::from_env().expect("from_env should succeed");
        assert_eq!(cfg.sync_interval, BACKEND_SYNC_INTERVAL);
        assert!(MCP_SYNC_INTERVAL < BACKEND_SYNC_INTERVAL);
    }
}
