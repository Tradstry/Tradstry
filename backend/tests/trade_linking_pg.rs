//! The rules every trade-link write must obey.
//!
//! `trade_tags` and `trade_principle_violations` carry no `user_id` and no clock of their
//! own. Ownership is purely transitive, and they reach the desktop only inside the journal
//! delta — which is pulled on the entry's `updated_at` cursor. So a link write has to do two
//! things the tables cannot do for themselves: check *both* sides belong to the caller, and
//! bump the entry it touched.

mod pg_support;
use pg_support::{reset_schema, seed_user_account, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::db::schema::tables::{tags_table, trading_principle_table as tp};

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

async fn seed_trade(pool: &PgPool, id: &str, user_id: &str, account_id: &str) {
    sqlx::query(
        "INSERT INTO journal_entries \
         (id, user_id, account_id, open_date, close_date, entry_price, exit_price, position_size, \
          symbol, symbol_name, status, total_pl, net_roi, duration, trade_type, mistakes, \
          entry_tactics, edges_spotted) \
         VALUES ($1, $2, $3, now(), now(), 10.0, 9.5, 100.0, 'WOK','WOK','loss', \
                 -5.0, -5.0, 60, 'long','','','')",
    )
    .bind(id)
    .bind(user_id)
    .bind(account_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_user_and_account(pool: &PgPool, user: &str, account: &str) {
    sqlx::query("INSERT INTO users (id, clerk_uuid) VALUES ($1, $1)")
        .bind(user)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO accounts (id, user_id, name) VALUES ($1, $2, 'Acct')")
        .bind(account)
        .bind(user)
        .execute(pool)
        .await
        .unwrap();
}

async fn trade_clock(pool: &PgPool, id: &str) -> (String, chrono::DateTime<chrono::Utc>) {
    sqlx::query_as("SELECT hlc, updated_at FROM journal_entries WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn tagging_a_trade_bumps_its_clock_so_the_link_reaches_the_desktop() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    seed_trade(&pool, "t1", &user_id, &account_id).await;

    tags_table::ensure_default_categories(&pool, &user_id)
        .await
        .unwrap();
    let cat = tags_table::list_categories(&pool, &user_id).await.unwrap()[0].clone();
    let tag = tags_table::create_tag(&pool, &user_id, &cat.id, "chased", None)
        .await
        .unwrap();

    let (before_hlc, before_at) = trade_clock(&pool, "t1").await;

    tags_table::set_trade_tags(&pool, &user_id, "t1", std::slice::from_ref(&tag.id))
        .await
        .unwrap();

    let (after_hlc, after_at) = trade_clock(&pool, "t1").await;
    assert!(
        after_hlc > before_hlc,
        "the trade's stamp must advance ({after_hlc} vs {before_hlc})"
    );
    assert!(after_at >= before_at, "the pull cursor must advance");
}

/// `trade_tags` has no `user_id`. Validating only the tag would let a caller staple their
/// own tags onto a stranger's trade.
#[tokio::test]
async fn a_foreign_trade_cannot_be_tagged() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, _account_id) = seed_user_account(&pool).await;

    seed_user_and_account(&pool, "stranger", "stranger-acct").await;
    seed_trade(&pool, "theirs", "stranger", "stranger-acct").await;

    tags_table::ensure_default_categories(&pool, &user_id)
        .await
        .unwrap();
    let cat = tags_table::list_categories(&pool, &user_id).await.unwrap()[0].clone();
    let mine = tags_table::create_tag(&pool, &user_id, &cat.id, "mine", None)
        .await
        .unwrap();

    let err = tags_table::set_trade_tags(&pool, &user_id, "theirs", &[mine.id])
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("not found"),
        "a stranger's trade must be invisible, got: {err}"
    );

    let links: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM trade_tags WHERE journal_entry_id = 'theirs'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(links, 0);
}

#[tokio::test]
async fn flagging_a_violation_bumps_the_trade_clock() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    seed_trade(&pool, "t1", &user_id, &account_id).await;

    let p = tp::create_principle(
        &pool,
        &user_id,
        tp::CreatePrincipleInput {
            account_id: account_id.clone(),
            playbook_id: None,
            evidence_note_id: None,
            title: "No chasing".into(),
            the_rule: "No entry >2% above the trigger".into(),
            why: "Chased entries cost the most".into(),
            intervention: None,
        },
    )
    .await
    .unwrap();

    let (before_hlc, _) = trade_clock(&pool, "t1").await;
    tp::set_trade_principle_violations(&pool, &user_id, "t1", std::slice::from_ref(&p.id))
        .await
        .unwrap();
    let (after_hlc, _) = trade_clock(&pool, "t1").await;

    assert!(after_hlc > before_hlc, "violation link must bump the trade");
}

/// Principles are account-scoped and so are trades. Linking across accounts would silently
/// corrupt per-account analytics.
#[tokio::test]
async fn a_principle_cannot_be_violated_by_a_trade_in_another_account() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, account_a) = seed_user_account(&pool).await;

    sqlx::query("INSERT INTO accounts (id, user_id, name) VALUES ('acct-b', $1, 'B')")
        .bind(&user_id)
        .execute(&pool)
        .await
        .unwrap();
    seed_trade(&pool, "t-in-b", &user_id, "acct-b").await;

    let p = tp::create_principle(
        &pool,
        &user_id,
        tp::CreatePrincipleInput {
            account_id: account_a.clone(),
            playbook_id: None,
            evidence_note_id: None,
            title: "Account A rule".into(),
            the_rule: "r".into(),
            why: "w".into(),
            intervention: None,
        },
    )
    .await
    .unwrap();

    let err = tp::set_trade_principle_violations(&pool, &user_id, "t-in-b", &[p.id])
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("governs account"),
        "cross-account link must be refused, got: {err}"
    );
}

/// The read side must close the loop. Without a trade's current tags and violations, an
/// agent cannot use `add` sensibly and cannot use `remove` at all — it has nothing to name.
#[tokio::test]
async fn a_trades_tags_and_violations_are_readable_back() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    seed_trade(&pool, "t1", &user_id, &account_id).await;
    seed_trade(&pool, "t2", &user_id, &account_id).await;

    tags_table::ensure_default_categories(&pool, &user_id)
        .await
        .unwrap();
    let mistake_cat = tags_table::list_categories(&pool, &user_id)
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.role.as_ref().map(|r| r.as_str()) == Some("mistake"))
        .expect("a seeded mistake-role category");

    let tag = tags_table::create_tag(&pool, &user_id, &mistake_cat.id, "chased entry", None)
        .await
        .unwrap();
    tags_table::set_trade_tags(&pool, &user_id, "t1", std::slice::from_ref(&tag.id))
        .await
        .unwrap();

    let p = tp::create_principle(
        &pool,
        &user_id,
        tp::CreatePrincipleInput {
            account_id: account_id.clone(),
            playbook_id: None,
            evidence_note_id: None,
            title: "No chasing".into(),
            the_rule: "r".into(),
            why: "w".into(),
            intervention: None,
        },
    )
    .await
    .unwrap();
    tp::set_trade_principle_violations(&pool, &user_id, "t1", std::slice::from_ref(&p.id))
        .await
        .unwrap();

    let ids = vec!["t1".to_string(), "t2".to_string()];
    let tags = tags_table::tags_for_trades(&pool, &ids).await.unwrap();
    let violations = tp::principles_for_trades(&pool, &user_id, &ids)
        .await
        .unwrap();

    let t1_tags = tags.get("t1").expect("t1 has tags");
    assert_eq!(t1_tags.len(), 1);
    assert_eq!(t1_tags[0].tag.name, "chased entry");
    // The role is the load-bearing bit: it is what marks the trade flawed.
    assert_eq!(
        t1_tags[0].role.as_ref().map(|r| r.as_str()),
        Some("mistake")
    );
    assert_eq!(violations.get("t1").map(Vec::len), Some(1));

    // An untouched trade is simply absent, rather than carrying empty arrays.
    assert!(!tags.contains_key("t2"));
    assert!(!violations.contains_key("t2"));
}
