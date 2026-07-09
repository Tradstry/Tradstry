mod pg_support;
use pg_support::{reset_schema, test_pool};
use sqlx::PgPool;

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

async fn seed_user(pool: &PgPool, id: &str) {
    sqlx::query("INSERT INTO users (id, clerk_uuid) VALUES ($1, $2)")
        .bind(id)
        .bind(format!("clerk_{id}"))
        .execute(pool)
        .await
        .expect("seed user");
}

/// `created_at` is explicit so the backfill's `ORDER BY created_at ASC` is deterministic.
async fn seed_account_at(pool: &PgPool, id: &str, user_id: &str, created_at: &str) {
    sqlx::query(
        "INSERT INTO accounts (id, user_id, name, created_at) VALUES ($1, $2, $3, $4::timestamptz)",
    )
    .bind(id)
    .bind(user_id)
    .bind("acct")
    .bind(created_at)
    .execute(pool)
    .await
    .expect("seed account");
}

#[tokio::test]
async fn migration_adds_account_id_and_drops_user_unique() {
    let pool = test_pool().await;
    reset_schema(&pool).await;
    migrate(&pool).await;

    let is_not_null: bool = sqlx::query_scalar(
        "SELECT attnotnull FROM pg_attribute \
         WHERE attrelid = 'position_calculator_rules'::regclass AND attname = 'account_id'",
    )
    .fetch_one(&pool)
    .await
    .expect("account_id column must exist");
    assert!(is_not_null, "account_id must be NOT NULL");

    let old_unique: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pg_indexes \
         WHERE tablename = 'position_calculator_rules' \
           AND indexname = 'position_calculator_rules_user_id_key'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(old_unique, 0, "the per-user UNIQUE must be dropped");

    let new_unique: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pg_indexes \
         WHERE tablename = 'position_calculator_rules' AND indexname = 'idx_pcr_user_account'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        new_unique, 1,
        "the (user_id, account_id) unique index must exist"
    );
}

#[tokio::test]
async fn one_user_can_hold_a_rule_per_account() {
    let pool = test_pool().await;
    reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_account_at(&pool, "a1", "u1", "2026-01-01T00:00:00Z").await;
    seed_account_at(&pool, "a2", "u1", "2026-02-01T00:00:00Z").await;

    for (id, account_id, risk) in [("r1", "a1", 1.0_f64), ("r2", "a2", 10.0_f64)] {
        sqlx::query(
            "INSERT INTO position_calculator_rules \
             (id, user_id, account_id, account_balance, account_risk, max_stop_loss_pct) \
             VALUES ($1, 'u1', $2, 1000.0, $3, 2.0)",
        )
        .bind(id)
        .bind(account_id)
        .bind(risk)
        .execute(&pool)
        .await
        .expect("two rules for one user across two accounts must coexist");
    }

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM position_calculator_rules WHERE user_id = 'u1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn a_second_rule_for_the_same_account_is_rejected() {
    let pool = test_pool().await;
    reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_account_at(&pool, "a1", "u1", "2026-01-01T00:00:00Z").await;

    sqlx::query(
        "INSERT INTO position_calculator_rules \
         (id, user_id, account_id, account_balance, account_risk, max_stop_loss_pct) \
         VALUES ('r1', 'u1', 'a1', 1000.0, 1.0, 2.0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query(
        "INSERT INTO position_calculator_rules \
         (id, user_id, account_id, account_balance, account_risk, max_stop_loss_pct) \
         VALUES ('r2', 'u1', 'a1', 9999.0, 9.0, 9.0)",
    )
    .execute(&pool)
    .await
    .expect_err("the (user_id, account_id) unique index must reject a duplicate");
    assert!(
        err.to_string().contains("idx_pcr_user_account"),
        "unexpected error: {err}"
    );
}
