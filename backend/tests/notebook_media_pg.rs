mod pg_support;
use pg_support::{reset_schema, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::db::schema::tables::notebook::images;

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

#[tokio::test]
async fn find_by_hash_returns_none_when_absent() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    migrate(&pool).await;

    let found = images::find_notebook_image_by_hash(&pool, "user-x", "deadbeef")
        .await
        .unwrap();
    assert!(found.is_none());

    let count = images::count_images_with_hash(&pool, "user-x", "deadbeef")
        .await
        .unwrap();
    assert_eq!(count, 0);
}
