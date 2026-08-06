mod pg_support;
use pg_support::{reset_schema, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::db::schema::tables::playbook_table as pb;

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
        .unwrap();
    sqlx::query("INSERT INTO workspaces (id, user_id, name) VALUES ($1, $2, 'Test')")
        .bind(format!("ws-{id}"))
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

fn args(id: &str, name: &str) -> pb::PlaybookWriteArgs {
    pb::PlaybookWriteArgs {
        id: id.into(),
        workspace_id: "ws-u1".into(),
        name: name.into(),
        edge_name: "Momentum".into(),
        entry_rules: "e".into(),
        exit_rules: "x".into(),
        position_sizing_rules: "p".into(),
        additional_rules: None,
    }
}

#[tokio::test]
async fn create_update_delete_flow_and_since() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    seed_user(&pool, "u1").await;

    let mut c = pool.acquire().await.unwrap();
    pb::create_playbook_tx(
        &mut c,
        "u1",
        &args("pb1", "Breakout"),
        "000000000000001:00000:client",
    )
    .await
    .unwrap();

    let deltas = pb::playbooks_since(&pool, "u1", "ws-u1", None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].name, "Breakout");
    assert!(deltas[0].deleted_at.is_none());

    pb::update_playbook_tx(
        &mut c,
        "u1",
        &args("pb1", "Breakout v2"),
        "000000000000002:00000:client",
    )
    .await
    .unwrap();
    let deltas = pb::playbooks_since(&pool, "u1", "ws-u1", None)
        .await
        .unwrap();
    assert_eq!(deltas[0].name, "Breakout v2");

    pb::soft_delete_playbook_tx(&mut c, "u1", "pb1", "000000000000003:00000:client")
        .await
        .unwrap();
    let deltas = pb::playbooks_since(&pool, "u1", "ws-u1", None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1, "tombstone still appears in deltas");
    assert!(deltas[0].deleted_at.is_some());
}

#[tokio::test]
async fn create_playbook_mutation_applies_through_push() {
    use tradstry_backend::graphql::notebook::sync::{NotebookMutation, apply_mutation};

    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    seed_user(&pool, "u2").await;

    let m = NotebookMutation {
        id: 1,
        name: "createPlaybook".into(),
        args: serde_json::json!({
            "id": "pbx",
            "workspaceId": "ws-u2",
            "name": "Pullback",
            "edgeName": "Trend",
            "entryRules": "e",
            "exitRules": "x",
            "positionSizingRules": "p",
            "additionalRules": null,
        })
        .to_string(),
        hlc: "000000000000009:00000:client".into(),
    };
    apply_mutation(&pool, "u2", "clientA", &m).await.unwrap();

    let deltas = pb::playbooks_since(&pool, "u2", "ws-u2", None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].name, "Pullback");
    assert_eq!(deltas[0].hlc, "000000000000009:00000:client");
}
