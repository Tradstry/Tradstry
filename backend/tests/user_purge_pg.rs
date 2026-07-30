mod pg_support;

use pg_support::{reset_schema, test_pool};
use tradstry_backend::service::users::purge::{collect_r2_keys, delete_user_by_clerk_uuid};
use uuid::Uuid;

/// Returns `(user_id, clerk_uuid, r2_key)`.
async fn seed_user_with_image(pool: &sqlx::PgPool) -> (String, String, String) {
    let user_id = Uuid::new_v4().to_string();
    let clerk_uuid = Uuid::new_v4().to_string();
    let account_id = Uuid::new_v4().to_string();
    let note_id = Uuid::new_v4().to_string();
    let image_id = Uuid::new_v4().to_string();
    let key = format!("notebook/{user_id}/media/deadbeef");

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

    sqlx::query(
        "INSERT INTO notebook_notes (id, user_id, account_id, title, document_json)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&note_id)
    .bind(&user_id)
    .bind(&account_id)
    .bind("A note")
    .bind("{}")
    .execute(pool)
    .await
    .expect("seed note");

    sqlx::query(
        "INSERT INTO notebook_images
         (id, note_id, user_id, account_id, cloudinary_asset_id, cloudinary_public_id,
          secure_url, width, height)
         VALUES ($1, $2, $3, $4, $5, $6, '', 10, 10)",
    )
    .bind(&image_id)
    .bind(&note_id)
    .bind(&user_id)
    .bind(&account_id)
    .bind(&image_id)
    .bind(&key)
    .execute(pool)
    .await
    .expect("seed image");

    (user_id, clerk_uuid, key)
}

async fn migrated_pool() -> sqlx::PgPool {
    let pool = test_pool().await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .expect("migrate");
    pool
}

#[tokio::test]
async fn collects_every_r2_key_for_the_user() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    let pool = migrated_pool().await;

    let (user_id, _clerk_uuid, key) = seed_user_with_image(&pool).await;

    let keys = collect_r2_keys(&pool, &user_id)
        .await
        .expect("collect keys");

    assert_eq!(keys, vec![key]);
}

#[tokio::test]
async fn deletes_the_user_and_returns_the_internal_id() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    let pool = migrated_pool().await;

    let (user_id, clerk_uuid, _key) = seed_user_with_image(&pool).await;

    let deleted = delete_user_by_clerk_uuid(&pool, &clerk_uuid)
        .await
        .expect("delete user");

    assert_eq!(deleted.as_deref(), Some(user_id.as_str()));

    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notebook_images WHERE user_id = $1")
            .bind(&user_id)
            .fetch_one(&pool)
            .await
            .expect("count images");
    assert_eq!(remaining, 0, "notebook_images should cascade away");
}

#[tokio::test]
async fn returns_none_for_an_unknown_clerk_uuid() {
    let pool = test_pool().await;
    let _guard = reset_schema(&pool).await;
    let pool = migrated_pool().await;

    let deleted = delete_user_by_clerk_uuid(&pool, "user_does_not_exist")
        .await
        .expect("delete user");

    assert!(deleted.is_none());
}
