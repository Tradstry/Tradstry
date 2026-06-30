use anyhow::{Context, Result};
use sqlx::PgPool;

/// Apply all pending schema migrations.
///
/// Migrations live as versioned SQL files in `backend/migrations/` and are
/// embedded into the binary at compile time by `sqlx::migrate!()`. sqlx records
/// applied migrations in the `_sqlx_migrations` table and runs ONLY the ones not
/// yet applied, in filename order, each exactly once. When the database is
/// up to date this is a single cheap lookup — so calling it on every boot is safe
/// and inexpensive.
///
/// To change the schema, ADD a new numbered file (e.g. `0002_add_x.sql`) with the
/// `ALTER`/`CREATE` statements. Never edit an already-applied migration — sqlx
/// checksums them and will refuse to start on a mismatch.
pub async fn migrate(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("Schema migration failed")?;
    Ok(())
}
