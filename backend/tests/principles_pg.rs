mod pg_support;
use pg_support::{reset_schema, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::db::schema::tables::trading_principle_table as tp;

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
        .expect("seed user");
}

async fn seed_account(pool: &PgPool, id: &str, user_id: &str) {
    sqlx::query("INSERT INTO accounts (id, user_id, name) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(user_id)
        .bind("acct")
        .execute(pool)
        .await
        .expect("seed account");
}

async fn seed_note(pool: &PgPool, id: &str, user_id: &str, account_id: &str) {
    sqlx::query(
        "INSERT INTO notebook_notes (id, user_id, account_id, title, document_json) \
         VALUES ($1, $2, $3, 'evidence', '{}')",
    )
    .bind(id)
    .bind(user_id)
    .bind(account_id)
    .execute(pool)
    .await
    .expect("seed note");
}

fn create_input(account_id: &str, title: &str) -> tp::CreatePrincipleInput {
    tp::CreatePrincipleInput {
        account_id: account_id.to_string(),
        title: title.to_string(),
        the_rule: "Do not touch a position 9:30-10:00 ET.".to_string(),
        why: "12 breaks cost -46%.".to_string(),
        intervention: None,
        playbook_id: None,
        evidence_note_id: None,
    }
}

#[tokio::test]
async fn evidence_note_from_another_account_is_rejected() {
    let pool = test_pool().await;
    reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_account(&pool, "a1", "u1").await;
    seed_account(&pool, "a2", "u1").await;
    seed_note(&pool, "n2", "u1", "a2").await;

    let mut input = create_input("a1", "30-min rule");
    input.evidence_note_id = Some("n2".to_string());

    let err = tp::create_principle(&pool, "u1", input)
        .await
        .expect_err("note in account a2 must not attach to a principle in a1");
    assert!(
        err.to_string().contains("not found in account"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn violation_from_another_account_is_rejected() {
    let pool = test_pool().await;
    reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_account(&pool, "a1", "u1").await;
    seed_account(&pool, "a2", "u1").await;

    // Principle governs account a2.
    let p = tp::create_principle(&pool, "u1", create_input("a2", "No setup, no trade"))
        .await
        .expect("create principle");

    // Trade lives in account a1.
    sqlx::query(
        "INSERT INTO journal_entries \
         (id, user_id, account_id, open_date, close_date, entry_price, exit_price, position_size, \
          symbol, symbol_name, status, total_pl, net_roi, duration, trade_type, mistakes, \
          entry_tactics, edges_spotted) \
         VALUES ('t1','u1','a1', now(), now(), 10.0, 11.0, 100.0, 'WOK','WOK','profit', \
                 10.0, 10.0, 60, 'long','','','')",
    )
    .execute(&pool)
    .await
    .expect("seed trade");

    let err = tp::set_trade_principle_violations(&pool, "u1", "t1", &[p.id.clone()])
        .await
        .expect_err("a trade in a1 must not violate a principle governing a2");
    assert!(
        err.to_string().contains("account"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn reorder_assigns_descending_priority() {
    let pool = test_pool().await;
    reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_account(&pool, "a1", "u1").await;

    let first = tp::create_principle(&pool, "u1", create_input("a1", "first"))
        .await
        .unwrap();
    let second = tp::create_principle(&pool, "u1", create_input("a1", "second"))
        .await
        .unwrap();

    // Put `second` at the top.
    tp::reorder_principles(&pool, "u1", &[second.id.clone(), first.id.clone()])
        .await
        .expect("reorder");

    let listed = tp::list_principles(&pool, "u1", "a1").await.unwrap();
    assert_eq!(listed[0].id, second.id, "first slice element sorts first");
    assert!(listed[0].priority > listed[1].priority);
}

#[tokio::test]
async fn violation_stats_use_dollar_expr_and_percent_roi() {
    use tradstry_backend::service::db::schema::tables::journal_table;

    let pool = test_pool().await;
    reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_account(&pool, "a1", "u1").await;

    let p = tp::create_principle(&pool, "u1", create_input("a1", "30-min rule"))
        .await
        .unwrap();

    // total_pl is a PERCENT. Dollars = position_size * entry_price * total_pl / 100.
    // 100 shares * $10 entry * -5% / 100 = -$50.
    sqlx::query(
        "INSERT INTO journal_entries \
         (id, user_id, account_id, open_date, close_date, entry_price, exit_price, position_size, \
          symbol, symbol_name, status, total_pl, net_roi, duration, trade_type, mistakes, \
          entry_tactics, edges_spotted) \
         VALUES ('t1','u1','a1', now(), now(), 10.0, 9.5, 100.0, 'WOK','WOK','loss', \
                 -5.0, -5.0, 60, 'long','','','')",
    )
    .execute(&pool)
    .await
    .unwrap();

    tp::set_trade_principle_violations(&pool, "u1", "t1", &[p.id.clone()])
        .await
        .unwrap();

    let rows = journal_table::aggregate_violation_stats_per_principle(&pool, "u1", "a1")
        .await
        .expect("aggregate");

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.principle_id, p.id);
    assert_eq!(row.total_trades, 1);
    assert_eq!(row.losing_trades, 1);
    assert_eq!(row.winning_trades, 0);
    assert!(
        (row.cumulative_roi - (-5.0)).abs() < 1e-9,
        "roi is the raw percent, got {}",
        row.cumulative_roi
    );
    assert!(
        (row.cumulative_profit - (-50.0)).abs() < 1e-9,
        "profit is dollars via DOLLAR_PL_EXPR, got {}",
        row.cumulative_profit
    );
}

#[tokio::test]
async fn deleting_a_playbook_with_principles_is_blocked() {
    use tradstry_backend::service::db::schema::tables::playbook_table;

    let pool = test_pool().await;
    reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_account(&pool, "a1", "u1").await;

    let pb = playbook_table::create_playbook(
        &pool,
        "u1",
        playbook_table::CreatePlaybookInput {
            name: "High Volume Edge".to_string(),
            edge_name: "HV".to_string(),
            entry_rules: "volume support".to_string(),
            exit_rules: "10-day EMA".to_string(),
            position_sizing_rules: "3% risk".to_string(),
            additional_rules: None,
        },
    )
    .await
    .expect("create playbook");

    let mut input = create_input("a1", "Never enter Whole number (late)");
    input.playbook_id = Some(pb.id.clone());
    tp::create_principle(&pool, "u1", input)
        .await
        .expect("create principle");

    let err = playbook_table::delete_playbook(&pool, &pb.id, "u1")
        .await
        .expect_err("playbook with principles must not delete");

    assert!(
        err.to_string().contains("Never enter Whole number (late)"),
        "error must name the blocking principle, got: {err}"
    );
}

#[tokio::test]
async fn deleting_a_playbook_without_principles_still_works() {
    use tradstry_backend::service::db::schema::tables::playbook_table;

    let pool = test_pool().await;
    reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;

    let pb = playbook_table::create_playbook(
        &pool,
        "u1",
        playbook_table::CreatePlaybookInput {
            name: "RS / Inside Day".to_string(),
            edge_name: "RS".to_string(),
            entry_rules: "inside day at 10-day EMA".to_string(),
            exit_rules: "trail 10-day EMA".to_string(),
            position_sizing_rules: "3% risk".to_string(),
            additional_rules: None,
        },
    )
    .await
    .expect("create playbook");

    // The blocking pre-check must not reject every deletion.
    assert!(
        playbook_table::delete_playbook(&pool, &pb.id, "u1")
            .await
            .expect("playbook with no principles must delete"),
    );
}

#[tokio::test]
async fn playbook_owned_by_another_user_is_rejected() {
    use tradstry_backend::service::db::schema::tables::playbook_table;

    let pool = test_pool().await;
    reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_user(&pool, "u2").await;
    seed_account(&pool, "a1", "u1").await;

    let other_pb = playbook_table::create_playbook(
        &pool,
        "u2",
        playbook_table::CreatePlaybookInput {
            name: "Not yours".to_string(),
            edge_name: "X".to_string(),
            entry_rules: "x".to_string(),
            exit_rules: "x".to_string(),
            position_sizing_rules: "x".to_string(),
            additional_rules: None,
        },
    )
    .await
    .expect("create other user's playbook");

    let mut input = create_input("a1", "Borrowed rule");
    input.playbook_id = Some(other_pb.id.clone());

    let err = tp::create_principle(&pool, "u1", input)
        .await
        .expect_err("must not reference another user's playbook");
    assert!(
        err.to_string().contains("not found"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn deleting_the_evidence_note_nulls_the_link_and_keeps_the_principle() {
    let pool = test_pool().await;
    reset_schema(&pool).await;
    migrate(&pool).await;

    seed_user(&pool, "u1").await;
    seed_account(&pool, "a1", "u1").await;
    seed_note(&pool, "n1", "u1", "a1").await;

    let mut input = create_input("a1", "30-min rule");
    input.evidence_note_id = Some("n1".to_string());
    let p = tp::create_principle(&pool, "u1", input)
        .await
        .expect("create principle with evidence note");
    assert_eq!(p.evidence_note_id.as_deref(), Some("n1"));

    sqlx::query("DELETE FROM notebook_notes WHERE id = 'n1'")
        .execute(&pool)
        .await
        .expect("delete note");

    let after = tp::find_principle(&pool, &p.id, "u1")
        .await
        .unwrap()
        .expect("principle must survive its evidence note");
    assert_eq!(
        after.evidence_note_id, None,
        "ON DELETE SET NULL must orphan the link, not the principle"
    );
}
