use anyhow::{Context, Result};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, Connection, PgPool, Postgres, Transaction};
use std::time::Duration;

use super::schema::migrate;

const DEFAULT_POOL_SIZE: u32 = 12;
const DEFAULT_POOL_MIN: u32 = 1;
const DEFAULT_ACQUIRE_TIMEOUT_SECS: u64 = 10;
const DEFAULT_SLOW_QUERY_MS: u64 = 250;

fn env_u32(name: &str, default: u32) -> Result<u32> {
    match std::env::var(name) {
        Ok(value) => value
            .parse::<u32>()
            .with_context(|| format!("{name} must be a positive integer")),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error).with_context(|| format!("failed to read {name}")),
    }
}

fn env_u64(name: &str, default: u64) -> Result<u64> {
    match std::env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .with_context(|| format!("{name} must be a positive integer")),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error).with_context(|| format!("failed to read {name}")),
    }
}

/// Postgres-backed database client wrapping a shared `sqlx::PgPool`.
#[derive(Clone)]
pub struct Db {
    pool: PgPool,
}

/// User-scoped database access. Holds a pool handle plus the owning user id.
#[derive(Clone)]
pub struct UserDb {
    pool: PgPool,
    user_id: String,
}

impl UserDb {
    // Create new UserDb for a user.
    pub fn new(pool: PgPool, user_id: String) -> Self {
        Self { pool, user_id }
    }

    // Get the connection pool for issuing queries.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // Get the user ID for this database.
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
}

impl Db {
    /// Build the connection pool from `POSTGRES_URL` and run schema migrations.
    pub async fn new() -> Result<Self> {
        let url = std::env::var("POSTGRES_URL").context("POSTGRES_URL not set")?;
        let slow_query_ms = env_u64("DB_SLOW_QUERY_MS", DEFAULT_SLOW_QUERY_MS)?.max(1);
        let opts: PgConnectOptions = url
            .parse::<PgConnectOptions>()
            .context("Failed to parse POSTGRES_URL")?
            .log_statements(log::LevelFilter::Debug)
            .log_slow_statements(log::LevelFilter::Warn, Duration::from_millis(slow_query_ms));

        // Per-environment schema (POSTGRES_DATABASE -> tradstry_<env>). When set,
        // every connection creates the schema if missing and points its
        // search_path at it, so all DDL/queries are isolated to that schema.
        let schema = super::config::env_schema()?;
        let search_path = super::config::search_path()?;
        let max_connections = env_u32("DB_MAX_CONNECTIONS", DEFAULT_POOL_SIZE)?.max(1);
        let min_connections = env_u32("DB_MIN_CONNECTIONS", DEFAULT_POOL_MIN)?.min(max_connections);
        let acquire_timeout =
            env_u64("DB_ACQUIRE_TIMEOUT_SECS", DEFAULT_ACQUIRE_TIMEOUT_SECS)?.max(1);

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .acquire_timeout(Duration::from_secs(acquire_timeout))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(1_800))
            // PostgreSQL detects broken connections on the next operation.
            // Pinging every idle checkout adds a full network round trip to
            // otherwise fast indexed queries.
            .test_before_acquire(false)
            .before_acquire(|conn, meta| {
                Box::pin(async move {
                    if meta.idle_for > Duration::from_secs(60) {
                        conn.ping().await?;
                    }
                    Ok(true)
                })
            })
            .after_connect(move |conn, _meta| {
                let schema = schema.clone();
                let search_path = search_path.clone();
                Box::pin(async move {
                    use sqlx::Executor;
                    if let Some(schema) = &schema {
                        conn.execute(sqlx::AssertSqlSafe(format!(
                            "CREATE SCHEMA IF NOT EXISTS \"{schema}\""
                        )))
                        .await?;
                    }
                    if let Some(sp) = &search_path {
                        conn.execute(sqlx::AssertSqlSafe(format!("SET search_path TO {sp}")))
                            .await?;
                    }
                    conn.execute("SET client_min_messages = WARNING").await?;
                    Ok(())
                })
            })
            .connect_with(opts)
            .await
            .context("Failed to connect to Postgres")?;

        migrate(&pool).await.context("Schema migration failed")?;
        let where_ = super::config::env_schema()?.unwrap_or_else(|| "public".into());
        log::info!(
            "Postgres pool established and schema migrated (schema={where_}, min={min_connections}, max={max_connections})"
        );

        Ok(Self { pool })
    }

    /// Wrap an already-established pool. Skips env-driven setup and migration, so
    /// the caller owns those — used by integration tests holding a migrated pool.
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Borrow the connection pool. sqlx acquires/returns a connection per query.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Begin a transaction for atomic multi-statement writes.
    pub async fn begin(&self) -> Result<Transaction<'static, Postgres>> {
        self.pool
            .begin()
            .await
            .context("Failed to begin transaction")
    }

    /// Get a user-specific database handle.
    pub fn get_user_db(&self, user_id: &str) -> UserDb {
        UserDb::new(self.pool.clone(), user_id.to_string())
    }

    /// Perform a database health check.
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
}
