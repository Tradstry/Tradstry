use anyhow::{Context, Result};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Postgres, Transaction};

use super::schema::migrate;

/// Number of connections in the pool. Constant, not an env knob.
const POOL_SIZE: u32 = 12;

/// Postgres-backed database client wrapping a shared `sqlx::PgPool`.
#[derive(Clone)]
pub struct Db {
    pool: PgPool,
}

/// User-scoped database access. Holds a pool handle plus the owning user id.
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
        let opts: PgConnectOptions = url.parse().context("Failed to parse POSTGRES_URL")?;

        // Per-environment schema (POSTGRES_DATABASE -> tradstry_<env>). When set,
        // every connection creates the schema if missing and points its
        // search_path at it, so all DDL/queries are isolated to that schema.
        let schema = super::config::env_schema()?;
        let search_path = super::config::search_path()?;

        let pool = PgPoolOptions::new()
            .max_connections(POOL_SIZE)
            .min_connections(1)
            .test_before_acquire(true)
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
        log::info!("Postgres pool ({POOL_SIZE}) established and schema migrated (schema={where_})");

        Ok(Self { pool })
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
