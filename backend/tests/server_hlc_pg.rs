//! Server-authored writes must carry a real HLC.
//!
//! The desktop resolves conflicts with `CASE WHEN pulled_hlc > local_hlc THEN pulled ELSE
//! local` — a plain string compare. While server writes left `hlc` at its `''` default,
//! `'' > anything` was false, so the desktop pulled the row and then kept every one of its
//! own values: a server-side edit was silently dropped on the desktop while looking applied
//! in the browser. These tests pin the stamp onto the rows the web and MCP actually write.

mod pg_support;
use pg_support::{reset_schema, seed_user_workspace, test_pool};
use sqlx::PgPool;
use tradstry_backend::service::db::schema::tables::{playbook_table, trading_principle_table};

async fn migrate(pool: &PgPool) {
    tradstry_backend::service::db::schema::pg::migrate(pool)
        .await
        .expect("migrate");
}

async fn hlc_of(pool: &PgPool, table: &str, id: &str) -> String {
    let sql = format!("SELECT hlc FROM {table} WHERE id = $1");
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// What the desktop does with a pulled row.
fn desktop_would_take(pulled: &str, local: &str) -> bool {
    pulled > local
}

#[tokio::test]
async fn a_server_authored_playbook_write_is_stamped_and_reaches_the_desktop() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let pb = playbook_table::create_playbook(
        &pool,
        &user_id,
        playbook_table::CreatePlaybookInput {
            workspace_id: workspace_id.clone(),
            name: "Relative strength".into(),
            edge_name: "Inside day".into(),
            entry_rules: "wait for the first pullback".into(),
            exit_rules: "close below the 10-day".into(),
            position_sizing_rules: "0.4% risk".into(),
            additional_rules: None,
        },
    )
    .await
    .unwrap();

    let created = hlc_of(&pool, "playbooks", &pb.id).await;
    assert!(!created.is_empty(), "a server write must carry a stamp");

    // The row already exists on the desktop carrying the stamp it pulled. Before this fix
    // the edit below left hlc unchanged, so the desktop kept its own stale copy forever.
    playbook_table::update_playbook(
        &pool,
        &pb.id,
        &user_id,
        playbook_table::UpdatePlaybookInput {
            name: Some("Relative strength v2".into()),
            edge_name: None,
            entry_rules: None,
            exit_rules: None,
            position_sizing_rules: None,
            additional_rules: None,
            clear_additional_rules: false,
        },
    )
    .await
    .unwrap();

    let edited = hlc_of(&pool, "playbooks", &pb.id).await;
    assert!(
        desktop_would_take(&edited, &created),
        "edit stamp {edited} must outrank the stamp the desktop holds ({created})"
    );
}

#[tokio::test]
async fn a_server_stamp_outranks_the_empty_default_rows_already_carry() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let p = trading_principle_table::create_principle(
        &pool,
        &user_id,
        trading_principle_table::CreatePrincipleInput {
            workspace_id: workspace_id.clone(),
            playbook_id: None,
            evidence_note_id: None,
            title: "No chasing".into(),
            the_rule: "No entry more than 2% above the trigger".into(),
            why: "Chased entries cost the most".into(),
            intervention: None,
        },
    )
    .await
    .unwrap();

    let stamp = hlc_of(&pool, "trading_principles", &p.id).await;

    // Every row written before this fix sits at ''. A stamped write must beat it, which is
    // what lets the existing divergence heal on the next pull rather than persist.
    assert!(
        desktop_would_take(&stamp, ""),
        "{stamp} must outrank the '' rows already on disk"
    );
}

/// A hard DELETE vanishes from the sync delta, so the desktop never learns the row is gone
/// and shows it forever. Server-authored deletes must leave a stamped tombstone — and the
/// read paths must then hide it, or the row would simply reappear in the browser.
#[tokio::test]
async fn a_server_delete_leaves_a_stamped_tombstone_that_reads_hide() {
    let pool = test_pool().await;
    let _g = reset_schema(&pool).await;
    migrate(&pool).await;
    let (user_id, workspace_id) = seed_user_workspace(&pool).await;

    let pb = playbook_table::create_playbook(
        &pool,
        &user_id,
        playbook_table::CreatePlaybookInput {
            workspace_id: workspace_id.clone(),
            name: "Doomed".into(),
            edge_name: "e".into(),
            entry_rules: "r".into(),
            exit_rules: "r".into(),
            position_sizing_rules: "r".into(),
            additional_rules: None,
        },
    )
    .await
    .unwrap();

    assert!(
        playbook_table::delete_playbook(&pool, &pb.id, &user_id)
            .await
            .unwrap()
    );

    // Gone from every read the web and MCP use...
    assert!(
        playbook_table::find_playbook(&pool, &pb.id, &user_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        playbook_table::list_playbooks(&pool, &user_id, &workspace_id)
            .await
            .unwrap()
            .iter()
            .all(|p| p.id != pb.id)
    );

    // ...but the row still exists, tombstoned and stamped, so the delta carries it to the
    // desktop and the delete actually propagates.
    let (deleted_at, hlc): (Option<chrono::DateTime<chrono::Utc>>, String) =
        sqlx::query_as("SELECT deleted_at, hlc FROM playbooks WHERE id = $1")
            .bind(&pb.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(deleted_at.is_some(), "tombstone written");
    assert!(
        desktop_would_take(&hlc, ""),
        "tombstone carries a real stamp"
    );
}
