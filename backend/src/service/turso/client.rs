use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use libsql::{Builder, Connection, Database};
use log::info;

use super::schema::migrate;

// Interval for syncing the main backend's embedded replica.
pub const BACKEND_SYNC_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

// Interval for syncing the MCP server's read-only replica.
pub const MCP_SYNC_INTERVAL: Duration = Duration::from_secs(60);

// Toggle for using embedded replica or remote-only access.
const EMBEDDED_REPLICA: bool = true;

// Default path for the embedded replica in the container.
const REPLICA_PATH: &str = "/data/replica.db";

// Number of database connections in the pool.
const REPLICA_POOL_SIZE: usize = 12;

#[derive(Clone)]
pub struct TursoConfig {
    pub db_url: String,
    pub db_token: String,
    // Interval for background replica sync.
    pub sync_interval: Duration,
}

impl TursoConfig {
    // Load Turso database configuration from environment variables.
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

// Turso client with connection pool and configuration.
pub struct TursoClient {
    db: Database,
    pool: Vec<Connection>,
    next: AtomicUsize,
    embedded: bool,
    #[allow(dead_code)]
    config: TursoConfig,
}

// User-scoped database access.
pub struct UserDb {
    conn: Connection,
    user_id: String,
}

impl UserDb {
    // Create new UserDb for a user.
    pub fn new(conn: Connection, user_id: String) -> Self {
        Self { conn, user_id }
    }

    // Get the database connection.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    // Get the user ID for this database.
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
}

impl TursoClient {
    // Create new Turso client and run database migrations.
    pub async fn new(config: TursoConfig) -> Result<Self> {
        let (db, pool) = if EMBEDDED_REPLICA {
            // Initialize the embedded replica, allowing path override and self-healing from corruption.
            let replica_path =
                std::env::var("TURSO_REPLICA_PATH").unwrap_or_else(|_| REPLICA_PATH.to_string());
            if let Some(parent) = std::path::Path::new(&replica_path).parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create replica dir {parent:?}"))?;
            }

            match build_embedded_replica(&replica_path, &config).await {
                Ok(pair) => pair,
                Err(e) if is_corruption_error(&e) => {
                    log::error!(
                        "Embedded replica at {replica_path} is corrupt ({e:#}); wiping local files and rebuilding from primary"
                    );
                    wipe_replica_files(&replica_path).context("Failed to wipe corrupt replica")?;
                    build_embedded_replica(&replica_path, &config)
                        .await
                        .context("Rebuild after wiping corrupt replica failed")?
                }
                Err(e) => return Err(e),
            }
        } else {
            // Connect to remote database directly.
            let db = Builder::new_remote(config.db_url.clone(), config.db_token.clone())
                .build()
                .await
                .context("Failed to connect to database")?;
            let pool = open_pool(&db).await?;
            (db, pool)
        };

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

    // Sync the embedded replica with the primary database (no-op for remote-only).
    pub async fn sync(&self) -> Result<()> {
        if self.embedded {
            self.db.sync().await.context("Replica sync failed")?;
        }
        Ok(())
    }

    // Get a pooled connection in a round-robin fashion.
    pub fn get_connection(&self) -> Result<Connection> {
        let idx = self.next.fetch_add(1, Ordering::Relaxed) % self.pool.len();
        Ok(self.pool[idx].clone())
    }

    // Open a dedicated connection outside of the main pool, enabling foreign keys.
    pub async fn dedicated_connection(&self) -> Result<Connection> {
        let conn = self
            .db
            .connect()
            .context("Failed to open dedicated connection")?;
        conn.execute("PRAGMA foreign_keys = ON", libsql::params![])
            .await
            .context("Failed to enable foreign keys on dedicated connection")?;
        Ok(conn)
    }

    // Get a user-specific database object.
    pub async fn get_user_db(&self, user_id: &str) -> Result<UserDb> {
        let conn = self.get_connection()?;
        Ok(UserDb::new(conn, user_id.to_string()))
    }

    // Perform a database health check.
    pub async fn health_check(&self) -> Result<()> {
        let conn = self.get_connection()?;
        conn.query("SELECT 1", libsql::params![]).await?;
        Ok(())
    }
}

// Build and initialize the embedded replica and its connection pool.
async fn build_embedded_replica(
    replica_path: &str,
    config: &TursoConfig,
) -> Result<(Database, Vec<Connection>)> {
    let db =
        Builder::new_remote_replica(replica_path, config.db_url.clone(), config.db_token.clone())
            .sync_interval(config.sync_interval)
            .read_your_writes(true)
            .build()
            .await
            .context("Failed to build embedded replica")?;
    db.sync().await.context("Initial replica sync failed")?;
    let pool = open_pool(&db).await?;
    Ok((db, pool))
}

// Open a new pool of connections and enable foreign keys on each.
async fn open_pool(db: &Database) -> Result<Vec<Connection>> {
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
    Ok(pool)
}

// Detect if an error is due to replica corruption.
fn is_corruption_error(e: &anyhow::Error) -> bool {
    let msg = format!("{e:#}").to_lowercase();
    msg.contains("not a database")
        || msg.contains("malformed")
        || msg.contains("disk image")
        || msg.contains("file is encrypted")
}

// Remove the replica database file and sidecar files for a fresh rebuild.
fn wipe_replica_files(replica_path: &str) -> Result<()> {
    let path = std::path::Path::new(replica_path);
    let Some(base) = path.file_name().and_then(|n| n.to_str()) else {
        return Ok(());
    };
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    for entry in std::fs::read_dir(&parent)
        .with_context(|| format!("Failed to read replica dir {parent:?}"))?
    {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str()
            && name.starts_with(base)
        {
            let file = entry.path();
            std::fs::remove_file(&file)
                .with_context(|| format!("Failed to remove replica file {file:?}"))?;
            log::warn!("Removed corrupt replica file {file:?}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_defaults_sync_interval_to_backend() {
        // Ensures environment-based config falls back to the backend interval.
        unsafe {
            std::env::set_var("TURSO_DB_URL", "libsql://test-db");
            std::env::set_var("TURSO_DB_TOKEN", "test-token");
        }
        let cfg = TursoConfig::from_env().expect("from_env should succeed");
        assert_eq!(cfg.sync_interval, BACKEND_SYNC_INTERVAL);
        assert!(MCP_SYNC_INTERVAL < BACKEND_SYNC_INTERVAL);
    }
}
