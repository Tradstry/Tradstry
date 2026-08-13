mod pg_support;
use pg_support::{reset_schema, seed_user_workspace, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::db::schema::tables::position_calculator_history_table as h;
use tradstry_backend::service::db::schema::tables::position_calculator_plans_table as p;
use tradstry_backend::service::db::schema::tables::position_calculator_rule_table as r;

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

#[tokio::test]
async fn rule_upsert_flow_and_since() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let mut c = pool.acquire().await.unwrap();
    r::upsert_rule_tx(
        &mut c,
        &user_id,
        &r::RuleWriteArgs {
            id: "rule1".into(),
            workspace_id: workspace_id.clone(),
            account_balance: 10000.0,
            account_risk: 1.0,
            max_stop_loss_pct: 5.0,
        },
        "000000000000001:00000:client",
    )
    .await
    .unwrap();

    let deltas = r::rules_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].workspace_id, workspace_id);
    assert_eq!(deltas[0].account_balance, 10000.0);
    assert!(deltas[0].deleted_at.is_none());
    assert_eq!(deltas[0].hlc, "000000000000001:00000:client");

    // Re-upsert (same account) overwrites in place, keyed by (user_id, workspace_id).
    r::upsert_rule_tx(
        &mut c,
        &user_id,
        &r::RuleWriteArgs {
            id: "rule1-again".into(),
            workspace_id: workspace_id.clone(),
            account_balance: 20000.0,
            account_risk: 2.0,
            max_stop_loss_pct: 6.0,
        },
        "000000000000002:00000:client",
    )
    .await
    .unwrap();

    let deltas = r::rules_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1, "upsert must not create a second row");
    assert_eq!(deltas[0].account_balance, 20000.0);
    assert_eq!(deltas[0].account_risk, 2.0);
    assert_eq!(deltas[0].max_stop_loss_pct, 6.0);
    assert_eq!(deltas[0].hlc, "000000000000002:00000:client");
}

#[tokio::test]
async fn plan_create_update_delete_flow_and_since() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let mut c = pool.acquire().await.unwrap();
    p::create_plan_tx(
        &mut c,
        &user_id,
        &p::CreatePlanWriteArgs {
            workspace_id: workspace_id.clone(),
            id: "plan1".into(),
            symbol: "AAPL".into(),
            position_type: "long".into(),
            entry_price: 100.0,
            stop_loss: 95.0,
            account_balance: 10000.0,
            account_risk: 1.0,
            total_shares: 20.0,
            position_value: 2000.0,
            status: "active".into(),
            tranches_json: "[]".into(),
            notes: Some("first plan".into()),
        },
        "000000000000001:00000:client",
    )
    .await
    .unwrap();

    let deltas = p::plans_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].symbol, "AAPL");
    assert_eq!(deltas[0].status, "active");
    assert_eq!(deltas[0].tranches_json, "[]");
    assert!(deltas[0].deleted_at.is_none());

    let tranches_json =
        serde_json::json!([{"id": "t1", "percent": 50.0, "shares": 10.0, "targetPrice": 110.0, "status": "planned", "filledAt": null}])
            .to_string();
    p::update_plan_tx(
        &mut c,
        &user_id,
        &p::UpdatePlanWriteArgs {
            id: "plan1".into(),
            status: "completed".into(),
            tranches_json: tranches_json.clone(),
            notes: Some("updated".into()),
        },
        "000000000000002:00000:client",
    )
    .await
    .unwrap();

    let deltas = p::plans_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(deltas[0].status, "completed");
    assert_eq!(deltas[0].tranches_json, tranches_json);
    assert_eq!(deltas[0].notes.as_deref(), Some("updated"));
    assert_eq!(deltas[0].hlc, "000000000000002:00000:client");

    p::soft_delete_plan_tx(&mut c, &user_id, "plan1", "000000000000003:00000:client")
        .await
        .unwrap();
    let deltas = p::plans_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1, "tombstone still appears in deltas");
    assert!(deltas[0].deleted_at.is_some());
    assert_eq!(deltas[0].hlc, "000000000000003:00000:client");
}

#[tokio::test]
async fn history_create_delete_flow_and_since() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let mut c = pool.acquire().await.unwrap();
    h::create_history_tx(
        &mut c,
        &user_id,
        &h::HistoryWriteArgs {
            workspace_id: workspace_id.clone(),
            id: "hist1".into(),
            symbol: "TSLA".into(),
            position_type: "short".into(),
            entry_price: 200.0,
            stop_loss: 210.0,
            account_balance: 10000.0,
            account_risk: 1.0,
            shares: 5.0,
            position_value: 1000.0,
            account_pct: 10.0,
            stop_loss_pct: 5.0,
            plan_id: Some("plan1".into()),
            tranches_json: serde_json::json!([
                {
                    "id": "t1",
                    "percent": 100.0,
                    "shares": 5.0,
                    "targetPrice": 200.0,
                    "status": "filled",
                    "filledAt": "2026-08-13T18:00:00Z"
                }
            ])
            .to_string(),
        },
        "000000000000001:00000:client",
    )
    .await
    .unwrap();

    let deltas = h::history_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].symbol, "TSLA");
    assert_eq!(deltas[0].plan_id.as_deref(), Some("plan1"));
    assert!(deltas[0].tranches_json.contains("filled"));
    assert!(deltas[0].deleted_at.is_none());
    assert_eq!(deltas[0].hlc, "000000000000001:00000:client");

    h::soft_delete_history_tx(&mut c, &user_id, "hist1", "000000000000002:00000:client")
        .await
        .unwrap();
    let deltas = h::history_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(deltas.len(), 1, "tombstone still appears in deltas");
    assert!(deltas[0].deleted_at.is_some());
    assert_eq!(deltas[0].hlc, "000000000000002:00000:client");
}

#[tokio::test]
async fn calculator_mutations_apply_through_push() {
    use tradstry_backend::graphql::notebook::sync::{NotebookMutation, apply_mutation};

    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let m1 = NotebookMutation {
        id: 1,
        name: "upsertPositionCalculatorRule".into(),
        args: serde_json::json!({
            "id": "rulex",
            "workspaceId": workspace_id,
            "accountBalance": 5000.0,
            "accountRisk": 1.5,
            "maxStopLossPct": 4.0,
        })
        .to_string(),
        hlc: "000000000000001:00000:client".into(),
    };
    apply_mutation(&pool, &user_id, "clientA", &m1)
        .await
        .unwrap();

    let rules = r::rules_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].workspace_id, workspace_id);
    assert_eq!(rules[0].account_balance, 5000.0);
    assert_eq!(rules[0].hlc, "000000000000001:00000:client");

    let m2 = NotebookMutation {
        id: 2,
        name: "createPositionCalculatorPlan".into(),
        args: serde_json::json!({
            "id": "planx",
            "workspaceId": workspace_id,
            "symbol": "MSFT",
            "positionType": "long",
            "entryPrice": 300.0,
            "stopLoss": 290.0,
            "accountBalance": 5000.0,
            "accountRisk": 1.5,
            "totalShares": 10.0,
            "positionValue": 3000.0,
            "status": "active",
            "tranchesJson": "[]",
            "notes": null,
        })
        .to_string(),
        hlc: "000000000000002:00000:client".into(),
    };
    apply_mutation(&pool, &user_id, "clientA", &m2)
        .await
        .unwrap();

    let plans = p::plans_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].symbol, "MSFT");
    assert_eq!(plans[0].hlc, "000000000000002:00000:client");

    let m3 = NotebookMutation {
        id: 3,
        name: "updatePositionCalculatorPlan".into(),
        args: serde_json::json!({
            "id": "planx",
            "status": "completed",
            "tranchesJson": "[]",
            "notes": "done",
        })
        .to_string(),
        hlc: "000000000000003:00000:client".into(),
    };
    apply_mutation(&pool, &user_id, "clientA", &m3)
        .await
        .unwrap();
    let plans = p::plans_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(plans[0].status, "completed");
    assert_eq!(plans[0].notes.as_deref(), Some("done"));

    let m4 = NotebookMutation {
        id: 4,
        name: "deletePositionCalculatorPlan".into(),
        args: serde_json::json!({ "id": "planx" }).to_string(),
        hlc: "000000000000004:00000:client".into(),
    };
    apply_mutation(&pool, &user_id, "clientA", &m4)
        .await
        .unwrap();
    let plans = p::plans_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert!(plans[0].deleted_at.is_some());

    let m5 = NotebookMutation {
        id: 5,
        name: "createPositionCalculatorHistory".into(),
        args: serde_json::json!({
            "id": "histx",
            "workspaceId": workspace_id,
            "symbol": "NVDA",
            "positionType": "long",
            "entryPrice": 400.0,
            "stopLoss": 380.0,
            "accountBalance": 5000.0,
            "accountRisk": 1.5,
            "shares": 5.0,
            "positionValue": 2000.0,
            "accountPct": 40.0,
            "stopLossPct": 5.0,
            "planId": "planx",
            "tranchesJson": "[{\"id\":\"t1\",\"status\":\"filled\"}]",
        })
        .to_string(),
        hlc: "000000000000005:00000:client".into(),
    };
    apply_mutation(&pool, &user_id, "clientA", &m5)
        .await
        .unwrap();
    let history = h::history_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].symbol, "NVDA");
    assert_eq!(history[0].plan_id.as_deref(), Some("planx"));
    assert!(history[0].tranches_json.contains("filled"));

    let m6 = NotebookMutation {
        id: 6,
        name: "deletePositionCalculatorHistory".into(),
        args: serde_json::json!({ "id": "histx" }).to_string(),
        hlc: "000000000000006:00000:client".into(),
    };
    apply_mutation(&pool, &user_id, "clientA", &m6)
        .await
        .unwrap();
    let history = h::history_since(&pool, &user_id, &workspace_id, None)
        .await
        .unwrap();
    assert!(history[0].deleted_at.is_some());
}
