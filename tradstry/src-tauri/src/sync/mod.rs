pub mod hlc;
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
    pull_account(db, hlc, transport, account_id, &mut report).await?;
    pull_updates(db, transport, account_id, &mut report).await?;

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
               sync_state       = 'synced'
             WHERE id = ?1",
            params![
                wf.id,
                wf.hlc,
                wf.name,
                wf.parent_folder_id,
                wf.sort_order,
                wf.deleted_at
            ],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO folders
             (id, account_id, parent_folder_id, name, sort_order, hlc_name, hlc_parent, hlc_sort_order, deleted_at, sync_state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?6, ?7, 'synced')",
            params![
                wf.id,
                account_id,
                wf.parent_folder_id,
                wf.name,
                wf.sort_order,
                wf.hlc,
                wf.deleted_at
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
