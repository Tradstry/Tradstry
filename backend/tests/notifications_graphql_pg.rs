mod pg_support;
use pg_support::{reset_schema, seed_user_workspace, test_pool};
use serde_json::json;
use sqlx::PgPool;
use tradstry_backend::service::notifications::{preferences, store, subscriptions};

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

#[tokio::test]
async fn the_feed_is_scoped_to_its_owner() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (owner, _) = seed_user_workspace(&pool).await;
    let (stranger, _) = seed_user_workspace(&pool).await;

    let mut conn = pool.acquire().await.unwrap();
    store::upsert_coalesced(&mut conn, &owner, "FillsLanded", None, &json!({}))
        .await
        .unwrap();

    assert_eq!(store::feed(&pool, &owner, 50, None).await.unwrap().len(), 1);
    assert_eq!(
        store::feed(&pool, &stranger, 50, None).await.unwrap().len(),
        0,
        "a stranger must never see another user's notifications"
    );
}

#[tokio::test]
async fn feed_pagination_walks_backwards_without_repeating() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _) = seed_user_workspace(&pool).await;

    for _ in 0..5 {
        let mut conn = pool.acquire().await.unwrap();
        store::upsert_coalesced(&mut conn, &user_id, "FillsLanded", None, &json!({}))
            .await
            .unwrap();
    }

    let first = store::feed(&pool, &user_id, 2, None).await.unwrap();
    assert_eq!(first.len(), 2);
    let cursor = first.last().unwrap().updated_at;
    let second = store::feed(&pool, &user_id, 2, Some(cursor)).await.unwrap();
    assert_eq!(second.len(), 2);
    for row in &second {
        assert!(
            first.iter().all(|f| f.id != row.id),
            "a page must not repeat rows from the previous page"
        );
    }
}

#[tokio::test]
async fn deleting_a_subscription_is_scoped_to_its_owner() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (owner, _) = seed_user_workspace(&pool).await;
    let (stranger, _) = seed_user_workspace(&pool).await;

    subscriptions::upsert(&pool, &owner, "https://push/1", "k", "a", None)
        .await
        .unwrap();

    assert!(
        !subscriptions::delete_by_endpoint(&pool, &stranger, "https://push/1")
            .await
            .unwrap()
    );
    assert_eq!(
        subscriptions::list_for_user(&pool, &owner)
            .await
            .unwrap()
            .len(),
        1
    );

    assert!(
        subscriptions::delete_by_endpoint(&pool, &owner, "https://push/1")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn preference_list_is_per_user() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (a, _) = seed_user_workspace(&pool).await;
    let (b, _) = seed_user_workspace(&pool).await;

    preferences::set(&pool, &a, "FillsLanded", false)
        .await
        .unwrap();

    let a_rows = preferences::list(&pool, &a).await.unwrap();
    let b_rows = preferences::list(&pool, &b).await.unwrap();
    assert!(
        !a_rows
            .iter()
            .find(|r| r.event_type == "FillsLanded")
            .unwrap()
            .enabled
    );
    assert!(
        b_rows
            .iter()
            .find(|r| r.event_type == "FillsLanded")
            .unwrap()
            .enabled
    );
}
