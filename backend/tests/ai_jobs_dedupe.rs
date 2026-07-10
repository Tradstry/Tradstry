use tradstry_backend::service::ai::db::dedupe_candidate;

mod pg_support;
use pg_support::{seed_user_account, test_pool};
use uuid::Uuid;

async fn insert_job(pool: &sqlx::PgPool, user_id: &str, account_id: &str, key: &str, status: &str) {
    sqlx::query(
        "INSERT INTO ai_jobs (id, user_id, account_id, job_type, payload_json, dedupe_key, status)
         VALUES ($1, $2, $3, 'reindex_account_sources', '{}', $4, $5)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(account_id)
    .bind(key)
    .bind(status)
    .execute(pool)
    .await
    .expect("insert ai_job");
}

/// A queued job already covers the work, so a second enqueue must collapse into it.
#[tokio::test]
async fn queued_job_dedupes() {
    let pool = test_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let key = format!("reindex:{user_id}:{account_id}");

    insert_job(&pool, &user_id, &account_id, &key, "queued").await;

    let found = dedupe_candidate(&pool, &key).await.unwrap();
    assert!(found.is_some(), "a queued job must dedupe a new enqueue");
}

/// A RUNNING job has already read its input. An edit that lands after it started
/// is not covered by it, so the enqueue must NOT collapse into it — otherwise the
/// edit is never indexed.
#[tokio::test]
async fn running_job_does_not_dedupe() {
    let pool = test_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let key = format!("reindex:{user_id}:{account_id}");

    insert_job(&pool, &user_id, &account_id, &key, "running").await;

    let found = dedupe_candidate(&pool, &key).await.unwrap();
    assert!(
        found.is_none(),
        "a running job must not swallow an edit that landed after it started",
    );
}

/// At most one queued job per key, even alongside a running one.
#[tokio::test]
async fn running_plus_queued_still_dedupes() {
    let pool = test_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let key = format!("reindex:{user_id}:{account_id}");

    insert_job(&pool, &user_id, &account_id, &key, "running").await;
    insert_job(&pool, &user_id, &account_id, &key, "queued").await;

    let found = dedupe_candidate(&pool, &key).await.unwrap();
    assert!(
        found.is_some(),
        "the queued job already covers the edit; do not stack a third",
    );
}

/// Terminal jobs never dedupe.
#[tokio::test]
async fn completed_job_does_not_dedupe() {
    let pool = test_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let key = format!("reindex:{user_id}:{account_id}");

    insert_job(&pool, &user_id, &account_id, &key, "completed").await;

    let found = dedupe_candidate(&pool, &key).await.unwrap();
    assert!(found.is_none(), "a completed job must not dedupe");
}
