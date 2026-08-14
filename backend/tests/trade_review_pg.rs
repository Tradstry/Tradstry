mod pg_support;

use chrono::{Duration, Utc};
use tradstry_backend::service::db::schema::tables::{
    brokerage_table::{self, NewBrokerageTransaction},
    journal_table, manual_execution_claim_table, position_calculator_plans_table,
    trade_review_table,
};

fn broker_fill(
    id: &str,
    side: &str,
    price: f64,
    units: f64,
    at: chrono::DateTime<Utc>,
) -> NewBrokerageTransaction {
    NewBrokerageTransaction {
        snaptrade_id: id.to_string(),
        symbol: Some("AVGO".to_string()),
        symbol_description: Some("Broadcom Inc.".to_string()),
        raw_symbol: Some("AVGO".to_string()),
        currency: "USD".to_string(),
        transaction_type: side.to_string(),
        option_type: None,
        price,
        units,
        amount: Some(price * units),
        fee: 0.5,
        fx_rate: Some(1.0),
        description: None,
        trade_date: Some(at.to_rfc3339()),
        settlement_date: at.to_rfc3339(),
        institution: "Webull".to_string(),
        external_reference_id: Some(id.to_string()),
        raw_json: "{}".to_string(),
        contract_multiplier: 1.0,
        underlying_symbol: None,
        option_kind: None,
        strike_price: None,
        option_expiration: None,
    }
}

#[tokio::test]
async fn broker_episode_can_be_confirmed_finalized_and_published() {
    let pool = pg_support::test_pool().await;
    let (user_id, workspace_id) = pg_support::seed_user_workspace(&pool).await;
    let plan = position_calculator_plans_table::create_plan(
        &pool,
        &user_id,
        position_calculator_plans_table::CreatePositionCalculatorPlanInput {
            workspace_id: workspace_id.clone(),
            symbol: "AVGO".to_string(),
            position_type: "long".to_string(),
            entry_price: 100.0,
            stop_loss: 95.0,
            account_balance: 10_000.0,
            account_risk: 1.0,
            total_shares: 10.0,
            position_value: 1_000.0,
            tranches: vec![position_calculator_plans_table::CreateTrancheInput {
                percent: 100.0,
                shares: 10.0,
                target_price: 100.0,
            }],
            notes: None,
            instrument_json: None,
        },
    )
    .await
    .unwrap();

    let tranche_id = plan.tranches[0].id.clone();
    let claim = manual_execution_claim_table::create_claim(
        &pool,
        &user_id,
        &plan.id,
        &tranche_id,
        "10",
        "100.50",
        &Utc::now().to_rfc3339(),
    )
    .await
    .unwrap();
    assert_eq!(claim.status, "pending");
    assert!(
        manual_execution_claim_table::create_claim(
            &pool,
            &user_id,
            &plan.id,
            &tranche_id,
            "11",
            "100.50",
            &Utc::now().to_rfc3339(),
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("cannot exceed")
    );

    let opened = Utc::now() + Duration::minutes(1);
    let fills = vec![
        broker_fill("buy", "BUY", 101.0, 10.0, opened),
        broker_fill("sell", "SELL", 106.0, 10.0, opened + Duration::minutes(5)),
    ];
    brokerage_table::upsert_transactions(
        &pool,
        &user_id,
        &workspace_id,
        &fills,
        &mut brokerage_table::SignatureCounts::new(),
    )
    .await
    .unwrap();

    assert_eq!(
        trade_review_table::rebuild_workspace(&pool, &user_id, &workspace_id)
            .await
            .unwrap(),
        1
    );
    let inbox = trade_review_table::list_inbox(&pool, &user_id, &workspace_id)
        .await
        .unwrap();
    let item = inbox
        .iter()
        .find(|item| item.instrument_key == "equity:AVGO")
        .unwrap();
    assert!(item.suggestions_json.contains(&plan.id));

    let episode_id = item.episode_id.clone();
    trade_review_table::confirm_match(&pool, &user_id, &episode_id, &plan.id)
        .await
        .unwrap();
    let reconciled = manual_execution_claim_table::list_claims(&pool, &user_id, &workspace_id)
        .await
        .unwrap();
    assert_eq!(reconciled.len(), 1);
    assert_eq!(reconciled[0].status, "reconciled");
    assert!(reconciled[0].reconciled_match_id.is_some());
    let confirmed = trade_review_table::list_inbox(&pool, &user_id, &workspace_id)
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.episode_id == episode_id)
        .unwrap();
    let match_id = confirmed.confirmed_match_id.unwrap();
    trade_review_table::finalize_review(&pool, &user_id, &match_id, "{}", None, true)
        .await
        .unwrap();
    let journal_id = trade_review_table::publish_review(&pool, &user_id, &match_id)
        .await
        .unwrap();
    let journal = journal_table::find_journal_entry(&pool, &journal_id, &user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(journal.symbol, "AVGO");
    assert_eq!(journal.position_size, 10.0);
    assert_eq!(journal.entry_price, 101.0);
    assert_eq!(journal.exit_price, 106.0);
    assert_eq!(
        trade_review_table::publish_review(&pool, &user_id, &match_id)
            .await
            .unwrap(),
        journal_id
    );
}
