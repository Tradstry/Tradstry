pub mod derive;
pub mod hlc;
pub mod media;
pub mod merge;
pub mod protocol;

use std::sync::Mutex;
use std::time::Duration;

use base64::Engine as _;
use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, Emitter, Listener, Manager};

use crate::db::notebook::updates;
use crate::sync::hlc::Hlc;
use crate::sync::merge::{merge_note, Merged, NoteRow};
use crate::sync::protocol::{OutboxRow, PullResult, WireFolder, WireNote};
use crate::AppState;

// Tuning lives in code, never in .env.
const SYNC_INTERVAL: Duration = Duration::from_secs(60);
const FLUSH_ON_QUIT_TIMEOUT: Duration = Duration::from_secs(3);
const PUSH_BATCH_MAX: usize = 100;

/// Flip to true once the local store has been exercised. Not an env var.
const SYNC_ENABLED: bool = true;

#[derive(Debug, Default)]
pub struct SyncReport {
    pub pushed: usize,
    pub pulled_notes: usize,
    pub pulled_folders: usize,
    pub pulled_updates: usize,
    /// Notes that gained update blobs this cycle. An open editor is told about them
    /// so it can apply them live instead of waiting to be reopened.
    pub updated_notes: Vec<String>,
}

/// The network half of sync. Abstracted so the loop can be driven by a fake in
/// tests without a server; the real impl is a thin shim over `protocol`.
pub trait Transport: Sync {
    fn push(
        &self,
        client_id: &str,
        account_id: &str,
        mutations: &[OutboxRow],
    ) -> impl std::future::Future<Output = Result<i64, String>> + Send;

    fn pull(
        &self,
        client_id: &str,
        account_id: &str,
        cookie: Option<&str>,
    ) -> impl std::future::Future<Output = Result<PullResult, String>> + Send;

    fn pull_updates(
        &self,
        account_id: &str,
        since_seq: i64,
    ) -> impl std::future::Future<Output = Result<Vec<protocol::RemoteUpdate>, String>> + Send;

    fn pull_playbook(
        &self,
        client_id: &str,
        cookie: Option<&str>,
    ) -> impl std::future::Future<Output = Result<protocol::PlaybookPullResult, String>> + Send;

    fn pull_journal(
        &self,
        client_id: &str,
        account_id: &str,
        cookie: Option<&str>,
    ) -> impl std::future::Future<Output = Result<protocol::JournalPullResult, String>> + Send;

    fn pull_tags_sync(
        &self,
        client_id: &str,
        cookie: Option<&str>,
    ) -> impl std::future::Future<Output = Result<protocol::TagsPullResult, String>> + Send;

    fn pull_principle(
        &self,
        client_id: &str,
        account_id: &str,
        cookie: Option<&str>,
    ) -> impl std::future::Future<Output = Result<protocol::PrinciplePullResult, String>> + Send;

    fn pull_accounts(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<protocol::WireAccount>, String>> + Send;

    fn pull_calculator(
        &self,
        client_id: &str,
        cookie: Option<&str>,
    ) -> impl std::future::Future<Output = Result<protocol::CalculatorPullResult, String>> + Send;
}

struct NetTransport;

impl Transport for NetTransport {
    async fn push(
        &self,
        client_id: &str,
        account_id: &str,
        mutations: &[OutboxRow],
    ) -> Result<i64, String> {
        protocol::push(client_id, account_id, mutations).await
    }

    async fn pull(
        &self,
        client_id: &str,
        account_id: &str,
        cookie: Option<&str>,
    ) -> Result<PullResult, String> {
        protocol::pull(client_id, account_id, cookie).await
    }

    async fn pull_updates(
        &self,
        account_id: &str,
        since_seq: i64,
    ) -> Result<Vec<protocol::RemoteUpdate>, String> {
        protocol::pull_updates(account_id, since_seq).await
    }

    async fn pull_playbook(
        &self,
        client_id: &str,
        cookie: Option<&str>,
    ) -> Result<protocol::PlaybookPullResult, String> {
        protocol::pull_playbook(client_id, cookie).await
    }

    async fn pull_journal(
        &self,
        client_id: &str,
        account_id: &str,
        cookie: Option<&str>,
    ) -> Result<protocol::JournalPullResult, String> {
        protocol::pull_journal(client_id, account_id, cookie).await
    }

    async fn pull_tags_sync(
        &self,
        client_id: &str,
        cookie: Option<&str>,
    ) -> Result<protocol::TagsPullResult, String> {
        protocol::pull_tags_sync(client_id, cookie).await
    }

    async fn pull_principle(
        &self,
        client_id: &str,
        account_id: &str,
        cookie: Option<&str>,
    ) -> Result<protocol::PrinciplePullResult, String> {
        protocol::pull_principle(client_id, account_id, cookie).await
    }

    async fn pull_accounts(&self) -> Result<Vec<protocol::WireAccount>, String> {
        protocol::pull_accounts().await
    }

    async fn pull_calculator(
        &self,
        client_id: &str,
        cookie: Option<&str>,
    ) -> Result<protocol::CalculatorPullResult, String> {
        protocol::pull_calculator(client_id, cookie).await
    }
}

/// One sync cycle for a single account. Never holds a lock across an `.await`:
/// the network calls happen with every guard already dropped, so the UI thread
/// and other commands are never blocked on the wire.
pub async fn sync_once<T: Transport>(
    db: &Mutex<Connection>,
    hlc: &Mutex<Hlc>,
    account_id: &str,
    transport: &T,
) -> Result<SyncReport, String> {
    let mut report = SyncReport::default();

    // HAZARD: push the outbox empty BEFORE pulling. Pulling with un-pushed local
    // edits rebases them against a server that lacks them, so the merge sees our
    // own work as a conflict and forks it. A push error returns here, skipping
    // the pull entirely; the interval retries next tick (delivery is at-least-once).
    flush_outbox(db, transport, account_id, &mut report).await?;

    // Media bytes are an independent channel from the note CRDT: a note can
    // reference a hash whose bytes have not uploaded yet without that being a
    // sync failure. A media upload error must NOT abort note push/pull, hence
    // `let _ =` + internal logging inside `flush_media`, unlike the outbox
    // drain above which uses `?`.
    let _ = media::flush_media(db, account_id).await;

    pull_account(db, hlc, transport, account_id, &mut report).await?;
    pull_updates(db, transport, account_id, &mut report).await?;

    // Journal entries are a separate, account-scoped channel with their own
    // cursor (`journal_sync`), independent of the notebook cookie above. A
    // failure here is non-fatal, like media: it must not abort the notebook
    // pull that already succeeded.
    let _ = pull_journal_account(db, hlc, transport, account_id).await;

    // Trading principles are another account-scoped channel with their own
    // cursor (`principle_sync`), independent of the journal/notebook cookies.
    // Same non-fatal treatment: violation stats are derived locally from
    // journal entries, so a failed principle pull never blocks anything else.
    let _ = pull_principle_account(db, hlc, transport, account_id).await;

    Ok(report)
}

/// The server's `seq` is a global BIGSERIAL, so a row can become visible out of
/// order: a transaction holding seq 5 may commit after seq 6 is already readable. A
/// cursor of `max(seq)` would skip seq 5 forever. Re-reading a small window behind
/// the cursor closes that gap; `pulled_seq` makes the re-read idempotent.
const REPLAY_WINDOW: i64 = 64;

/// Emitted after a sync that stored new update blobs. Payload: the note ids.
pub const NOTE_UPDATES_EVENT: &str = "notebook://updates";

/// Pulls Yjs update blobs for every note in the account and stores them locally.
///
/// Without this the desktop only ever sees a note's updates as they were when it
/// opened the note: a note authored on the web would have no local updates at all,
/// and an open note would never see a remote edit.
async fn pull_updates<T: Transport>(
    db: &Mutex<Connection>,
    transport: &T,
    account_id: &str,
    report: &mut SyncReport,
) -> Result<(), String> {
    let since = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let cursor = updates::update_cursor(&conn, account_id)?;
        (cursor - REPLAY_WINDOW).max(0)
    };

    let rows = transport.pull_updates(account_id, since).await?;
    if rows.is_empty() {
        return Ok(());
    }

    let mut conn = db.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let mut highest = 0i64;
    for row in &rows {
        highest = highest.max(row.seq);

        // Already stored — this is the deliberate re-read of the replay window.
        let fresh = tx
            .execute("INSERT OR IGNORE INTO pulled_seq (seq) VALUES (?1)", params![row.seq])
            .map_err(|e| e.to_string())?;
        if fresh == 0 {
            continue;
        }

        let blob = base64::engine::general_purpose::STANDARD
            .decode(row.update.trim())
            .map_err(|e| e.to_string())?;
        updates::insert_remote_update(&tx, &row.note_id, &blob)?;

        report.pulled_updates += 1;
        if !report.updated_notes.contains(&row.note_id) {
            report.updated_notes.push(row.note_id.clone());
        }
    }

    updates::set_update_cursor(&tx, account_id, highest)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Pushes the whole outbox in `PUSH_BATCH_MAX` batches until it is empty. Returns
/// `Ok` only when fully drained; any transport failure short-circuits with `Err`.
async fn flush_outbox<T: Transport>(
    db: &Mutex<Connection>,
    transport: &T,
    account_id: &str,
    report: &mut SyncReport,
) -> Result<(), String> {
    loop {
        let (client_id, batch) = {
            let conn = db.lock().map_err(|e| e.to_string())?;
            let client_id: String = conn
                .query_row("SELECT id FROM client LIMIT 1", [], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            let batch = read_outbox(&conn, PUSH_BATCH_MAX)?;
            (client_id, batch)
        };

        if batch.is_empty() {
            return Ok(());
        }

        // HAZARD: `push(&[])` returns `Ok(0)` WITHOUT a round-trip. We only reach
        // the wire with a non-empty batch, so `last` below is always a real ack.
        let last = transport.push(&client_id, account_id, &batch).await?;

        let removed = {
            let conn = db.lock().map_err(|e| e.to_string())?;
            let removed = conn
                .execute("DELETE FROM outbox WHERE mutation_id <= ?1", params![last])
                .map_err(|e| e.to_string())?;
            // The watermark only ever moves forward; a stale ack must not rewind it
            // and re-send every mutation forever.
            conn.execute(
                "INSERT INTO sync_meta (account_id, last_mutation_id) VALUES (?1, ?2)
                 ON CONFLICT(account_id)
                 DO UPDATE SET last_mutation_id = MAX(last_mutation_id, excluded.last_mutation_id)",
                params![account_id, last],
            )
            .map_err(|e| e.to_string())?;
            removed
        };
        report.pushed += removed;

        // Loop until the outbox is genuinely empty (top-of-loop check). A partial
        // server ack leaves rows behind; re-reading and re-pushing is the only way
        // to guarantee the pull that follows never sees un-pushed local edits.
        if removed == 0 {
            return Err(format!(
                "push acked {last} but drained no outbox rows; aborting to avoid a spin"
            ));
        }
    }
}

async fn pull_account<T: Transport>(
    db: &Mutex<Connection>,
    hlc: &Mutex<Hlc>,
    transport: &T,
    account_id: &str,
    report: &mut SyncReport,
) -> Result<(), String> {
    let (client_id, cookie): (String, Option<String>) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let client_id: String = conn
            .query_row("SELECT id FROM client LIMIT 1", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let cookie = conn
            .query_row(
                "SELECT cookie FROM sync_meta WHERE account_id = ?1",
                params![account_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        (client_id, cookie)
    };

    let result = transport
        .pull(&client_id, account_id, cookie.as_deref())
        .await?;

    let mut conn = db.lock().map_err(|e| e.to_string())?;
    let mut clock = hlc.lock().map_err(|e| e.to_string())?;

    for wf in &result.folders {
        apply_folder(&conn, &mut clock, account_id, wf)?;
        report.pulled_folders += 1;
    }
    for wn in &result.notes {
        apply_note(&mut conn, &mut clock, account_id, wn)?;
        report.pulled_notes += 1;
    }

    // The cookie is opaque: store it verbatim, never parse or compare it.
    conn.execute(
        "INSERT INTO sync_meta (account_id, cookie, last_sync_at)
         VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(account_id)
         DO UPDATE SET cookie = excluded.cookie, last_sync_at = excluded.last_sync_at",
        params![account_id, result.cookie],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn apply_note(
    conn: &mut Connection,
    hlc: &mut Hlc,
    account_id: &str,
    wn: &WireNote,
) -> Result<(), String> {
    // Observe BEFORE merging so a peer's fast clock can't make our later writes
    // look older than theirs.
    hlc.observe(&wn.hlc);

    let server = wire_to_noterow(wn);

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    match read_local_note(&tx, &wn.id)? {
        // Brand-new server row: nothing local to conflict with.
        None => write_note_row(&tx, account_id, &server)?,
        Some(local) => match merge_note(&local, &server) {
            Merged::Unchanged => {}
            Merged::Take(row) => write_note_row(&tx, account_id, &row)?,
            Merged::Tombstone => {
                tx.execute(
                    "UPDATE notes
                     SET deleted_at = COALESCE(deleted_at, ?1, datetime('now')), sync_state = 'synced'
                     WHERE id = ?2",
                    params![wn.deleted_at, wn.id],
                )
                .map_err(|e| e.to_string())?;
            }
        },
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Playbooks are user-scoped, so they pull on their own cursor (`playbook_sync`),
/// once per full sync cycle rather than per account. Push rides the shared outbox.
async fn pull_playbooks<T: Transport>(
    db: &Mutex<Connection>,
    hlc: &Mutex<Hlc>,
    transport: &T,
) -> Result<usize, String> {
    let (client_id, cookie): (String, Option<String>) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let client_id: String = conn
            .query_row("SELECT id FROM client LIMIT 1", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let cookie = conn
            .query_row(
                "SELECT cookie FROM playbook_sync WHERE id = 1",
                [],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        (client_id, cookie)
    };

    let result = transport.pull_playbook(&client_id, cookie.as_deref()).await?;

    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut clock = hlc.lock().map_err(|e| e.to_string())?;
    for wp in &result.playbooks {
        apply_playbook(&conn, &mut clock, wp)?;
    }
    conn.execute(
        "INSERT INTO playbook_sync (id, cookie, last_sync_at) VALUES (1, ?1, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET cookie = excluded.cookie, last_sync_at = excluded.last_sync_at",
        params![result.cookie],
    )
    .map_err(|e| e.to_string())?;
    Ok(result.playbooks.len())
}

/// Whole-row LWW against the single server `hlc` stamp; delete always wins. The
/// playbook form saves every field at once, so per-field granularity would buy
/// nothing.
fn apply_playbook(
    conn: &Connection,
    hlc: &mut Hlc,
    wp: &protocol::WirePlaybook,
) -> Result<(), String> {
    hlc.observe(&wp.hlc);

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM playbooks WHERE id = ?1)",
            params![wp.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute(
            "UPDATE playbooks SET
               name                  = CASE WHEN ?2 > hlc THEN ?3 ELSE name END,
               edge_name             = CASE WHEN ?2 > hlc THEN ?4 ELSE edge_name END,
               entry_rules           = CASE WHEN ?2 > hlc THEN ?5 ELSE entry_rules END,
               exit_rules            = CASE WHEN ?2 > hlc THEN ?6 ELSE exit_rules END,
               position_sizing_rules = CASE WHEN ?2 > hlc THEN ?7 ELSE position_sizing_rules END,
               additional_rules      = CASE WHEN ?2 > hlc THEN ?8 ELSE additional_rules END,
               hlc                   = CASE WHEN ?2 > hlc THEN ?2 ELSE hlc END,
               deleted_at            = COALESCE(deleted_at, ?9),
               sync_state            = 'synced'
             WHERE id = ?1",
            params![
                wp.id,
                wp.hlc,
                wp.name,
                wp.edge_name,
                wp.entry_rules,
                wp.exit_rules,
                wp.position_sizing_rules,
                wp.additional_rules,
                wp.deleted_at
            ],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO playbooks
             (id, name, edge_name, entry_rules, exit_rules, position_sizing_rules, additional_rules, hlc, deleted_at, sync_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'synced')",
            params![
                wp.id,
                wp.name,
                wp.edge_name,
                wp.entry_rules,
                wp.exit_rules,
                wp.position_sizing_rules,
                wp.additional_rules,
                wp.hlc,
                wp.deleted_at
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Journal entries are account-scoped (unlike playbooks), so they get their own
/// cursor (`journal_sync`) keyed by `account_id` rather than a single-row table.
/// Push rides the shared outbox; this function only drives the pull side.
async fn pull_journal_account<T: Transport>(
    db: &Mutex<Connection>,
    hlc: &Mutex<Hlc>,
    transport: &T,
    account_id: &str,
) -> Result<usize, String> {
    let (client_id, cookie): (String, Option<String>) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let client_id: String = conn
            .query_row("SELECT id FROM client LIMIT 1", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let cookie = conn
            .query_row(
                "SELECT cookie FROM journal_sync WHERE account_id = ?1",
                params![account_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        (client_id, cookie)
    };

    let result = transport
        .pull_journal(&client_id, account_id, cookie.as_deref())
        .await?;

    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut clock = hlc.lock().map_err(|e| e.to_string())?;
    for w in &result.entries {
        apply_journal(&conn, &mut clock, account_id, w)?;
    }
    conn.execute(
        "INSERT INTO journal_sync (account_id, cookie, last_sync_at) VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(account_id)
         DO UPDATE SET cookie = excluded.cookie, last_sync_at = excluded.last_sync_at",
        params![account_id, result.cookie],
    )
    .map_err(|e| e.to_string())?;
    Ok(result.entries.len())
}

/// Trading principles are account-scoped (like journal entries), so they get
/// their own cursor (`principle_sync`) keyed by `account_id`. Push rides the
/// shared outbox; this function only drives the pull side. Violation stats
/// are never part of the wire payload — they are derived on-device at read
/// time from the local `journal_entries` cache.
async fn pull_principle_account<T: Transport>(
    db: &Mutex<Connection>,
    hlc: &Mutex<Hlc>,
    transport: &T,
    account_id: &str,
) -> Result<usize, String> {
    let (client_id, cookie): (String, Option<String>) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let client_id: String = conn
            .query_row("SELECT id FROM client LIMIT 1", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let cookie = conn
            .query_row(
                "SELECT cookie FROM principle_sync WHERE account_id = ?1",
                params![account_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        (client_id, cookie)
    };

    let result = transport
        .pull_principle(&client_id, account_id, cookie.as_deref())
        .await?;

    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut clock = hlc.lock().map_err(|e| e.to_string())?;
    for w in &result.principles {
        apply_principle(&conn, &mut clock, account_id, w)?;
    }
    conn.execute(
        "INSERT INTO principle_sync (account_id, cookie, last_sync_at) VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(account_id)
         DO UPDATE SET cookie = excluded.cookie, last_sync_at = excluded.last_sync_at",
        params![account_id, result.cookie],
    )
    .map_err(|e| e.to_string())?;
    Ok(result.principles.len())
}

/// Whole-row LWW against the single server `hlc` stamp; delete always wins.
/// `account_id` is a separate parameter (like `apply_journal`) even though the
/// wire type also carries `accountId` — the pull is already account-scoped by
/// the query argument, so the parameter is the source of truth on insert.
fn apply_principle(
    conn: &Connection,
    hlc: &mut Hlc,
    account_id: &str,
    w: &protocol::WirePrinciple,
) -> Result<(), String> {
    hlc.observe(&w.hlc);

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM trading_principles WHERE id = ?1)",
            params![w.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute(
            "UPDATE trading_principles SET
               playbook_id      = CASE WHEN ?2 > hlc THEN ?3  ELSE playbook_id END,
               evidence_note_id = CASE WHEN ?2 > hlc THEN ?4  ELSE evidence_note_id END,
               title            = CASE WHEN ?2 > hlc THEN ?5  ELSE title END,
               the_rule         = CASE WHEN ?2 > hlc THEN ?6  ELSE the_rule END,
               why              = CASE WHEN ?2 > hlc THEN ?7  ELSE why END,
               intervention     = CASE WHEN ?2 > hlc THEN ?8  ELSE intervention END,
               priority         = CASE WHEN ?2 > hlc THEN ?9  ELSE priority END,
               is_active        = CASE WHEN ?2 > hlc THEN ?10 ELSE is_active END,
               hlc              = CASE WHEN ?2 > hlc THEN ?2  ELSE hlc END,
               deleted_at       = COALESCE(deleted_at, ?11),
               sync_state       = 'synced'
             WHERE id = ?1",
            params![
                w.id,
                w.hlc,
                w.playbook_id,
                w.evidence_note_id,
                w.title,
                w.the_rule,
                w.why,
                w.intervention,
                w.priority,
                w.is_active,
                w.deleted_at,
            ],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO trading_principles
             (id, account_id, playbook_id, evidence_note_id, title, the_rule, why, intervention,
              priority, is_active, hlc, deleted_at, sync_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'synced')",
            params![
                w.id,
                account_id,
                w.playbook_id,
                w.evidence_note_id,
                w.title,
                w.the_rule,
                w.why,
                w.intervention,
                w.priority,
                w.is_active,
                w.hlc,
                w.deleted_at,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Tags/categories are user-scoped like playbooks, so they pull on their own
/// cursor (`tag_sync`), once per full sync cycle rather than per account. Push
/// rides the shared outbox (local CRUD lives in `db::tags`); this replaces the
/// old wholesale `refresh_tags_cache` with a delta pull + LWW apply.
async fn pull_tags_sync_apply<T: Transport>(
    db: &Mutex<Connection>,
    hlc: &Mutex<Hlc>,
    transport: &T,
) -> Result<usize, String> {
    let (client_id, cookie): (String, Option<String>) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let client_id: String = conn
            .query_row("SELECT id FROM client LIMIT 1", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let cookie = conn
            .query_row("SELECT cookie FROM tag_sync WHERE id = 1", [], |r| {
                r.get::<_, Option<String>>(0)
            })
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        (client_id, cookie)
    };

    let result = transport.pull_tags_sync(&client_id, cookie.as_deref()).await?;

    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut clock = hlc.lock().map_err(|e| e.to_string())?;
    for wc in &result.categories {
        apply_tag_category(&conn, &mut clock, wc)?;
    }
    for wt in &result.tags {
        apply_tag(&conn, &mut clock, wt)?;
    }
    conn.execute(
        "INSERT INTO tag_sync (id, cookie, last_sync_at) VALUES (1, ?1, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET cookie = excluded.cookie, last_sync_at = excluded.last_sync_at",
        params![result.cookie],
    )
    .map_err(|e| e.to_string())?;
    Ok(result.categories.len() + result.tags.len())
}

/// Whole-row LWW against the single server `hlc` stamp; delete always wins.
/// Mirrors `apply_playbook`.
fn apply_tag_category(
    conn: &Connection,
    hlc: &mut Hlc,
    wc: &protocol::WireTagCategoryDelta,
) -> Result<(), String> {
    hlc.observe(&wc.hlc);

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tag_categories_cache WHERE id = ?1)",
            params![wc.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute(
            "UPDATE tag_categories_cache SET
               name       = CASE WHEN ?2 > hlc THEN ?3 ELSE name END,
               role       = CASE WHEN ?2 > hlc THEN ?4 ELSE role END,
               color      = CASE WHEN ?2 > hlc THEN ?5 ELSE color END,
               sort_order = CASE WHEN ?2 > hlc THEN ?6 ELSE sort_order END,
               hlc        = CASE WHEN ?2 > hlc THEN ?2 ELSE hlc END,
               deleted_at = COALESCE(deleted_at, ?7),
               sync_state = 'synced'
             WHERE id = ?1",
            params![wc.id, wc.hlc, wc.name, wc.role, wc.color, wc.sort_order, wc.deleted_at],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO tag_categories_cache
             (id, name, role, color, sort_order, hlc, deleted_at, sync_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'synced')",
            params![wc.id, wc.name, wc.role, wc.color, wc.sort_order, wc.hlc, wc.deleted_at],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Whole-row LWW against the single server `hlc` stamp; delete always wins.
/// Mirrors `apply_playbook`.
fn apply_tag(conn: &Connection, hlc: &mut Hlc, wt: &protocol::WireTagDelta) -> Result<(), String> {
    hlc.observe(&wt.hlc);

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tags_cache WHERE id = ?1)",
            params![wt.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute(
            "UPDATE tags_cache SET
               name        = CASE WHEN ?2 > hlc THEN ?3 ELSE name END,
               color       = CASE WHEN ?2 > hlc THEN ?4 ELSE color END,
               category_id = CASE WHEN ?2 > hlc THEN ?5 ELSE category_id END,
               hlc         = CASE WHEN ?2 > hlc THEN ?2 ELSE hlc END,
               deleted_at  = COALESCE(deleted_at, ?6),
               sync_state  = 'synced'
             WHERE id = ?1",
            params![wt.id, wt.hlc, wt.name, wt.color, wt.category_id, wt.deleted_at],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO tags_cache (id, name, color, category_id, hlc, deleted_at, sync_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'synced')",
            params![wt.id, wt.name, wt.color, wt.category_id, wt.hlc, wt.deleted_at],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Position-calculator rules/plans/history are user-scoped like tags: pull
/// deltas for all three once per cycle on one shared cursor (`calculator_sync`),
/// not per account. Push rides the shared outbox (local CRUD lives in
/// `db::calculator`).
async fn pull_calculator_sync<T: Transport>(
    db: &Mutex<Connection>,
    hlc: &Mutex<Hlc>,
    transport: &T,
) -> Result<usize, String> {
    let (client_id, cookie): (String, Option<String>) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let client_id: String = conn
            .query_row("SELECT id FROM client LIMIT 1", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let cookie = conn
            .query_row("SELECT cookie FROM calculator_sync WHERE id = 1", [], |r| {
                r.get::<_, Option<String>>(0)
            })
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
        (client_id, cookie)
    };

    let result = transport.pull_calculator(&client_id, cookie.as_deref()).await?;

    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut clock = hlc.lock().map_err(|e| e.to_string())?;
    for wr in &result.rules {
        apply_rule(&conn, &mut clock, wr)?;
    }
    for wp in &result.plans {
        apply_plan(&conn, &mut clock, wp)?;
    }
    for wh in &result.history {
        apply_history(&conn, &mut clock, wh)?;
    }
    conn.execute(
        "INSERT INTO calculator_sync (id, cookie, last_sync_at) VALUES (1, ?1, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET cookie = excluded.cookie, last_sync_at = excluded.last_sync_at",
        params![result.cookie],
    )
    .map_err(|e| e.to_string())?;
    Ok(result.rules.len() + result.plans.len() + result.history.len())
}

/// Whole-row LWW against the single server `hlc` stamp; delete always wins.
/// Keyed by `account_id` (not `id`) to match the local PK: one rule row per
/// account, matching the server's `UNIQUE(user_id, account_id)`.
fn apply_rule(conn: &Connection, hlc: &mut Hlc, wr: &protocol::WireRule) -> Result<(), String> {
    hlc.observe(&wr.hlc);

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM calc_rules WHERE account_id = ?1)",
            params![wr.account_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute(
            "UPDATE calc_rules SET
               id                = CASE WHEN ?2 > hlc THEN ?3 ELSE id END,
               account_balance   = CASE WHEN ?2 > hlc THEN ?4 ELSE account_balance END,
               account_risk      = CASE WHEN ?2 > hlc THEN ?5 ELSE account_risk END,
               max_stop_loss_pct = CASE WHEN ?2 > hlc THEN ?6 ELSE max_stop_loss_pct END,
               hlc               = CASE WHEN ?2 > hlc THEN ?2 ELSE hlc END,
               deleted_at        = COALESCE(deleted_at, ?7),
               sync_state        = 'synced'
             WHERE account_id = ?1",
            params![
                wr.account_id,
                wr.hlc,
                wr.id,
                wr.account_balance,
                wr.account_risk,
                wr.max_stop_loss_pct,
                wr.deleted_at
            ],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO calc_rules
             (account_id, id, account_balance, account_risk, max_stop_loss_pct, hlc, deleted_at, sync_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'synced')",
            params![
                wr.account_id,
                wr.id,
                wr.account_balance,
                wr.account_risk,
                wr.max_stop_loss_pct,
                wr.hlc,
                wr.deleted_at
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Whole-row LWW against the single server `hlc` stamp; delete always wins.
/// `tranches_json` travels as an opaque string, same as the local store.
fn apply_plan(conn: &Connection, hlc: &mut Hlc, wp: &protocol::WirePlan) -> Result<(), String> {
    hlc.observe(&wp.hlc);

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM calc_plans WHERE id = ?1)",
            params![wp.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute(
            "UPDATE calc_plans SET
               symbol          = CASE WHEN ?2 > hlc THEN ?3  ELSE symbol END,
               position_type   = CASE WHEN ?2 > hlc THEN ?4  ELSE position_type END,
               entry_price     = CASE WHEN ?2 > hlc THEN ?5  ELSE entry_price END,
               stop_loss       = CASE WHEN ?2 > hlc THEN ?6  ELSE stop_loss END,
               account_balance = CASE WHEN ?2 > hlc THEN ?7  ELSE account_balance END,
               account_risk    = CASE WHEN ?2 > hlc THEN ?8  ELSE account_risk END,
               total_shares    = CASE WHEN ?2 > hlc THEN ?9  ELSE total_shares END,
               position_value  = CASE WHEN ?2 > hlc THEN ?10 ELSE position_value END,
               status          = CASE WHEN ?2 > hlc THEN ?11 ELSE status END,
               tranches_json   = CASE WHEN ?2 > hlc THEN ?12 ELSE tranches_json END,
               notes           = CASE WHEN ?2 > hlc THEN ?13 ELSE notes END,
               hlc             = CASE WHEN ?2 > hlc THEN ?2  ELSE hlc END,
               deleted_at      = COALESCE(deleted_at, ?14),
               sync_state      = 'synced'
             WHERE id = ?1",
            params![
                wp.id,
                wp.hlc,
                wp.symbol,
                wp.position_type,
                wp.entry_price,
                wp.stop_loss,
                wp.account_balance,
                wp.account_risk,
                wp.total_shares,
                wp.position_value,
                wp.status,
                wp.tranches_json,
                wp.notes,
                wp.deleted_at,
            ],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO calc_plans
             (id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk,
              total_shares, position_value, status, tranches_json, notes, hlc, deleted_at, sync_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 'synced')",
            params![
                wp.id,
                wp.symbol,
                wp.position_type,
                wp.entry_price,
                wp.stop_loss,
                wp.account_balance,
                wp.account_risk,
                wp.total_shares,
                wp.position_value,
                wp.status,
                wp.tranches_json,
                wp.notes,
                wp.hlc,
                wp.deleted_at,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Whole-row LWW against the single server `hlc` stamp; delete always wins.
/// History has no update mutation, but still gets the same pull-side
/// insert/merge/tombstone treatment as every other synced entity.
fn apply_history(conn: &Connection, hlc: &mut Hlc, wh: &protocol::WireHistory) -> Result<(), String> {
    hlc.observe(&wh.hlc);

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM calc_history WHERE id = ?1)",
            params![wh.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute(
            "UPDATE calc_history SET
               symbol          = CASE WHEN ?2 > hlc THEN ?3  ELSE symbol END,
               position_type   = CASE WHEN ?2 > hlc THEN ?4  ELSE position_type END,
               entry_price     = CASE WHEN ?2 > hlc THEN ?5  ELSE entry_price END,
               stop_loss       = CASE WHEN ?2 > hlc THEN ?6  ELSE stop_loss END,
               account_balance = CASE WHEN ?2 > hlc THEN ?7  ELSE account_balance END,
               account_risk    = CASE WHEN ?2 > hlc THEN ?8  ELSE account_risk END,
               shares          = CASE WHEN ?2 > hlc THEN ?9  ELSE shares END,
               position_value  = CASE WHEN ?2 > hlc THEN ?10 ELSE position_value END,
               account_pct     = CASE WHEN ?2 > hlc THEN ?11 ELSE account_pct END,
               stop_loss_pct   = CASE WHEN ?2 > hlc THEN ?12 ELSE stop_loss_pct END,
               hlc             = CASE WHEN ?2 > hlc THEN ?2  ELSE hlc END,
               deleted_at      = COALESCE(deleted_at, ?13),
               sync_state      = 'synced'
             WHERE id = ?1",
            params![
                wh.id,
                wh.hlc,
                wh.symbol,
                wh.position_type,
                wh.entry_price,
                wh.stop_loss,
                wh.account_balance,
                wh.account_risk,
                wh.shares,
                wh.position_value,
                wh.account_pct,
                wh.stop_loss_pct,
                wh.deleted_at,
            ],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO calc_history
             (id, symbol, position_type, entry_price, stop_loss, account_balance, account_risk,
              shares, position_value, account_pct, stop_loss_pct, hlc, deleted_at, sync_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'synced')",
            params![
                wh.id,
                wh.symbol,
                wh.position_type,
                wh.entry_price,
                wh.stop_loss,
                wh.account_balance,
                wh.account_risk,
                wh.shares,
                wh.position_value,
                wh.account_pct,
                wh.stop_loss_pct,
                wh.hlc,
                wh.deleted_at,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Refreshes the pull-only `accounts_cache` so the account list and its
/// equity (`total_value`) are available offline. Never written by a local
/// mutation — account CRUD stays online.
async fn refresh_accounts_cache<T: Transport>(
    db: &Mutex<Connection>,
    transport: &T,
) -> Result<(), String> {
    let accounts = transport.pull_accounts().await?;

    let conn = db.lock().map_err(|e| e.to_string())?;
    for a in &accounts {
        conn.execute(
            "INSERT INTO accounts_cache (id, name, broker, currency, icon, total_value, risk_profile)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
               name         = excluded.name,
               broker       = excluded.broker,
               currency     = excluded.currency,
               icon         = excluded.icon,
               total_value  = excluded.total_value,
               risk_profile = excluded.risk_profile",
            params![a.id, a.name, a.broker, a.currency, a.icon, a.total_value, a.risk_profile],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Whole-row LWW against the single server `hlc` stamp; delete always wins.
/// `account_id` is a separate parameter (like `apply_folder`) because the wire
/// type has no `accountId` field — the pull is already account-scoped by the
/// query argument.
fn apply_journal(
    conn: &Connection,
    hlc: &mut Hlc,
    account_id: &str,
    w: &protocol::WireJournalEntry,
) -> Result<(), String> {
    hlc.observe(&w.hlc);

    let tag_ids = serde_json::to_string(&w.tag_ids).map_err(|e| e.to_string())?;

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM journal_entries WHERE id = ?1)",
            params![w.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute(
            "UPDATE journal_entries SET
               open_date             = CASE WHEN ?2 > hlc THEN ?3  ELSE open_date END,
               close_date            = CASE WHEN ?2 > hlc THEN ?4  ELSE close_date END,
               entry_price           = CASE WHEN ?2 > hlc THEN ?5  ELSE entry_price END,
               exit_price            = CASE WHEN ?2 > hlc THEN ?6  ELSE exit_price END,
               position_size         = CASE WHEN ?2 > hlc THEN ?7  ELSE position_size END,
               stop_loss             = CASE WHEN ?2 > hlc THEN ?8  ELSE stop_loss END,
               symbol                = CASE WHEN ?2 > hlc THEN ?9  ELSE symbol END,
               symbol_name           = CASE WHEN ?2 > hlc THEN ?10 ELSE symbol_name END,
               status                = CASE WHEN ?2 > hlc THEN ?11 ELSE status END,
               total_pl              = CASE WHEN ?2 > hlc THEN ?12 ELSE total_pl END,
               net_roi               = CASE WHEN ?2 > hlc THEN ?13 ELSE net_roi END,
               duration              = CASE WHEN ?2 > hlc THEN ?14 ELSE duration END,
               risk_reward           = CASE WHEN ?2 > hlc THEN ?15 ELSE risk_reward END,
               trade_type            = CASE WHEN ?2 > hlc THEN ?16 ELSE trade_type END,
               playbook_id           = CASE WHEN ?2 > hlc THEN ?17 ELSE playbook_id END,
               notes                 = CASE WHEN ?2 > hlc THEN ?18 ELSE notes END,
               broke_30min_rule      = CASE WHEN ?2 > hlc THEN ?19 ELSE broke_30min_rule END,
               pre_trade_conviction  = CASE WHEN ?2 > hlc THEN ?20 ELSE pre_trade_conviction END,
               market_regime         = CASE WHEN ?2 > hlc THEN ?21 ELSE market_regime END,
               is_planned_pre_market = CASE WHEN ?2 > hlc THEN ?22 ELSE is_planned_pre_market END,
               revenge_trade         = CASE WHEN ?2 > hlc THEN ?23 ELSE revenge_trade END,
               rule_adherence_score  = CASE WHEN ?2 > hlc THEN ?24 ELSE rule_adherence_score END,
               tag_ids               = CASE WHEN ?2 > hlc THEN ?25 ELSE tag_ids END,
               hlc                   = CASE WHEN ?2 > hlc THEN ?2  ELSE hlc END,
               deleted_at            = COALESCE(deleted_at, ?26),
               sync_state            = 'synced'
             WHERE id = ?1",
            params![
                w.id,
                w.hlc,
                w.open_date,
                w.close_date,
                w.entry_price,
                w.exit_price,
                w.position_size,
                w.stop_loss,
                w.symbol,
                w.symbol_name,
                w.status,
                w.total_pl,
                w.net_roi,
                w.duration,
                w.risk_reward,
                w.trade_type,
                w.playbook_id,
                w.notes,
                w.broke_30min_rule,
                w.pre_trade_conviction,
                w.market_regime,
                w.is_planned_pre_market,
                w.revenge_trade,
                w.rule_adherence_score,
                tag_ids,
                w.deleted_at,
            ],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO journal_entries
             (id, account_id, open_date, close_date, entry_price, exit_price, position_size,
              stop_loss, symbol, symbol_name, status, total_pl, net_roi, duration, risk_reward,
              trade_type, playbook_id, notes, broke_30min_rule, pre_trade_conviction,
              market_regime, is_planned_pre_market, revenge_trade, rule_adherence_score,
              tag_ids, hlc, deleted_at, sync_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                     ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, 'synced')",
            params![
                w.id,
                account_id,
                w.open_date,
                w.close_date,
                w.entry_price,
                w.exit_price,
                w.position_size,
                w.stop_loss,
                w.symbol,
                w.symbol_name,
                w.status,
                w.total_pl,
                w.net_roi,
                w.duration,
                w.risk_reward,
                w.trade_type,
                w.playbook_id,
                w.notes,
                w.broke_30min_rule,
                w.pre_trade_conviction,
                w.market_regime,
                w.is_planned_pre_market,
                w.revenge_trade,
                w.rule_adherence_score,
                tag_ids,
                w.hlc,
                w.deleted_at,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Folders never fork; they are pure metadata. Per-field LWW against the single
/// server stamp, and delete always wins.
fn apply_folder(
    conn: &Connection,
    hlc: &mut Hlc,
    account_id: &str,
    wf: &WireFolder,
) -> Result<(), String> {
    hlc.observe(&wf.hlc);

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM folders WHERE id = ?1)",
            params![wf.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute(
            "UPDATE folders SET
               name             = CASE WHEN ?2 > hlc_name       THEN ?3 ELSE name END,
               hlc_name         = CASE WHEN ?2 > hlc_name       THEN ?2 ELSE hlc_name END,
               parent_folder_id = CASE WHEN ?2 > hlc_parent     THEN ?4 ELSE parent_folder_id END,
               hlc_parent       = CASE WHEN ?2 > hlc_parent     THEN ?2 ELSE hlc_parent END,
               sort_order       = CASE WHEN ?2 > hlc_sort_order THEN ?5 ELSE sort_order END,
               hlc_sort_order   = CASE WHEN ?2 > hlc_sort_order THEN ?2 ELSE hlc_sort_order END,
               deleted_at       = COALESCE(deleted_at, ?6),
               is_system        = ?7,
               sync_state       = 'synced'
             WHERE id = ?1",
            params![
                wf.id,
                wf.hlc,
                wf.name,
                wf.parent_folder_id,
                wf.sort_order,
                wf.deleted_at,
                wf.is_system
            ],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO folders
             (id, account_id, parent_folder_id, name, sort_order, is_system, hlc_name, hlc_parent, hlc_sort_order, deleted_at, sync_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?8, ?6, ?6, ?6, ?7, 'synced')",
            params![
                wf.id,
                account_id,
                wf.parent_folder_id,
                wf.name,
                wf.sort_order,
                wf.hlc,
                wf.deleted_at,
                wf.is_system
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Writes the canonical metadata row from a pulled/merged note.
fn write_note_row(conn: &Connection, account_id: &str, r: &NoteRow) -> Result<(), String> {
    let trade_ids = serde_json::to_string(&r.trade_ids).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO notes
         (id, account_id, folder_id, title, document_json, sort_order,
          trade_ids, hlc_folder_id, hlc_sort_order, hlc_trade_ids, body_hlc, deleted_at, sync_state)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'synced')
         ON CONFLICT(id) DO UPDATE SET
           folder_id          = excluded.folder_id,
           title              = excluded.title,
           document_json      = excluded.document_json,
           sort_order         = excluded.sort_order,
           trade_ids          = excluded.trade_ids,
           hlc_folder_id      = excluded.hlc_folder_id,
           hlc_sort_order     = excluded.hlc_sort_order,
           hlc_trade_ids      = excluded.hlc_trade_ids,
           body_hlc           = excluded.body_hlc,
           deleted_at         = excluded.deleted_at,
           sync_state         = 'synced'",
        params![
            r.id,
            account_id,
            r.folder_id,
            r.title,
            r.document_json,
            r.sort_order,
            trade_ids,
            r.hlc_folder_id,
            r.hlc_sort_order,
            r.hlc_trade_ids,
            r.body_hlc,
            r.deleted_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn wire_to_noterow(w: &WireNote) -> NoteRow {
    // The wire carries one stamp per note; fan it out to every per-field hlc so
    // metadata LWW compares against the server's authoritative time.
    NoteRow {
        id: w.id.clone(),
        folder_id: w.folder_id.clone(),
        title: w.title.clone(),
        document_json: w.document_json.clone(),
        sort_order: w.sort_order,
        trade_ids: w.trade_ids.clone(),
        hlc_folder_id: w.hlc.clone(),
        hlc_sort_order: w.hlc.clone(),
        hlc_trade_ids: w.hlc.clone(),
        body_hlc: w.hlc.clone(),
        deleted_at: w.deleted_at.clone(),
    }
}

fn read_local_note(conn: &Connection, id: &str) -> Result<Option<NoteRow>, String> {
    conn.query_row(
        "SELECT id, folder_id, title, document_json, sort_order, trade_ids,
                hlc_folder_id, hlc_sort_order, hlc_trade_ids, body_hlc, deleted_at
         FROM notes WHERE id = ?1",
        params![id],
        |r| {
            let trade_ids: String = r.get(5)?;
            Ok(NoteRow {
                id: r.get(0)?,
                folder_id: r.get(1)?,
                title: r.get(2)?,
                document_json: r.get(3)?,
                sort_order: r.get(4)?,
                trade_ids: serde_json::from_str(&trade_ids).unwrap_or_default(),
                hlc_folder_id: r.get(6)?,
                hlc_sort_order: r.get(7)?,
                hlc_trade_ids: r.get(8)?,
                body_hlc: r.get(9)?,
                deleted_at: r.get(10)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn read_outbox(conn: &Connection, limit: usize) -> Result<Vec<OutboxRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT mutation_id, name, args, hlc FROM outbox ORDER BY mutation_id ASC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit as i64], |r| {
            Ok(OutboxRow {
                mutation_id: r.get(0)?,
                name: r.get(1)?,
                args: r.get(2)?,
                hlc: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

fn discover_accounts(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT account_id FROM notes
             UNION SELECT account_id FROM folders
             UNION SELECT account_id FROM sync_meta",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

async fn run_all_accounts(app: &AppHandle) {
    let state = app.state::<AppState>();
    let accounts = {
        let Ok(conn) = state.db.lock() else { return };
        match discover_accounts(&conn) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("notebook sync: cannot list accounts: {e}");
                return;
            }
        }
    };

    for account_id in accounts {
        match sync_once(&state.db, &state.hlc, &account_id, &NetTransport).await {
            // Tell an open editor which notes gained updates, so it applies them live
            // rather than only on reopen. The payload is note ids, not blobs: the
            // webview reads the bytes back through `note_updates`.
            Ok(report) => {
                if !report.updated_notes.is_empty() {
                    let _ = app.emit(NOTE_UPDATES_EVENT, &report.updated_notes);
                }
            }
            // A failed sync is normal (offline); surface it as a log, not an error.
            Err(e) => eprintln!("notebook sync ({account_id}): {e}"),
        }
    }

    // Playbooks are user-scoped: pull them once per cycle, not per account. Push
    // already rode the shared outbox during the per-account flushes above. A
    // failure is normal offline — log, don't abort.
    if let Err(e) = pull_playbooks(&state.db, &state.hlc, &NetTransport).await {
        eprintln!("playbook sync: {e}");
    }

    // Tags/categories are user-scoped like playbooks: pull deltas once per
    // cycle, not per account. Push already rode the shared outbox during the
    // per-account flushes above. A failure is normal offline — log, don't abort.
    if let Err(e) = pull_tags_sync_apply(&state.db, &state.hlc, &NetTransport).await {
        eprintln!("tag sync: {e}");
    }

    // Position-calculator rules/plans/history are user-scoped like tags: pull
    // deltas once per cycle, not per account. Push already rode the shared
    // outbox during the per-account flushes above. A failure is normal
    // offline — log, don't abort.
    if let Err(e) = pull_calculator_sync(&state.db, &state.hlc, &NetTransport).await {
        eprintln!("calculator sync: {e}");
    }

    // Accounts are read-only offline too: refresh the cache once per cycle,
    // best-effort (normal to fail offline).
    let _ = refresh_accounts_cache(&state.db, &NetTransport).await;
}

/// Starts the background sync: a `SYNC_INTERVAL` ticker plus a re-sync on window
/// focus. A no-op while the feature is disabled.
pub fn spawn(app: AppHandle) {
    if !SYNC_ENABLED {
        return;
    }

    let interval_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(SYNC_INTERVAL);
        loop {
            ticker.tick().await;
            run_all_accounts(&interval_app).await;
        }
    });

    let focus_app = app.clone();
    app.listen("tauri://focus", move |_| {
        let app = focus_app.clone();
        tauri::async_runtime::spawn(async move {
            run_all_accounts(&app).await;
        });
    });
}

/// Best-effort final push on quit, bounded so it can never hang the shutdown.
pub fn flush_on_quit(app: &AppHandle) {
    if !SYNC_ENABLED {
        return;
    }
    let state = app.state::<AppState>();
    let accounts = {
        let Ok(conn) = state.db.lock() else { return };
        match discover_accounts(&conn) {
            Ok(a) => a,
            Err(_) => return,
        }
    };
    let _ = tauri::async_runtime::block_on(tokio::time::timeout(FLUSH_ON_QUIT_TIMEOUT, async {
        let mut report = SyncReport::default();
        for account_id in &accounts {
            let _ = flush_outbox(&state.db, &NetTransport, account_id, &mut report).await;
        }
    }));
}

#[tauri::command]
pub async fn sync_now(app: AppHandle) -> Result<(), String> {
    if !SYNC_ENABLED {
        return Ok(());
    }
    run_all_accounts(&app).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeTransport {
        fail_push: bool,
        push_calls: AtomicUsize,
        pull_calls: AtomicUsize,
        pull_result: PullResult,
        remote_updates: Vec<crate::sync::protocol::RemoteUpdate>,
        /// Every `since_seq` the loop asked for, so a test can assert the cursor.
        since_seqs: Mutex<Vec<i64>>,
        playbook_result: protocol::PlaybookPullResult,
        journal_result: protocol::JournalPullResult,
        tags_sync_result: protocol::TagsPullResult,
        principle_result: protocol::PrinciplePullResult,
        accounts_result: Vec<protocol::WireAccount>,
        calculator_result: protocol::CalculatorPullResult,
    }

    impl FakeTransport {
        fn new(pull_result: PullResult) -> Self {
            Self {
                fail_push: false,
                push_calls: AtomicUsize::new(0),
                pull_calls: AtomicUsize::new(0),
                pull_result,
                remote_updates: Vec::new(),
                since_seqs: Mutex::new(Vec::new()),
                playbook_result: protocol::PlaybookPullResult {
                    cookie: None,
                    last_mutation_id: 0,
                    playbooks: Vec::new(),
                },
                journal_result: protocol::JournalPullResult {
                    cookie: None,
                    last_mutation_id: 0,
                    entries: Vec::new(),
                },
                tags_sync_result: protocol::TagsPullResult {
                    cookie: None,
                    last_mutation_id: 0,
                    categories: Vec::new(),
                    tags: Vec::new(),
                },
                principle_result: protocol::PrinciplePullResult {
                    cookie: None,
                    last_mutation_id: 0,
                    principles: Vec::new(),
                },
                accounts_result: Vec::new(),
                calculator_result: protocol::CalculatorPullResult {
                    cookie: None,
                    last_mutation_id: 0,
                    rules: Vec::new(),
                    plans: Vec::new(),
                    history: Vec::new(),
                },
            }
        }
    }

    impl Transport for FakeTransport {
        async fn push(
            &self,
            _client_id: &str,
            _account_id: &str,
            mutations: &[OutboxRow],
        ) -> Result<i64, String> {
            self.push_calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_push {
                return Err("offline".into());
            }
            Ok(mutations.iter().map(|m| m.mutation_id).max().unwrap_or(0))
        }

        async fn pull(
            &self,
            _client_id: &str,
            _account_id: &str,
            _cookie: Option<&str>,
        ) -> Result<PullResult, String> {
            self.pull_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.pull_result.clone())
        }

        async fn pull_updates(
            &self,
            _account_id: &str,
            since_seq: i64,
        ) -> Result<Vec<crate::sync::protocol::RemoteUpdate>, String> {
            self.since_seqs.lock().unwrap().push(since_seq);
            Ok(self
                .remote_updates
                .iter()
                .filter(|u| u.seq > since_seq)
                .map(|u| crate::sync::protocol::RemoteUpdate {
                    note_id: u.note_id.clone(),
                    seq: u.seq,
                    update: u.update.clone(),
                })
                .collect())
        }

        async fn pull_playbook(
            &self,
            _client_id: &str,
            _cookie: Option<&str>,
        ) -> Result<protocol::PlaybookPullResult, String> {
            Ok(self.playbook_result.clone())
        }

        async fn pull_journal(
            &self,
            _client_id: &str,
            _account_id: &str,
            _cookie: Option<&str>,
        ) -> Result<protocol::JournalPullResult, String> {
            Ok(self.journal_result.clone())
        }

        async fn pull_tags_sync(
            &self,
            _client_id: &str,
            _cookie: Option<&str>,
        ) -> Result<protocol::TagsPullResult, String> {
            Ok(self.tags_sync_result.clone())
        }

        async fn pull_principle(
            &self,
            _client_id: &str,
            _account_id: &str,
            _cookie: Option<&str>,
        ) -> Result<protocol::PrinciplePullResult, String> {
            Ok(self.principle_result.clone())
        }

        async fn pull_accounts(&self) -> Result<Vec<protocol::WireAccount>, String> {
            Ok(self.accounts_result.clone())
        }

        async fn pull_calculator(
            &self,
            _client_id: &str,
            _cookie: Option<&str>,
        ) -> Result<protocol::CalculatorPullResult, String> {
            Ok(self.calculator_result.clone())
        }
    }

    fn wire_playbook(id: &str, name: &str, hlc: &str, deleted: bool) -> protocol::WirePlaybook {
        protocol::WirePlaybook {
            id: id.into(),
            name: name.into(),
            edge_name: "Momentum".into(),
            entry_rules: "e".into(),
            exit_rules: "x".into(),
            position_sizing_rules: "p".into(),
            additional_rules: None,
            hlc: hlc.into(),
            deleted_at: if deleted { Some("2026-01-01T00:00:00.000000Z".into()) } else { None },
            updated_at: "2026-01-01T00:00:00.000000Z".into(),
        }
    }

    #[test]
    fn playbook_pull_inserts_merges_idempotently_and_tombstones() {
        use crate::db::playbook::list_playbooks;

        let (db, hlc) = setup();
        let mut fake = FakeTransport::new(empty_pull());
        fake.playbook_result = protocol::PlaybookPullResult {
            cookie: Some("pb-cookie-1".into()),
            last_mutation_id: 0,
            playbooks: vec![wire_playbook("pb1", "Breakout", "000000000000005:00000:remote", false)],
        };

        let n = tauri::async_runtime::block_on(pull_playbooks(&db, &hlc, &fake)).unwrap();
        assert_eq!(n, 1);
        {
            let conn = db.lock().unwrap();
            let rows = list_playbooks(&conn).unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].name, "Breakout");
        }

        // Applying the same delta again is a no-op (idempotent).
        tauri::async_runtime::block_on(pull_playbooks(&db, &hlc, &fake)).unwrap();
        assert_eq!(db.lock().unwrap().query_row("SELECT COUNT(*) FROM playbooks", [], |r| r.get::<_, i64>(0)).unwrap(), 1);

        // A newer tombstone hides it.
        fake.playbook_result.playbooks =
            vec![wire_playbook("pb1", "Breakout", "000000000000006:00000:remote", true)];
        tauri::async_runtime::block_on(pull_playbooks(&db, &hlc, &fake)).unwrap();
        assert_eq!(list_playbooks(&db.lock().unwrap()).unwrap().len(), 0);
    }

    fn wire_tag_category(
        id: &str,
        name: &str,
        hlc: &str,
        deleted: bool,
    ) -> protocol::WireTagCategoryDelta {
        protocol::WireTagCategoryDelta {
            id: id.into(),
            name: name.into(),
            role: None,
            color: Some("#ff0000".into()),
            sort_order: 0,
            hlc: hlc.into(),
            deleted_at: if deleted { Some("2026-01-01T00:00:00.000000Z".into()) } else { None },
            updated_at: "2026-01-01T00:00:00.000000Z".into(),
        }
    }

    fn wire_tag(id: &str, category_id: &str, name: &str, hlc: &str, deleted: bool) -> protocol::WireTagDelta {
        protocol::WireTagDelta {
            id: id.into(),
            category_id: category_id.into(),
            name: name.into(),
            color: None,
            hlc: hlc.into(),
            deleted_at: if deleted { Some("2026-01-01T00:00:00.000000Z".into()) } else { None },
            updated_at: "2026-01-01T00:00:00.000000Z".into(),
        }
    }

    #[test]
    fn tags_pull_inserts_merges_idempotently_and_tombstones() {
        let (db, hlc) = setup();
        let mut fake = FakeTransport::new(empty_pull());
        fake.tags_sync_result = protocol::TagsPullResult {
            cookie: Some("tag-cookie-1".into()),
            last_mutation_id: 0,
            categories: vec![wire_tag_category(
                "cat1",
                "Custom",
                "000000000000005:00000:remote",
                false,
            )],
            tags: vec![wire_tag("tag1", "cat1", "Breakout", "000000000000005:00000:remote", false)],
        };

        let n = tauri::async_runtime::block_on(pull_tags_sync_apply(&db, &hlc, &fake)).unwrap();
        assert_eq!(n, 2);
        {
            let conn = db.lock().unwrap();
            let (cat_name, cat_color): (String, Option<String>) = conn
                .query_row(
                    "SELECT name, color FROM tag_categories_cache WHERE id = 'cat1'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(cat_name, "Custom");
            assert_eq!(cat_color.as_deref(), Some("#ff0000"));
            let tag_name: String = conn
                .query_row("SELECT name FROM tags_cache WHERE id = 'tag1'", [], |r| r.get(0))
                .unwrap();
            assert_eq!(tag_name, "Breakout");
        }

        // Applying the same delta again is a no-op (idempotent).
        tauri::async_runtime::block_on(pull_tags_sync_apply(&db, &hlc, &fake)).unwrap();
        assert_eq!(
            db.lock()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM tag_categories_cache", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.lock()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM tags_cache", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );

        // A newer tombstone hides both.
        fake.tags_sync_result.categories =
            vec![wire_tag_category("cat1", "Custom", "000000000000006:00000:remote", true)];
        fake.tags_sync_result.tags =
            vec![wire_tag("tag1", "cat1", "Breakout", "000000000000006:00000:remote", true)];
        tauri::async_runtime::block_on(pull_tags_sync_apply(&db, &hlc, &fake)).unwrap();
        let conn = db.lock().unwrap();
        let cat_deleted: Option<String> = conn
            .query_row("SELECT deleted_at FROM tag_categories_cache WHERE id = 'cat1'", [], |r| r.get(0))
            .unwrap();
        assert!(cat_deleted.is_some());
        let tag_deleted: Option<String> = conn
            .query_row("SELECT deleted_at FROM tags_cache WHERE id = 'tag1'", [], |r| r.get(0))
            .unwrap();
        assert!(tag_deleted.is_some());
    }

    fn wire_journal_entry(
        id: &str,
        symbol: &str,
        hlc: &str,
        deleted: bool,
    ) -> protocol::WireJournalEntry {
        protocol::WireJournalEntry {
            id: id.into(),
            open_date: "2026-01-01T00:00:00Z".into(),
            close_date: "2026-01-01T01:00:00Z".into(),
            entry_price: 100.0,
            exit_price: 110.0,
            position_size: 10.0,
            stop_loss: Some(95.0),
            symbol: symbol.into(),
            symbol_name: "Apple Inc.".into(),
            status: "profit".into(),
            total_pl: 10.0,
            net_roi: 10.0,
            duration: 3600,
            risk_reward: Some(2.0),
            trade_type: "long".into(),
            playbook_id: None,
            notes: Some("clean breakout".into()),
            broke_30min_rule: Some(false),
            pre_trade_conviction: Some(4),
            market_regime: Some("trending".into()),
            is_planned_pre_market: Some(true),
            revenge_trade: Some(false),
            rule_adherence_score: Some(5),
            tag_ids: vec!["tag-1".into()],
            hlc: hlc.into(),
            deleted_at: if deleted { Some("2026-01-01T00:00:00.000000Z".into()) } else { None },
            updated_at: "2026-01-01T00:00:00.000000Z".into(),
        }
    }

    #[test]
    fn journal_pull_inserts_merges_idempotently_and_tombstones() {
        use crate::db::journal::list_journal_entries;

        let (db, hlc) = setup();
        let mut fake = FakeTransport::new(empty_pull());
        fake.journal_result = protocol::JournalPullResult {
            cookie: Some("j-cookie-1".into()),
            last_mutation_id: 0,
            entries: vec![wire_journal_entry(
                "t1",
                "AAPL",
                "000000000000005:00000:remote",
                false,
            )],
        };

        let n =
            tauri::async_runtime::block_on(pull_journal_account(&db, &hlc, &fake, "acct-1"))
                .unwrap();
        assert_eq!(n, 1);
        {
            let conn = db.lock().unwrap();
            let rows = list_journal_entries(&conn, "acct-1").unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].symbol, "AAPL");
            assert_eq!(rows[0].tag_ids, vec!["tag-1".to_string()]);
        }

        // Applying the same delta again is a no-op (idempotent).
        tauri::async_runtime::block_on(pull_journal_account(&db, &hlc, &fake, "acct-1"))
            .unwrap();
        assert_eq!(
            db.lock()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM journal_entries", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );

        // A newer tombstone hides it.
        fake.journal_result.entries = vec![wire_journal_entry(
            "t1",
            "AAPL",
            "000000000000006:00000:remote",
            true,
        )];
        tauri::async_runtime::block_on(pull_journal_account(&db, &hlc, &fake, "acct-1"))
            .unwrap();
        assert_eq!(
            list_journal_entries(&db.lock().unwrap(), "acct-1").unwrap().len(),
            0
        );
    }

    fn wire_principle(id: &str, title: &str, hlc: &str, deleted: bool) -> protocol::WirePrinciple {
        protocol::WirePrinciple {
            id: id.into(),
            account_id: "acct-1".into(),
            playbook_id: None,
            evidence_note_id: None,
            title: title.into(),
            the_rule: "Wait 30 minutes after a loss".into(),
            why: "Revenge trades blow up accounts".into(),
            intervention: Some("Close the platform".into()),
            priority: 0,
            is_active: true,
            hlc: hlc.into(),
            deleted_at: if deleted { Some("2026-01-01T00:00:00.000000Z".into()) } else { None },
            updated_at: "2026-01-01T00:00:00.000000Z".into(),
        }
    }

    #[test]
    fn principle_pull_inserts_merges_idempotently_and_tombstones() {
        use crate::db::principle::list_principles;

        let (db, hlc) = setup();
        let mut fake = FakeTransport::new(empty_pull());
        fake.principle_result = protocol::PrinciplePullResult {
            cookie: Some("p-cookie-1".into()),
            last_mutation_id: 0,
            principles: vec![wire_principle(
                "pr1",
                "No revenge trades",
                "000000000000005:00000:remote",
                false,
            )],
        };

        let n =
            tauri::async_runtime::block_on(pull_principle_account(&db, &hlc, &fake, "acct-1"))
                .unwrap();
        assert_eq!(n, 1);
        {
            let conn = db.lock().unwrap();
            let rows = list_principles(&conn, "acct-1").unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].title, "No revenge trades");
            assert!(rows[0].is_active);
        }

        // Applying the same delta again is a no-op (idempotent).
        tauri::async_runtime::block_on(pull_principle_account(&db, &hlc, &fake, "acct-1"))
            .unwrap();
        assert_eq!(
            db.lock()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM trading_principles", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );

        // A newer tombstone hides it.
        fake.principle_result.principles = vec![wire_principle(
            "pr1",
            "No revenge trades",
            "000000000000006:00000:remote",
            true,
        )];
        tauri::async_runtime::block_on(pull_principle_account(&db, &hlc, &fake, "acct-1"))
            .unwrap();
        assert_eq!(
            list_principles(&db.lock().unwrap(), "acct-1").unwrap().len(),
            0
        );
    }

    #[test]
    fn refresh_accounts_cache_upserts_from_fake_transport() {
        let (db, _hlc) = setup();
        let mut fake = FakeTransport::new(empty_pull());
        fake.accounts_result = vec![protocol::WireAccount {
            id: "acct-1".into(),
            name: "Main".into(),
            broker: Some("Webull".into()),
            currency: Some("USD".into()),
            icon: Some("chart-line-data-01".into()),
            total_value: Some(12345.67),
            risk_profile: Some("moderate".into()),
        }];

        tauri::async_runtime::block_on(refresh_accounts_cache(&db, &fake)).unwrap();

        let conn = db.lock().unwrap();
        let (name, total_value): (String, f64) = conn
            .query_row(
                "SELECT name, total_value FROM accounts_cache WHERE id = 'acct-1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(name, "Main");
        assert!((total_value - 12345.67).abs() < 1e-9);
    }

    fn wire_rule(account_id: &str, id: &str, balance: f64, hlc: &str, deleted: bool) -> protocol::WireRule {
        protocol::WireRule {
            id: id.into(),
            account_id: account_id.into(),
            account_balance: balance,
            account_risk: 1.0,
            max_stop_loss_pct: 2.0,
            hlc: hlc.into(),
            deleted_at: if deleted { Some("2026-01-01T00:00:00.000000Z".into()) } else { None },
            updated_at: "2026-01-01T00:00:00.000000Z".into(),
        }
    }

    #[test]
    fn calculator_rule_pull_inserts_merges_idempotently_and_tombstones() {
        use crate::db::calculator::get_rule;

        let (db, hlc) = setup();
        let mut fake = FakeTransport::new(empty_pull());
        fake.calculator_result = protocol::CalculatorPullResult {
            cookie: Some("calc-cookie-1".into()),
            last_mutation_id: 0,
            rules: vec![wire_rule(
                "acct-1",
                "rule1",
                10_000.0,
                "000000000000005:00000:remote",
                false,
            )],
            plans: vec![],
            history: vec![],
        };

        let n = tauri::async_runtime::block_on(pull_calculator_sync(&db, &hlc, &fake)).unwrap();
        assert_eq!(n, 1);
        {
            let conn = db.lock().unwrap();
            let rule = get_rule(&conn, "acct-1").unwrap().unwrap();
            assert_eq!(rule.id, "rule1");
            assert_eq!(rule.account_balance, 10_000.0);
        }

        // Applying the same delta again is a no-op (idempotent).
        tauri::async_runtime::block_on(pull_calculator_sync(&db, &hlc, &fake)).unwrap();
        assert_eq!(
            db.lock()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM calc_rules", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );

        // A newer remote edit wins (merge).
        fake.calculator_result.rules = vec![wire_rule(
            "acct-1",
            "rule1",
            20_000.0,
            "000000000000006:00000:remote",
            false,
        )];
        tauri::async_runtime::block_on(pull_calculator_sync(&db, &hlc, &fake)).unwrap();
        assert_eq!(
            get_rule(&db.lock().unwrap(), "acct-1").unwrap().unwrap().account_balance,
            20_000.0
        );

        // A newer tombstone hides it.
        fake.calculator_result.rules = vec![wire_rule(
            "acct-1",
            "rule1",
            20_000.0,
            "000000000000007:00000:remote",
            true,
        )];
        tauri::async_runtime::block_on(pull_calculator_sync(&db, &hlc, &fake)).unwrap();
        assert_eq!(get_rule(&db.lock().unwrap(), "acct-1").unwrap(), None);
    }

    fn empty_pull() -> PullResult {
        PullResult {
            cookie: Some("cookie-1".into()),
            last_mutation_id: 0,
            notes: vec![],
            folders: vec![],
        }
    }

    fn setup() -> (Mutex<Connection>, Mutex<Hlc>) {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../db/schema.sql"))
            .unwrap();
        conn.execute("INSERT INTO client (id) VALUES ('client-1')", [])
            .unwrap();
        (Mutex::new(conn), Mutex::new(Hlc::new("client-1")))
    }

    fn enqueue_outbox(db: &Mutex<Connection>) {
        db.lock()
            .unwrap()
            .execute(
                "INSERT INTO outbox (name, args, hlc) VALUES ('appendNoteUpdate', '{}', 'h')",
                [],
            )
            .unwrap();
    }

    #[test]
    fn refuses_to_pull_when_outbox_not_drained() {
        let (db, hlc) = setup();
        enqueue_outbox(&db);
        let mut fake = FakeTransport::new(empty_pull());
        fake.fail_push = true;

        let res = tauri::async_runtime::block_on(sync_once(&db, &hlc, "acct-1", &fake));

        assert!(res.is_err(), "a failed push must fail the cycle");
        assert_eq!(
            fake.pull_calls.load(Ordering::SeqCst),
            0,
            "must never pull while the outbox is non-empty"
        );
        let remaining: i64 = db
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM outbox", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            remaining, 1,
            "the un-pushed mutation survives for the retry"
        );
    }

    #[test]
    fn empty_outbox_never_pushes() {
        let (db, hlc) = setup();
        let fake = FakeTransport::new(empty_pull());

        tauri::async_runtime::block_on(sync_once(&db, &hlc, "acct-1", &fake)).unwrap();

        assert_eq!(
            fake.push_calls.load(Ordering::SeqCst),
            0,
            "an empty outbox must not push (Ok(0) would rewind the watermark)"
        );
        let wm: i64 = db
            .lock()
            .unwrap()
            .query_row(
                "SELECT COALESCE((SELECT last_mutation_id FROM sync_meta WHERE account_id='acct-1'), 0)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(wm, 0, "watermark must not be created from a no-op push");
    }

    #[test]
    fn drains_then_pulls_and_advances_watermark() {
        let (db, hlc) = setup();
        enqueue_outbox(&db);
        let fake = FakeTransport::new(empty_pull());

        let report = tauri::async_runtime::block_on(sync_once(&db, &hlc, "acct-1", &fake)).unwrap();

        assert_eq!(fake.pull_calls.load(Ordering::SeqCst), 1);
        assert_eq!(report.pushed, 1);
        let conn = db.lock().unwrap();
        let remaining: i64 = conn
            .query_row("SELECT count(*) FROM outbox", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 0, "outbox drained before pull");
        let wm: i64 = conn
            .query_row(
                "SELECT last_mutation_id FROM sync_meta WHERE account_id='acct-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(wm, 1);
        let cookie: Option<String> = conn
            .query_row(
                "SELECT cookie FROM sync_meta WHERE account_id='acct-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cookie.as_deref(), Some("cookie-1"), "opaque cookie stored");
    }

    fn remote(note_id: &str, seq: i64, bytes: &[u8]) -> crate::sync::protocol::RemoteUpdate {
        crate::sync::protocol::RemoteUpdate {
            note_id: note_id.into(),
            seq,
            update: base64::engine::general_purpose::STANDARD.encode(bytes),
        }
    }

    #[test]
    fn pulled_updates_land_locally_and_are_never_echoed_back() {
        let (db, hlc) = setup();
        let mut fake = FakeTransport::new(empty_pull());
        fake.remote_updates = vec![remote("note-a", 7, b"one"), remote("note-a", 9, b"two")];

        let report = tauri::async_runtime::block_on(sync_once(&db, &hlc, "acct-1", &fake)).unwrap();
        assert_eq!(report.pulled_updates, 2);
        assert_eq!(report.updated_notes, vec!["note-a".to_string()]);

        let conn = db.lock().unwrap();
        let stored: i64 = conn
            .query_row("SELECT count(*) FROM note_updates WHERE note_id = 'note-a'", params![], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, 2);

        // A server update must never become an outbox row: it would be pushed back,
        // pulled again, and grow the note's log without bound.
        let outbox: i64 = conn
            .query_row("SELECT count(*) FROM outbox", params![], |r| r.get(0))
            .unwrap();
        assert_eq!(outbox, 0, "pulled updates must not be echoed back to the server");

        let cursor: i64 = conn
            .query_row("SELECT last_seq FROM update_cursor WHERE account_id = 'acct-1'", params![], |r| r.get(0))
            .unwrap();
        assert_eq!(cursor, 9);
    }

    #[test]
    fn the_replay_window_re_reads_without_duplicating() {
        let (db, hlc) = setup();
        let mut fake = FakeTransport::new(empty_pull());
        fake.remote_updates = vec![remote("note-a", 7, b"one")];

        tauri::async_runtime::block_on(sync_once(&db, &hlc, "acct-1", &fake)).unwrap();
        let second = tauri::async_runtime::block_on(sync_once(&db, &hlc, "acct-1", &fake)).unwrap();

        // The cursor deliberately rewinds by REPLAY_WINDOW, so the server re-serves
        // seq 7. Storing it twice would replay a duplicate update into the editor.
        assert_eq!(second.pulled_updates, 0, "a re-read row must not be stored twice");

        let conn = db.lock().unwrap();
        let stored: i64 = conn
            .query_row("SELECT count(*) FROM note_updates", params![], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, 1);

        drop(conn);
        let asked = fake.since_seqs.lock().unwrap().clone();
        assert_eq!(asked, vec![0, 0], "cursor 7 minus the window clamps to 0");
    }
}
