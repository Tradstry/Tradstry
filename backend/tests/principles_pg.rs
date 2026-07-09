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
