mod pg_support;
use pg_support::{seed_user_account, test_pool};
use tradstry_backend::graphql::notebook::crdt as notebook_crdt;
use tradstry_backend::service::ai::db as ai_db;
use tradstry_backend::service::ai::jobs;
use tradstry_backend::service::ai::projector;
use tradstry_backend::service::ai::types::AiSourceDocument;
use tradstry_backend::service::db::client::Db;
use tradstry_backend::service::db::schema::tables::{notebook::crdt, notebook::notes};
use uuid::Uuid;

/// A one-paragraph Lexical document with a distinctive marker word, so a
/// duplicated projection is trivially detectable by counting occurrences.
const SEED_DOC: &str = r#"{"root":{"children":[{"type":"paragraph","children":[{"type":"text","text":"seedmarker","format":0,"detail":0,"mode":"normal","style":"","version":1}],"direction":null,"format":"","indent":0,"version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#;

/// A second document carrying a distinct marker. Appended as an independent seed
/// update, it merges by concatenation, so a correct projection contains both
/// markers while a stale `document_json` still holds only the original.
const FRESH_DOC: &str = r#"{"root":{"children":[{"type":"paragraph","children":[{"type":"text","text":"freshmarker","format":0,"detail":0,"mode":"normal","style":"","version":1}],"direction":null,"format":"","indent":0,"version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#;

/// H1 + paragraph so both the derived title and the body survive a round-trip.
const TITLE_DOC: &str = r#"{"root":{"children":[{"type":"heading","tag":"h1","children":[{"type":"text","text":"Refreshed Title","format":0,"detail":0,"mode":"normal","style":"","version":1}],"direction":null,"format":"","indent":0,"version":1},{"type":"paragraph","children":[{"type":"text","text":"bodymarker","format":0,"detail":0,"mode":"normal","style":"","version":1}],"direction":null,"format":"","indent":0,"version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#;

async fn make_note(pool: &sqlx::PgPool, user_id: &str, account_id: String, doc: &str) -> String {
    notes::create_notebook_note(
        pool,
        user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            account_id,
            document_json: doc.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap()
    .id
}

async fn append_update(pool: &sqlx::PgPool, note_id: &str, update: &[u8]) {
    sqlx::query("INSERT INTO notebook_note_updates (note_id, update) VALUES ($1, $2)")
        .bind(note_id)
        .bind(update)
        .execute(pool)
        .await
        .unwrap();
}

async fn make_legacy_note(pool: &sqlx::PgPool, user_id: &str, account_id: String) -> String {
    notes::create_notebook_note(
        pool,
        user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            account_id,
            document_json: SEED_DOC.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap()
    .id
}

async fn update_row_count(pool: &sqlx::PgPool, note_id: &str) -> i64 {
    sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM notebook_note_updates WHERE note_id = $1")
        .bind(note_id)
        .fetch_one(pool)
        .await
        .unwrap()
        .0
}

async fn migrated_pool() -> sqlx::PgPool {
    let pool = test_pool().await;
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .expect("migrate");
    pool
}

#[tokio::test]
async fn crdt_note_rejects_plain_body_write() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            account_id,
            document_json: r#"{"root":{"children":[]}}"#.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap();

    crdt::mark_crdt(&pool, &note.id, &[1, 2, 3]).await.unwrap();

    let err = notes::update_notebook_note(
        &pool,
        &note.id,
        &user_id,
        notes::UpdateNotebookNoteInput {
            document_json: Some(r#"{"root":{"children":[1]}}"#.into()),
            ..Default::default()
        },
    )
    .await;

    let msg = format!("{:?}", err.unwrap_err());
    assert!(
        msg.contains("CRDT_NOTE"),
        "expected CRDT_NOTE rejection, got {msg}"
    );
}

#[tokio::test]
async fn crdt_note_still_accepts_metadata_writes() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            account_id,
            document_json: r#"{"root":{"children":[]}}"#.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap();
    crdt::mark_crdt(&pool, &note.id, &[1, 2, 3]).await.unwrap();

    // folder_id is metadata; it still merges by HLC LWW and must not be blocked.
    notes::update_notebook_note(
        &pool,
        &note.id,
        &user_id,
        notes::UpdateNotebookNoteInput {
            folder_id: None,
            ..Default::default()
        },
    )
    .await
    .expect("metadata write on a crdt note must succeed");
}

#[tokio::test]
async fn seeding_note_rejects_plain_body_write() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            account_id,
            document_json: r#"{"root":{"children":[]}}"#.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO notebook_note_crdt (note_id, state, state_vector) VALUES ($1, 'seeding', $2)",
    )
    .bind(&note.id)
    .bind(&[1u8, 2, 3][..])
    .execute(&pool)
    .await
    .unwrap();

    let err = notes::update_notebook_note(
        &pool,
        &note.id,
        &user_id,
        notes::UpdateNotebookNoteInput {
            document_json: Some(r#"{"root":{"children":[1]}}"#.into()),
            ..Default::default()
        },
    )
    .await;

    let msg = format!("{:?}", err.unwrap_err());
    assert!(
        msg.contains("CRDT_NOTE"),
        "expected CRDT_NOTE rejection, got {msg}"
    );
}

#[tokio::test]
async fn legacy_note_accepts_plain_body_write() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            account_id,
            document_json: r#"{"root":{"children":[]}}"#.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap();

    notes::update_notebook_note(
        &pool,
        &note.id,
        &user_id,
        notes::UpdateNotebookNoteInput {
            document_json: Some(r#"{"root":{"children":[1]}}"#.into()),
            ..Default::default()
        },
    )
    .await
    .expect("legacy notes are unchanged");
}

#[tokio::test]
async fn migration_adds_crdt_tables() {
    let pool = test_pool().await;
    // Test binaries share one database and run in an arbitrary order. Migrating
    // here (idempotent, no reset) means this file does not depend on schema_pg
    // having run first.
    tradstry_backend::service::db::schema::pg::migrate(&pool)
        .await
        .expect("migrate");

    for table in ["notebook_note_crdt", "notebook_note_updates"] {
        let (exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(exists, "{table} missing");
    }

    let (exists,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns
         WHERE table_name='ai_source_documents' AND column_name='body_version')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(exists, "ai_source_documents.body_version missing");
}

#[tokio::test]
async fn seed_note_transitions_legacy_to_crdt() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note_id = make_legacy_note(&pool, &user_id, account_id).await;

    assert_eq!(
        crdt::note_state(&pool, &note_id).await.unwrap(),
        crdt::NoteState::Legacy
    );

    crdt::seed_note(&pool, &note_id).await.unwrap();

    assert_eq!(
        crdt::note_state(&pool, &note_id).await.unwrap(),
        crdt::NoteState::Crdt
    );
    assert_eq!(update_row_count(&pool, &note_id).await, 1);

    let (state_vector, projected_seq): (Vec<u8>, i64) = sqlx::query_as(
        "SELECT state_vector, projected_seq FROM notebook_note_crdt WHERE note_id = $1",
    )
    .bind(&note_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !state_vector.is_empty(),
        "state vector must be a real, non-empty Yjs vector"
    );
    assert_eq!(projected_seq, 0, "seeding must not advance projected_seq");
}

#[tokio::test]
async fn seed_note_is_idempotent() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note_id = make_legacy_note(&pool, &user_id, account_id).await;

    crdt::seed_note(&pool, &note_id).await.unwrap();
    crdt::seed_note(&pool, &note_id).await.unwrap();

    assert_eq!(
        update_row_count(&pool, &note_id).await,
        1,
        "re-seeding a crdt note must not append a second update row"
    );
    assert_eq!(
        crdt::note_state(&pool, &note_id).await.unwrap(),
        crdt::NoteState::Crdt
    );
}

/// The corruption test. Two Y.Docs independently seeded from the same
/// document_json do NOT conflict — Yjs merges them by concatenation, so every
/// paragraph appears twice with no error. This is why only the server seeds,
/// exactly once, and why seed_note is a no-op on a crdt note.
#[tokio::test]
async fn two_independent_seeds_duplicate_content() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    // Bypass seed_note: build two independent seed updates from the same source.
    let update_a = projector::seed(SEED_DOC).await.unwrap().update;
    let update_b = projector::seed(SEED_DOC).await.unwrap().update;
    let projected = projector::project(&[update_a, update_b]).await.unwrap();

    let occurrences = projected.matches("seedmarker").count();
    assert!(
        occurrences >= 2,
        "two independent seeds must duplicate content; expected >=2 marker occurrences, \
         got {occurrences} in: {projected}"
    );

    // The real path can never reach the above: seed_note refuses to re-seed a crdt note.
    let note_id = make_legacy_note(&pool, &user_id, account_id).await;
    crdt::seed_note(&pool, &note_id).await.unwrap();
    crdt::seed_note(&pool, &note_id).await.unwrap();
    assert_eq!(
        update_row_count(&pool, &note_id).await,
        1,
        "seed_note on a crdt note must be a no-op, so content can never duplicate via the real path"
    );
}

#[tokio::test]
async fn seed_note_leaves_document_json_projectable() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note_id = make_legacy_note(&pool, &user_id, account_id).await;

    crdt::seed_note(&pool, &note_id).await.unwrap();

    let (stored_update,): (Vec<u8>,) =
        sqlx::query_as("SELECT update FROM notebook_note_updates WHERE note_id = $1 ORDER BY seq")
            .bind(&note_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let projected = projector::project(&[stored_update]).await.unwrap();
    assert!(
        projected.contains("seedmarker"),
        "projecting the stored update must recover the original text: {projected}"
    );
}

/// Hard-gate evidence: a document with the node types most likely to lose
/// attributes must survive seed -> project with its title still derivable.
#[tokio::test]
async fn seed_note_round_trips_a_realistic_document() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    let doc = r#"{"root":{"children":[
      {"type":"heading","tag":"h1","children":[{"type":"text","text":"Jul 7 choppy open","format":0,"detail":0,"mode":"normal","style":"","version":1}],"direction":null,"format":"","indent":0,"version":1},
      {"type":"paragraph","children":[{"type":"text","text":"Waited for the reclaim.","format":0,"detail":0,"mode":"normal","style":"","version":1}],"direction":null,"format":"","indent":0,"version":1},
      {"type":"code","language":"rust","children":[{"type":"text","text":"fn main() {}","format":0,"detail":0,"mode":"normal","style":"","version":1}],"direction":null,"format":"","indent":0,"version":1},
      {"type":"horizontalrule","version":1}
    ],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#;

    let note = notes::create_notebook_note(
        &pool,
        &user_id,
        notes::CreateNotebookNoteInput {
            id: None,
            account_id,
            document_json: doc.into(),
            trade_ids: vec![],
            folder_id: None,
        },
    )
    .await
    .unwrap();

    // The title is derived server-side from the first H1.
    assert_eq!(note.title, "Jul 7 choppy open");

    crdt::seed_note(&pool, &note.id).await.unwrap();

    let updates: Vec<Vec<u8>> = sqlx::query_as::<_, (Vec<u8>,)>(
        "SELECT update FROM notebook_note_updates WHERE note_id = $1 ORDER BY seq",
    )
    .bind(&note.id)
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.0)
    .collect();

    let projected = projector::project(&updates).await.unwrap();

    for needle in [
        "Jul 7 choppy open",
        "Waited for the reclaim.",
        "fn main() {}",
        "\"language\":\"rust\"",
        "horizontalrule",
    ] {
        assert!(
            projected.contains(needle),
            "projection lost {needle}: {projected}"
        );
    }
    assert_eq!(
        projected.matches("Waited for the reclaim.").count(),
        1,
        "content duplicated"
    );
}

/// Defense 1 (catch-up). A crdt note whose projection lags must be projected
/// inline by the reindex before its blocks are embedded, so the index never
/// carries text the user already changed. Fails before the catch-up is wired in.
#[tokio::test]
async fn reindex_catches_up_stale_crdt_projection() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note_id = make_note(&pool, &user_id, account_id.clone(), SEED_DOC).await;

    // seed -> update seq 1, projected_seq 0, document_json still "seedmarker".
    crdt::seed_note(&pool, &note_id).await.unwrap();

    // Append a content-changing update but do NOT project. The projection now
    // contains "freshmarker"; the stale document_json does not.
    let extra = projector::seed(FRESH_DOC).await.unwrap().update;
    append_update(&pool, &note_id, &extra).await;

    let db = Db::from_pool(pool.clone());
    let docs = jobs::build_indexable_sources(&db, &user_id, &account_id)
        .await
        .unwrap();
    let (doc, _, _) = docs
        .iter()
        .find(|(d, _, _)| d.source_id == note_id)
        .expect("the note must be indexable");

    assert!(
        doc.body_text.contains("freshmarker"),
        "reindex must project the crdt note inline before indexing; got: {}",
        doc.body_text
    );

    // seq is a global BIGSERIAL, so body_version carries the note's max update seq
    // (projected_seq), not a per-note count. It must equal that max and be > 0.
    let (max_seq,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(seq), 0) FROM notebook_note_updates WHERE note_id = $1",
    )
    .bind(&note_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(max_seq > 0);
    assert_eq!(
        doc.body_version, max_seq,
        "body_version must be stamped with projected_seq (the note's max update seq)"
    );
}

/// Defense 2 (version guard). Leases + retries make out-of-order completion
/// inevitable: a slow job carrying older blocks must not overwrite a newer
/// vector. The source-doc upsert rejects a lower body_version.
#[tokio::test]
async fn source_document_upsert_rejects_older_body_version() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let db = Db::from_pool(pool.clone());
    let source_id = Uuid::new_v4().to_string();

    let mk = |body: &str, version: i64| AiSourceDocument {
        id: Uuid::new_v4().to_string(),
        user_id: user_id.clone(),
        account_id: account_id.clone(),
        source_type: "notebook_note".into(),
        source_id: source_id.clone(),
        title: "t".into(),
        body_text: body.into(),
        metadata_json: "{}".into(),
        content_hash: format!("hash-{version}"),
        body_version: version,
    };

    ai_db::replace_source_documents_for_account(&db, &user_id, &account_id, &[mk("v5-text", 5)])
        .await
        .unwrap();
    ai_db::replace_source_documents_for_account(&db, &user_id, &account_id, &[mk("v3-text", 3)])
        .await
        .unwrap();

    let (body,): (String,) = sqlx::query_as(
        "SELECT body_text FROM ai_source_documents WHERE user_id=$1 AND account_id=$2 AND source_id=$3",
    )
    .bind(&user_id)
    .bind(&account_id)
    .bind(&source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        body, "v5-text",
        "an older body_version must not overwrite a newer vector's source doc"
    );
}

/// refresh_projection writes document_json matching the CRDT, keeps the derived
/// title correct, and advances freshness.
#[tokio::test]
async fn refresh_projection_writes_document_title_and_advances_freshness() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note_id = make_note(&pool, &user_id, account_id, TITLE_DOC).await;

    crdt::seed_note(&pool, &note_id).await.unwrap();
    assert!(
        !crdt::is_projection_fresh(&pool, &note_id).await.unwrap(),
        "an unprojected seed update leaves the projection stale"
    );

    crdt::refresh_projection(&pool, &note_id).await.unwrap();
    assert!(crdt::is_projection_fresh(&pool, &note_id).await.unwrap());

    let (document_json, title): (String, String) =
        sqlx::query_as("SELECT document_json, title FROM notebook_notes WHERE id=$1")
            .bind(&note_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        document_json.contains("bodymarker"),
        "projection must match the CRDT body: {document_json}"
    );
    assert_eq!(
        title, "Refreshed Title",
        "projection must keep the derived title correct"
    );
}

/// is_projection_fresh flips false on every append past projected_seq and true
/// again after re-projection.
#[tokio::test]
async fn is_projection_fresh_tracks_each_append() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note_id = make_note(&pool, &user_id, account_id, SEED_DOC).await;
    crdt::seed_note(&pool, &note_id).await.unwrap();

    crdt::refresh_projection(&pool, &note_id).await.unwrap();
    assert!(crdt::is_projection_fresh(&pool, &note_id).await.unwrap());

    let extra = projector::seed(FRESH_DOC).await.unwrap().update;
    append_update(&pool, &note_id, &extra).await;
    assert!(
        !crdt::is_projection_fresh(&pool, &note_id).await.unwrap(),
        "an append past the projected seq is stale until re-projected"
    );

    crdt::refresh_projection(&pool, &note_id).await.unwrap();
    assert!(crdt::is_projection_fresh(&pool, &note_id).await.unwrap());
}

// ---- Task 8: the byte pipe (appendNotebookUpdates / notebookUpdatesSince) ----

#[tokio::test]
async fn append_updates_returns_new_max_seq() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note_id = make_note(&pool, &user_id, account_id, SEED_DOC).await;
    crdt::mark_crdt(&pool, &note_id, &[1, 2, 3]).await.unwrap();

    let a = vec![10u8, 20, 30];
    let b = vec![40u8, 50];
    let max = notebook_crdt::append_updates(&pool, &user_id, &note_id, &[a.clone(), b.clone()])
        .await
        .unwrap();

    let rows = notebook_crdt::updates_since(&pool, &user_id, &note_id, 0)
        .await
        .unwrap();
    assert_eq!(
        rows.iter().map(|(_, u)| u.clone()).collect::<Vec<_>>(),
        vec![a, b],
        "rows must land in insertion order"
    );
    assert_eq!(
        rows.last().unwrap().0,
        max,
        "the returned value must be the note's new max seq"
    );
}

#[tokio::test]
async fn append_to_legacy_note_is_rejected() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note_id = make_legacy_note(&pool, &user_id, account_id).await;

    let err = notebook_crdt::append_updates(&pool, &user_id, &note_id, &[vec![1, 2, 3]]).await;
    assert!(err.is_err(), "appending to a legacy note must be rejected");
    assert_eq!(
        update_row_count(&pool, &note_id).await,
        0,
        "a rejected append must not write any rows"
    );
}

#[tokio::test]
async fn append_to_seeding_note_is_rejected() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note_id = make_note(&pool, &user_id, account_id, SEED_DOC).await;
    sqlx::query(
        "INSERT INTO notebook_note_crdt (note_id, state, state_vector) VALUES ($1, 'seeding', $2)",
    )
    .bind(&note_id)
    .bind(&[1u8, 2, 3][..])
    .execute(&pool)
    .await
    .unwrap();

    let err = notebook_crdt::append_updates(&pool, &user_id, &note_id, &[vec![1, 2, 3]]).await;
    assert!(err.is_err(), "appending to a seeding note must be rejected");
    assert_eq!(update_row_count(&pool, &note_id).await, 0);
}

#[tokio::test]
async fn updates_since_returns_only_newer() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note_id = make_note(&pool, &user_id, account_id, SEED_DOC).await;
    crdt::mark_crdt(&pool, &note_id, &[9]).await.unwrap();

    notebook_crdt::append_updates(&pool, &user_id, &note_id, &[vec![1], vec![2], vec![3]])
        .await
        .unwrap();

    let all = notebook_crdt::updates_since(&pool, &user_id, &note_id, 0)
        .await
        .unwrap();
    assert_eq!(all.len(), 3);

    let first_seq = all[0].0;
    let newer = notebook_crdt::updates_since(&pool, &user_id, &note_id, first_seq)
        .await
        .unwrap();
    assert_eq!(
        newer.iter().map(|(_, u)| u.clone()).collect::<Vec<_>>(),
        vec![vec![2u8], vec![3u8]],
        "since the first seq, only the two later updates come back"
    );
}

/// The test that matters: a blob with `0x00` and invalid UTF-8 must come back
/// byte-identical after a full Postgres `bytea` round trip.
#[tokio::test]
async fn update_bytes_survive_postgres_round_trip() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note_id = make_note(&pool, &user_id, account_id, SEED_DOC).await;
    crdt::mark_crdt(&pool, &note_id, &[7]).await.unwrap();

    let blob: Vec<u8> = vec![0x00, 0xff, 0xfe, 0x41, 0x00, 0x80, 0x7f];
    notebook_crdt::append_updates(&pool, &user_id, &note_id, std::slice::from_ref(&blob))
        .await
        .unwrap();

    let rows = notebook_crdt::updates_since(&pool, &user_id, &note_id, 0)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].1, blob,
        "update bytes must be byte-identical after a Postgres round trip"
    );
}

/// M2 sweeper. A note left in `seeding` past the staleness window (a process that
/// died mid-seed) is re-driven to `crdt` with exactly ONE update row — re-seeding
/// must not duplicate content — while a fresh seed, a crdt note, and a legacy note
/// are all left untouched, and the returned count reflects only what was recovered.
///
/// This is the only test that both creates stale seeding rows and calls the global
/// sweep, so its exact count is deterministic despite the shared, non-reset,
/// parallel test DB (every other test creates only fresh `now()` seeds).
#[tokio::test]
async fn sweeper_recovers_only_stale_seeding_notes() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;

    // 1. Stale seed: crdt row claimed an hour ago, no update row (the process died
    //    in step 2 before the flip-to-crdt tx committed).
    let stale = make_legacy_note(&pool, &user_id, account_id.clone()).await;
    sqlx::query(
        "INSERT INTO notebook_note_crdt (note_id, state, state_vector, crdt_seeded_at)
         VALUES ($1, 'seeding', ''::bytea, now() - interval '1 hour')",
    )
    .bind(&stale)
    .execute(&pool)
    .await
    .unwrap();

    // 2. Fresh seed: claimed just now, still legitimately in flight.
    let fresh = make_legacy_note(&pool, &user_id, account_id.clone()).await;
    sqlx::query(
        "INSERT INTO notebook_note_crdt (note_id, state, state_vector) VALUES ($1, 'seeding', ''::bytea)",
    )
    .bind(&fresh)
    .execute(&pool)
    .await
    .unwrap();

    // 3. Already crdt.
    let done = make_note(&pool, &user_id, account_id.clone(), SEED_DOC).await;
    crdt::seed_note(&pool, &done).await.unwrap();
    assert_eq!(update_row_count(&pool, &done).await, 1);

    // 4. Legacy (no crdt row).
    let legacy = make_legacy_note(&pool, &user_id, account_id).await;

    let recovered = crdt::sweep_stale_seeding(&pool).await.unwrap();
    assert_eq!(recovered, 1, "only the one stale seeding note is recovered");

    assert_eq!(
        crdt::note_state(&pool, &stale).await.unwrap(),
        crdt::NoteState::Crdt,
        "the stale seed must be re-driven to crdt"
    );
    assert_eq!(
        update_row_count(&pool, &stale).await,
        1,
        "re-seeding must land exactly one update row, never a duplicate"
    );

    assert_eq!(
        crdt::note_state(&pool, &fresh).await.unwrap(),
        crdt::NoteState::Seeding,
        "a not-yet-stale seed must be left alone"
    );
    assert_eq!(
        crdt::note_state(&pool, &done).await.unwrap(),
        crdt::NoteState::Crdt
    );
    assert_eq!(
        update_row_count(&pool, &done).await,
        1,
        "an already-crdt note must not be re-seeded"
    );
    assert_eq!(
        crdt::note_state(&pool, &legacy).await.unwrap(),
        crdt::NoteState::Legacy,
        "a legacy note (no row) must be left alone"
    );
}

#[tokio::test]
async fn another_user_cannot_read_or_append() {
    let pool = migrated_pool().await;
    let (owner, account_id) = seed_user_account(&pool).await;
    let (intruder, _) = seed_user_account(&pool).await;
    let note_id = make_note(&pool, &owner, account_id, SEED_DOC).await;
    crdt::mark_crdt(&pool, &note_id, &[1]).await.unwrap();
    notebook_crdt::append_updates(&pool, &owner, &note_id, &[vec![1, 2, 3]])
        .await
        .unwrap();

    assert!(
        notebook_crdt::append_updates(&pool, &intruder, &note_id, &[vec![9]])
            .await
            .is_err(),
        "a different user must not append to the note"
    );
    assert!(
        notebook_crdt::updates_since(&pool, &intruder, &note_id, 0)
            .await
            .is_err(),
        "a different user must not read the note's updates"
    );
    assert_eq!(
        update_row_count(&pool, &note_id).await,
        1,
        "the intruder's rejected append must not have written a row"
    );
}

/// The bug this guards: a note created through the desktop's sync push is seeded but
/// never reindexed, so nothing ever calls `refresh_projection` and the note reads
/// "Untitled" with no preview on every client, forever — even though its body syncs.
///
/// Asserts the sweep's *selection*, then refreshes only this note. The sweep itself
/// is database-wide, and other tests in this binary run concurrently and depend on
/// their own notes staying stale — running it here would corrupt them.
#[tokio::test]
async fn a_seeded_note_no_reindex_touched_is_swept_up_for_projection() {
    let pool = migrated_pool().await;
    let (user_id, account_id) = seed_user_account(&pool).await;
    let note_id = make_note(&pool, &user_id, account_id, TITLE_DOC).await;

    crdt::seed_note(&pool, &note_id).await.unwrap();

    // Exactly the desktop-created state: seeded, one update row, projection never run.
    // `create_notebook_note` derived the title on insert, so blank it to model a note
    // whose body only ever arrived as CRDT updates.
    sqlx::query("UPDATE notebook_notes SET title = 'Untitled', document_json = $2 WHERE id = $1")
        .bind(&note_id)
        .bind(r#"{"root":{"children":[]}}"#)
        .execute(&pool)
        .await
        .unwrap();

    let stale = crdt::stale_projection_note_ids(&pool).await.unwrap();
    assert!(
        stale.contains(&note_id),
        "the sweep must select a seeded, never-projected note"
    );

    crdt::refresh_projection(&pool, &note_id).await.unwrap();

    let (title, document_json): (String, String) =
        sqlx::query_as("SELECT title, document_json FROM notebook_notes WHERE id = $1")
            .bind(&note_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(title, "Refreshed Title", "the title must be re-derived");
    assert!(
        document_json.contains("bodymarker"),
        "the body must be re-projected, got {document_json}"
    );

    // Idempotent: a fresh note must not be re-projected on every 60s tick.
    let stale = crdt::stale_projection_note_ids(&pool).await.unwrap();
    assert!(
        !stale.contains(&note_id),
        "a fresh projection must not be swept again"
    );
}
