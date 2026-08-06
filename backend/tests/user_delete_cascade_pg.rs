mod pg_support;

use pg_support::{reset_schema, seed_user_workspace, test_pool};

async fn seed_tag(pool: &sqlx::PgPool, user_id: &str, workspace_id: &str) {
    sqlx::query(
        "INSERT INTO tag_categories (id, user_id, workspace_id, name, created_at, updated_at)
         VALUES ($1, $2, $3, $4, now(), now())",
    )
    .bind("cat-1")
    .bind(user_id)
    .bind(workspace_id)
    .bind("Mistakes")
    .execute(pool)
    .await
    .expect("seed category");

    sqlx::query(
        "INSERT INTO tags (id, user_id, workspace_id, category_id, name, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, now(), now())",
    )
    .bind("tag-1")
    .bind(user_id)
    .bind(workspace_id)
    .bind("cat-1")
    .bind("Moved stop")
    .execute(pool)
    .await
    .expect("seed tag");

    sqlx::query(
        "INSERT INTO usage_counters (user_id, metric, period_start, period_end, used)
         VALUES ($1, $2, now(), now(), 1)",
    )
    .bind(user_id)
    .bind("trades")
    .execute(pool)
    .await
    .expect("seed usage counter");
}

async fn count(pool: &sqlx::PgPool, sql: &'static str, user_id: &str) -> i64 {
    sqlx::query_scalar(sql)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("count rows")
}

#[tokio::test]
async fn deleting_a_user_removes_their_tags_and_categories() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .expect("migrate");

    let (user_id, workspace_id) = seed_user_workspace(&pool).await;
    seed_tag(&pool, &user_id, &workspace_id).await;

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&user_id)
        .execute(&pool)
        .await
        .expect("delete user");

    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM tags WHERE user_id = $1",
            &user_id
        )
        .await,
        0,
        "tags survived the user delete"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM tag_categories WHERE user_id = $1",
            &user_id
        )
        .await,
        0,
        "tag_categories survived the user delete"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM usage_counters WHERE user_id = $1",
            &user_id
        )
        .await,
        0,
        "usage_counters survived the user delete"
    );
}
