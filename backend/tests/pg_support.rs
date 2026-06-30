use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Shared connection helper for migration integration tests.
/// Requires the local Docker Postgres from docker-compose.test.yml.
pub async fn test_pool() -> PgPool {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://tradstry:tradstry@localhost:5435/tradstry_test".to_string()
    });
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("connect to test postgres (is docker-compose.test.yml up?)")
}

/// Drops and recreates the public schema so each test starts clean.
#[allow(dead_code)]
pub async fn reset_schema(pool: &PgPool) {
    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(pool)
        .await
        .expect("drop schema");
    sqlx::query("CREATE SCHEMA public")
        .execute(pool)
        .await
        .expect("create schema");
}
