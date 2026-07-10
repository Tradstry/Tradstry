use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex, OwnedMutexGuard};
use uuid::Uuid;

/// Shared connection helper for migration integration tests.
/// Requires the local Docker Postgres from docker-compose.test.yml.
///
/// Migrates on first use. Each test binary is its own process, so without this a
/// binary that never migrates passes only when some *other* binary happened to
/// migrate the database first — which is true until the database is recreated.
pub async fn test_pool() -> PgPool {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://tradstry:tradstry@localhost:5435/tradstry_test".to_string()
    });
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("connect to test postgres (is docker-compose.test.yml up?)");

    static MIGRATED: OnceLock<()> = OnceLock::new();
    if MIGRATED.get().is_none() {
        // Serialize against the concurrent tests in this process; sqlx itself takes
        // an advisory lock, so cross-process races are already handled.
        let _guard = schema_lock().lock_owned().await;
        tradstry_backend::service::db::schema::pg::migrate(&pool)
            .await
            .expect("migrate test database");
        let _ = MIGRATED.set(());
    }

    pool
}

fn schema_lock() -> Arc<Mutex<()>> {
    static LOCK: OnceLock<Arc<Mutex<()>>> = OnceLock::new();
    LOCK.get_or_init(|| Arc::new(Mutex::new(()))).clone()
}

/// Drops and recreates the public schema so each test starts clean.
///
/// All tests in this binary share one physical database, and `cargo test` runs
/// `#[tokio::test]`s concurrently by default, so an unguarded reset races with
/// other tests' resets/migrations. Bind the returned guard (`let _guard = ...`)
/// for the lifetime of the test to serialize schema-resetting tests against
/// each other.
#[allow(dead_code)]
pub async fn reset_schema(pool: &PgPool) -> OwnedMutexGuard<()> {
    let guard = schema_lock().lock_owned().await;
    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(pool)
        .await
        .expect("drop schema");
    sqlx::query("CREATE SCHEMA public")
        .execute(pool)
        .await
        .expect("create schema");
    guard
}

/// Inserts a user and an account, returning `(user_id, account_id)`.
/// `notebook_notes` and `notebook_folders` both FK to these, so no notebook row
/// can exist without them.
#[allow(dead_code)]
pub async fn seed_user_account(pool: &PgPool) -> (String, String) {
    let user_id = Uuid::new_v4().to_string();
    let account_id = Uuid::new_v4().to_string();
    let clerk_uuid = Uuid::new_v4().to_string();

    sqlx::query("INSERT INTO users (id, clerk_uuid, email, full_name) VALUES ($1, $2, $3, $4)")
        .bind(&user_id)
        .bind(&clerk_uuid)
        .bind(format!("{user_id}@test.local"))
        .bind("Test User")
        .execute(pool)
        .await
        .expect("seed user");

    sqlx::query("INSERT INTO accounts (id, user_id, name) VALUES ($1, $2, $3)")
        .bind(&account_id)
        .bind(&user_id)
        .bind("Test Account")
        .execute(pool)
        .await
        .expect("seed account");

    (user_id, account_id)
}
