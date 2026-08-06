mod pg_support;
use pg_support::{reset_schema, seed_user_workspace, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::db::schema::tables::trading_principle_table as pt;

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

fn args(id: &str, workspace_id: &str, title: &str, priority: i64) -> pt::PrincipleWriteArgs {
    pt::PrincipleWriteArgs {
        id: id.into(),
        workspace_id: workspace_id.into(),
        playbook_id: None,
        evidence_note_id: None,
        title: title.into(),
        the_rule: "Never average down".into(),
        why: "It turns a small loss into a big one".into(),
        intervention: Some("Walk away for 10 minutes".into()),
        is_active: true,
        priority,
    }
}

#[tokio::test]
async fn create_update_delete_flow_and_since() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let mut c = pool.acquire().await.unwrap();
    pt::create_principle_tx(
        &mut c,
        &user_id,
        &args("pr1", &workspace_id, "30-min rule", 0),
        "000000000000001:00000:client",
    )
    .await
    .unwrap();

    let deltas = pt::principles_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].title, "30-min rule");
    assert_eq!(deltas[0].the_rule, "Never average down");
    assert_eq!(deltas[0].priority, 0);
    assert!(deltas[0].is_active);
    assert_eq!(deltas[0].hlc, "000000000000001:00000:client");
    assert!(deltas[0].deleted_at.is_none());

    // Update: whole-row LWW overwrite with new values.
    pt::update_principle_tx(
        &mut c,
        &user_id,
        &pt::PrincipleWriteArgs {
            title: "Updated rule".into(),
            is_active: false,
            priority: 5,
            ..args("pr1", &workspace_id, "30-min rule", 0)
        },
        "000000000000002:00000:client",
    )
    .await
    .unwrap();
    let deltas = pt::principles_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].title, "Updated rule");
    assert!(!deltas[0].is_active);
    assert_eq!(deltas[0].priority, 5);
    assert_eq!(deltas[0].hlc, "000000000000002:00000:client");

    pt::soft_delete_principle_tx(&mut c, &user_id, "pr1", "000000000000003:00000:client")
        .await
        .unwrap();
    let deltas = pt::principles_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1, "tombstone still appears in deltas");
    assert!(deltas[0].deleted_at.is_some());
    assert_eq!(deltas[0].hlc, "000000000000003:00000:client");
}

#[tokio::test]
async fn reorder_assigns_absolute_priority_and_hlc() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let mut c = pool.acquire().await.unwrap();
    pt::create_principle_tx(
        &mut c,
        &user_id,
        &args("pr1", &workspace_id, "First", 0),
        "000000000000001:00000:client",
    )
    .await
    .unwrap();
    pt::create_principle_tx(
        &mut c,
        &user_id,
        &args("pr2", &workspace_id, "Second", 0),
        "000000000000002:00000:client",
    )
    .await
    .unwrap();

    pt::reorder_principles_tx(
        &mut c,
        &user_id,
        &["pr2".to_string(), "pr1".to_string()],
        "000000000000003:00000:client",
    )
    .await
    .unwrap();

    let mut deltas = pt::principles_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    deltas.sort_by(|a, b| a.id.cmp(&b.id));

    let pr1 = deltas.iter().find(|d| d.id == "pr1").unwrap();
    let pr2 = deltas.iter().find(|d| d.id == "pr2").unwrap();
    // First id gets the highest priority (`top - index`), matching the web
    // convention; both surfaces list DESC so order agrees cross-device.
    assert_eq!(
        pr2.priority, 2,
        "pr2 was first in ordered_ids → highest priority"
    );
    assert_eq!(pr1.priority, 1, "pr1 was second in ordered_ids");
    assert_eq!(pr1.hlc, "000000000000003:00000:client");
    assert_eq!(pr2.hlc, "000000000000003:00000:client");
}

#[tokio::test]
async fn create_principle_mutation_applies_through_push() {
    use tradstry_backend::graphql::notebook::sync::{NotebookMutation, apply_mutation};

    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let m = NotebookMutation {
        id: 1,
        name: "createPrinciple".into(),
        args: serde_json::json!({
            "id": "prx",
            "workspaceId": workspace_id,
            "playbookId": null,
            "evidenceNoteId": null,
            "title": "30-min rule",
            "theRule": "Never average down",
            "why": "It turns a small loss into a big one",
            "intervention": null,
            "isActive": true,
            "priority": 0,
        })
        .to_string(),
        hlc: "000000000000009:00000:client".into(),
    };
    apply_mutation(&pool, &user_id, "clientA", &m)
        .await
        .unwrap();

    let deltas = pt::principles_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].title, "30-min rule");
    assert_eq!(deltas[0].hlc, "000000000000009:00000:client");
}
