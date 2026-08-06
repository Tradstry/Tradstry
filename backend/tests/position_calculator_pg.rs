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
        "INSERT INTO workspaces (id, user_id, name, created_at) VALUES ($1, $2, $3, $4::timestamptz)",
    )
    .bind(id)
    .bind(user_id)
    .bind("acct")
    .bind(created_at)
    .execute(pool)
    .await
    .expect("seed workspace");
}

#[tokio::test]
async fn migration_adds_account_id_and_drops_user_unique() {
    let pool = test_pool().await;
    let _schema_guard = reset_schema(&pool).await;
    migrate(&pool).await;

    let is_not_null: bool = sqlx::query_scalar(
        "SELECT attnotnull FROM pg_attribute \
         WHERE attrelid = 'position_calculator_rules'::regclass AND attname = 'workspace_id'",
    )
    .fetch_one(&pool)
    .await
    .expect("workspace_id column must exist");
    assert!(is_not_null, "workspace_id must be NOT NULL");

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
        "the (user_id, workspace_id) unique index must exist"
    );
}

#[tokio::test]
async fn one_user_can_hold_a_rule_per_account() {
    let pool = test_pool().await;
    let _schema_guard = reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_account_at(&pool, "a1", "u1", "2026-01-01T00:00:00Z").await;
    seed_account_at(&pool, "a2", "u1", "2026-02-01T00:00:00Z").await;

    for (id, workspace_id, risk) in [("r1", "a1", 1.0_f64), ("r2", "a2", 10.0_f64)] {
        sqlx::query(
            "INSERT INTO position_calculator_rules \
             (id, user_id, workspace_id, account_balance, account_risk, max_stop_loss_pct) \
             VALUES ($1, 'u1', $2, 1000.0, $3, 2.0)",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(risk)
        .execute(&pool)
        .await
        .expect("two rules for one user across two workspaces must coexist");
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
    let _schema_guard = reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_account_at(&pool, "a1", "u1", "2026-01-01T00:00:00Z").await;

    sqlx::query(
        "INSERT INTO position_calculator_rules \
         (id, user_id, workspace_id, account_balance, account_risk, max_stop_loss_pct) \
         VALUES ('r1', 'u1', 'a1', 1000.0, 1.0, 2.0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query(
        "INSERT INTO position_calculator_rules \
         (id, user_id, workspace_id, account_balance, account_risk, max_stop_loss_pct) \
         VALUES ('r2', 'u1', 'a1', 9999.0, 9.0, 9.0)",
    )
    .execute(&pool)
    .await
    .expect_err("the (user_id, workspace_id) unique index must reject a duplicate");
    assert!(
        err.to_string().contains("idx_pcr_user_account"),
        "unexpected error: {err}"
    );
}

use tradstry_backend::service::db::schema::tables::position_calculator_rule_table as pcr;

fn upsert_input(workspace_id: &str, risk: f64) -> pcr::UpsertPositionCalculatorRuleInput {
    pcr::UpsertPositionCalculatorRuleInput {
        workspace_id: workspace_id.to_string(),
        account_balance: 10_000.0,
        account_risk: risk,
        max_stop_loss_pct: 2.0,
    }
}

#[tokio::test]
async fn upsert_rejects_an_account_owned_by_another_user() {
    let pool = test_pool().await;
    let _schema_guard = reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_user(&pool, "u2").await;
    seed_account_at(&pool, "a2", "u2", "2026-01-01T00:00:00Z").await;

    // u1 tries to write a rule against u2's workspace. The FK would happily allow it.
    let err = pcr::upsert_rule(&pool, "u1", upsert_input("a2", 5.0))
        .await
        .expect_err("must not write a rule against another user's workspace");
    assert!(
        err.to_string().contains("not found"),
        "unexpected error: {err}"
    );

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM position_calculator_rules")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "nothing may be written");
}

#[tokio::test]
async fn upsert_updates_only_the_targeted_account_rule() {
    let pool = test_pool().await;
    let _schema_guard = reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_account_at(&pool, "a1", "u1", "2026-01-01T00:00:00Z").await;
    seed_account_at(&pool, "a2", "u1", "2026-02-01T00:00:00Z").await;

    pcr::upsert_rule(&pool, "u1", upsert_input("a1", 1.0))
        .await
        .unwrap();
    pcr::upsert_rule(&pool, "u1", upsert_input("a2", 10.0))
        .await
        .unwrap();

    // Re-upsert a1. a2 must be untouched.
    pcr::upsert_rule(&pool, "u1", upsert_input("a1", 3.0))
        .await
        .unwrap();

    let r1 = pcr::get_rule(&pool, "u1", "a1").await.unwrap().unwrap();
    let r2 = pcr::get_rule(&pool, "u1", "a2").await.unwrap().unwrap();
    assert_eq!(r1.account_risk, 3.0);
    assert_eq!(
        r2.account_risk, 10.0,
        "the other workspace's rule must not change"
    );
    assert_eq!(r1.workspace_id, "a1");
    assert_eq!(r2.workspace_id, "a2");
}

#[tokio::test]
async fn get_rule_is_scoped_to_the_account() {
    let pool = test_pool().await;
    let _schema_guard = reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_account_at(&pool, "a1", "u1", "2026-01-01T00:00:00Z").await;
    seed_account_at(&pool, "a2", "u1", "2026-02-01T00:00:00Z").await;

    pcr::upsert_rule(&pool, "u1", upsert_input("a1", 1.0))
        .await
        .unwrap();

    assert!(pcr::get_rule(&pool, "u1", "a1").await.unwrap().is_some());
    assert!(
        pcr::get_rule(&pool, "u1", "a2").await.unwrap().is_none(),
        "an workspace with no rule must return None, not the other workspace's rule"
    );
}
