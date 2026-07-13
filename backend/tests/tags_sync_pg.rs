mod pg_support;
use pg_support::{reset_schema, seed_user_account, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::db::schema::tables::tags_table as t;

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

/// Inserts a minimal journal_entries row directly (bypassing business-rule
/// validation, which is irrelevant here) so `trade_tags` has a valid FK target
/// for the merge test below.
async fn seed_journal_entry(pool: &PgPool, id: &str, user_id: &str, account_id: &str) {
    sqlx::query(
        "INSERT INTO journal_entries (id, user_id, account_id, open_date, close_date, \
         entry_price, exit_price, position_size, symbol, symbol_name, status, total_pl, \
         net_roi, duration, stop_loss, risk_reward, trade_type, mistakes, entry_tactics, edges_spotted) \
         VALUES ($1, $2, $3, now(), now(), 1, 2, 1, 'AAPL', 'Apple', 'profit', 1, 1, 1, 0, 1, 'long', '', '', '')",
    )
    .bind(id)
    .bind(user_id)
    .bind(account_id)
    .execute(pool)
    .await
    .expect("seed journal entry");
}

#[tokio::test]
async fn category_create_rename_delete_flow_and_since() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _account_id) = seed_user_account(&pool).await;

    let mut c = pool.acquire().await.unwrap();
    t::create_category_tx(
        &mut c,
        &user_id,
        "cat1",
        "Setups",
        Some("#fff"),
        0,
        "000000000000001:00000:client",
    )
    .await
    .unwrap();

    let deltas = t::categories_since(&pool, &user_id, None).await.unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].name, "Setups");
    assert!(deltas[0].role.is_none());
    assert!(deltas[0].deleted_at.is_none());

    t::rename_category_tx(
        &mut c,
        &user_id,
        "cat1",
        "Setups v2",
        "000000000000002:00000:client",
    )
    .await
    .unwrap();
    let deltas = t::categories_since(&pool, &user_id, None).await.unwrap();
    assert_eq!(deltas[0].name, "Setups v2");

    t::set_category_color_tx(
        &mut c,
        &user_id,
        "cat1",
        Some("#000"),
        "000000000000003:00000:client",
    )
    .await
    .unwrap();
    let deltas = t::categories_since(&pool, &user_id, None).await.unwrap();
    assert_eq!(deltas[0].color.as_deref(), Some("#000"));

    t::soft_delete_category_tx(&mut c, &user_id, "cat1", "000000000000004:00000:client")
        .await
        .unwrap();
    let deltas = t::categories_since(&pool, &user_id, None).await.unwrap();
    assert_eq!(deltas.len(), 1, "tombstone still appears in deltas");
    assert!(deltas[0].deleted_at.is_some());
}

#[tokio::test]
async fn reorder_categories_sets_absolute_sort_order_and_hlc() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _account_id) = seed_user_account(&pool).await;

    let mut c = pool.acquire().await.unwrap();
    t::create_category_tx(
        &mut c,
        &user_id,
        "cat1",
        "Setups",
        None,
        0,
        "000000000000001:00000:client",
    )
    .await
    .unwrap();
    t::create_category_tx(
        &mut c,
        &user_id,
        "cat2",
        "Mistakes2",
        None,
        1,
        "000000000000002:00000:client",
    )
    .await
    .unwrap();

    let pairs = vec![("cat1".to_string(), 5i64), ("cat2".to_string(), 3i64)];
    t::reorder_categories_tx(&mut c, &user_id, &pairs, "000000000000003:00000:client")
        .await
        .unwrap();

    let deltas = t::categories_since(&pool, &user_id, None).await.unwrap();
    let cat1 = deltas.iter().find(|d| d.id == "cat1").unwrap();
    let cat2 = deltas.iter().find(|d| d.id == "cat2").unwrap();
    assert_eq!(cat1.sort_order, 5);
    assert_eq!(cat2.sort_order, 3);
    assert_eq!(cat1.hlc, "000000000000003:00000:client");
}

#[tokio::test]
async fn tag_create_rename_delete_flow_and_since() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _account_id) = seed_user_account(&pool).await;

    let mut c = pool.acquire().await.unwrap();
    t::create_category_tx(
        &mut c,
        &user_id,
        "cat1",
        "Setups",
        None,
        0,
        "000000000000001:00000:client",
    )
    .await
    .unwrap();
    t::create_tag_tx(
        &mut c,
        &user_id,
        "tag1",
        "cat1",
        "Breakout",
        Some("#000"),
        "000000000000002:00000:client",
    )
    .await
    .unwrap();

    let deltas = t::tags_since(&pool, &user_id, None).await.unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].name, "Breakout");
    assert_eq!(deltas[0].category_id, "cat1");
    assert!(deltas[0].deleted_at.is_none());

    t::rename_tag_tx(
        &mut c,
        &user_id,
        "tag1",
        "Breakout v2",
        "000000000000003:00000:client",
    )
    .await
    .unwrap();
    let deltas = t::tags_since(&pool, &user_id, None).await.unwrap();
    assert_eq!(deltas[0].name, "Breakout v2");

    t::set_tag_color_tx(
        &mut c,
        &user_id,
        "tag1",
        Some("#fff"),
        "000000000000004:00000:client",
    )
    .await
    .unwrap();
    let deltas = t::tags_since(&pool, &user_id, None).await.unwrap();
    assert_eq!(deltas[0].color.as_deref(), Some("#fff"));

    t::soft_delete_tag_tx(&mut c, &user_id, "tag1", "000000000000005:00000:client")
        .await
        .unwrap();
    let deltas = t::tags_since(&pool, &user_id, None).await.unwrap();
    assert_eq!(deltas.len(), 1, "tombstone still appears in deltas");
    assert!(deltas[0].deleted_at.is_some());
}

#[tokio::test]
async fn merge_tags_tombstones_from_and_repoints_trade_tags() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    let mut c = pool.acquire().await.unwrap();
    t::create_category_tx(
        &mut c,
        &user_id,
        "cat1",
        "Setups",
        None,
        0,
        "000000000000001:00000:client",
    )
    .await
    .unwrap();
    t::create_tag_tx(
        &mut c,
        &user_id,
        "from_tag",
        "cat1",
        "Old",
        None,
        "000000000000002:00000:client",
    )
    .await
    .unwrap();
    t::create_tag_tx(
        &mut c,
        &user_id,
        "into_tag",
        "cat1",
        "New",
        None,
        "000000000000003:00000:client",
    )
    .await
    .unwrap();

    seed_journal_entry(&pool, "je1", &user_id, &account_id).await;
    sqlx::query("INSERT INTO trade_tags (journal_entry_id, tag_id) VALUES ('je1', 'from_tag')")
        .execute(&pool)
        .await
        .unwrap();

    t::merge_tags_tx(
        &mut c,
        &user_id,
        "from_tag",
        "into_tag",
        "000000000000004:00000:client",
    )
    .await
    .unwrap();

    let deltas = t::tags_since(&pool, &user_id, None).await.unwrap();
    let from_delta = deltas.iter().find(|d| d.id == "from_tag").unwrap();
    assert!(
        from_delta.deleted_at.is_some(),
        "from tag must be tombstoned, not hard-deleted"
    );
    let into_delta = deltas.iter().find(|d| d.id == "into_tag").unwrap();
    assert!(into_delta.deleted_at.is_none());

    let linked: Vec<(String, String)> =
        sqlx::query_as("SELECT journal_entry_id, tag_id FROM trade_tags")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        linked,
        vec![("je1".to_string(), "into_tag".to_string())],
        "trade_tags must be repointed to into_id, with the old from_id link removed"
    );
}

#[tokio::test]
async fn create_tag_category_and_tag_mutations_apply_through_push() {
    use tradstry_backend::graphql::notebook::sync::{NotebookMutation, apply_mutation};

    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _account_id) = seed_user_account(&pool).await;

    let m1 = NotebookMutation {
        id: 1,
        name: "createTagCategory".into(),
        args: serde_json::json!({
            "id": "catx",
            "name": "Tactics2",
            "color": "#abc",
            "sortOrder": 5,
        })
        .to_string(),
        hlc: "000000000000001:00000:client".into(),
    };
    apply_mutation(&pool, &user_id, "clientA", &m1)
        .await
        .unwrap();

    let m2 = NotebookMutation {
        id: 2,
        name: "createTag".into(),
        args: serde_json::json!({
            "id": "tagx",
            "categoryId": "catx",
            "name": "Gapper",
            "color": null,
        })
        .to_string(),
        hlc: "000000000000002:00000:client".into(),
    };
    apply_mutation(&pool, &user_id, "clientA", &m2)
        .await
        .unwrap();

    let categories = t::categories_since(&pool, &user_id, None).await.unwrap();
    assert_eq!(categories.len(), 1);
    assert_eq!(categories[0].name, "Tactics2");
    assert_eq!(categories[0].sort_order, 5);
    assert_eq!(categories[0].hlc, "000000000000001:00000:client");

    let tags = t::tags_since(&pool, &user_id, None).await.unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name, "Gapper");
    assert_eq!(tags[0].category_id, "catx");
    assert_eq!(tags[0].hlc, "000000000000002:00000:client");
}
