use base64::Engine as _;
use rusqlite::{params, Connection, Transaction};
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::sync::hlc::Hlc;

const UNTITLED_TITLE: &str = "Untitled";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub folder_id: Option<String>,
    /// Server-derived cache of the first H1; never an input.
    pub title: String,
    pub document_json: String,
    pub trade_ids: Vec<String>,
    pub sort_order: i64,
}

/// Writes one outbox row on a caller-owned transaction, so the data write and its
/// mutation are committed atomically. `args` carries every generated id and stamp.
pub(crate) fn enqueue(
    tx: &Transaction,
    name: &str,
    args: &Value,
    hlc: &str,
) -> Result<(), String> {
    tx.execute(
        "INSERT INTO outbox (name, args, hlc) VALUES (?1, ?2, ?3)",
        params![name, args.to_string(), hlc],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

fn collect_text(node: &Value, out: &mut String) {
    if let Some(t) = node.get("text").and_then(Value::as_str) {
        out.push_str(t);
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child in children {
            collect_text(child, out);
        }
    }
}

fn node_text(node: &Value) -> Option<String> {
    let mut out = String::new();
    collect_text(node, &mut out);
    let trimmed = out.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// The note title is a pure function of the document — the first H1's text, else
/// the first block's text. Mirrors the server's `derive_note_title` so the title
/// the desktop shows offline matches the one the server confirms on sync.
fn derive_title(document_json: &str) -> String {
    let Ok(doc) = serde_json::from_str::<Value>(document_json) else {
        return UNTITLED_TITLE.to_string();
    };
    let Some(children) = doc
        .get("root")
        .and_then(|r| r.get("children"))
        .and_then(Value::as_array)
    else {
        return UNTITLED_TITLE.to_string();
    };

    for child in children {
        let is_h1 = child.get("type").and_then(Value::as_str) == Some("heading")
            && child.get("tag").and_then(Value::as_str) == Some("h1");
        if is_h1 {
            if let Some(title) = node_text(child) {
                return title;
            }
        }
    }
    for child in children {
        if let Some(title) = node_text(child) {
            return title;
        }
    }
    UNTITLED_TITLE.to_string()
}

pub fn create_note(
    conn: &mut Connection,
    hlc: &mut Hlc,
    account_id: &str,
    folder_id: Option<&str>,
    document_json: &str,
    seed: &[u8],
    state_vector: &[u8],
) -> Result<String, String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let id = create_note_tx(&tx, hlc, account_id, folder_id, document_json, seed, state_vector)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

/// The creator seeds. The note's first Yjs update is minted by the device that
/// creates it, stored locally so the editor opens as a CRDT note even offline, and
/// carried in the `createNote` mutation so the server installs that exact blob
/// rather than seeding a second, conflicting Y.Doc of its own.
pub(crate) fn create_note_tx(
    tx: &Transaction,
    hlc: &mut Hlc,
    account_id: &str,
    folder_id: Option<&str>,
    document_json: &str,
    seed: &[u8],
    state_vector: &[u8],
) -> Result<String, String> {
    // UUIDv7 for Postgres index locality. Minted HERE, passed into the mutation
    // as an argument: mutations are replayed during rebase, so an id generated
    // inside one would differ on every replay.
    let id = Uuid::now_v7().to_string();
    let stamp = hlc.now();

    tx.execute(
        "INSERT INTO notes (id, account_id, folder_id, title, document_json, body_hlc)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, account_id, folder_id, derive_title(document_json), document_json, stamp],
    )
    .map_err(|e| e.to_string())?;

    // No outbox row for the seed: it rides along in `createNote` below. Enqueuing it
    // as an `appendNoteUpdate` too would push the same blob twice.
    tx.execute(
        "INSERT INTO note_updates (note_id, \"update\") VALUES (?1, ?2)",
        params![id, seed],
    )
    .map_err(|e| e.to_string())?;

    let b64 = base64::engine::general_purpose::STANDARD;
    enqueue(
        tx,
        "createNote",
        &json!({
            "id": id,
            "accountId": account_id,
            "documentJson": document_json,
            "tradeIds": [],
            "folderId": folder_id,
            "seedUpdate": b64.encode(seed),
            "seedStateVector": b64.encode(state_vector),
        }),
        &stamp,
    )
    .map_err(|e| e.to_string())?;

    Ok(id)
}

/// Refreshes the locally cached projection of a note's body: `document_json` for the
/// list preview and the derived `title`. Purely a cache — no HLC stamp, no outbox
/// row. The body itself is a CRDT and travels only as update blobs; the server
/// re-derives both of these from the projected document.
pub fn cache_note_body(conn: &Connection, id: &str, document_json: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE notes SET document_json = ?1, title = ?2 WHERE id = ?3",
        params![document_json, derive_title(document_json), id],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Reparents a note and restamps the two fields the server merges per-field.
/// `folder_id: None` moves it back to Uncategorized.
pub fn move_note(
    conn: &mut Connection,
    hlc: &mut Hlc,
    id: &str,
    folder_id: Option<&str>,
    sort_order: i64,
) -> Result<(), String> {
    let stamp = hlc.now();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let account_id: String = tx
        .query_row("SELECT account_id FROM notes WHERE id = ?1", params![id], |r| r.get(0))
        .map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE notes SET folder_id = ?1, sort_order = ?2, hlc_folder_id = ?3,
                          hlc_sort_order = ?3, sync_state = 'pending'
         WHERE id = ?4",
        params![folder_id, sort_order, stamp, id],
    )
    .map_err(|e| e.to_string())?;

    enqueue(
        &tx,
        "moveNode",
        &json!({
            "accountId": account_id,
            "nodeId": id,
            "nodeType": "note",
            "newParentFolderId": folder_id,
            "newSortOrder": sort_order,
        }),
        &stamp,
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_note(conn: &mut Connection, hlc: &mut Hlc, id: &str) -> Result<(), String> {
    let stamp = hlc.now();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE notes SET deleted_at = datetime('now'), sync_state = 'pending' WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    enqueue(&tx, "deleteNote", &json!({ "id": id }), &stamp).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_notes(conn: &Connection, account_id: &str) -> Result<Vec<Note>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, folder_id, title, document_json, trade_ids, sort_order
             FROM notes
             WHERE account_id = ?1 AND deleted_at IS NULL
             ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![account_id], |r| {
            let trade_ids_json: String = r.get(4)?;
            Ok(Note {
                id: r.get(0)?,
                folder_id: r.get(1)?,
                title: r.get(2)?,
                document_json: r.get(3)?,
                trade_ids: serde_json::from_str(&trade_ids_json).unwrap_or_default(),
                sort_order: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    /// The local store treats a seed as opaque bytes; only the server applies it.
    const TEST_DOC: &str = r#"{"root":{"children":[],"type":"root","version":1}}"#;

    use super::*;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        conn
    }

    #[test]
    fn the_creator_seeds_exactly_once() {
        let mut conn = memory_db();
        let mut hlc = Hlc::new("c1");
        let id = create_note(&mut conn, &mut hlc, "acct-1", None, TEST_DOC, b"seed", b"sv").unwrap();

        // Stored locally, so the editor opens the note as a CRDT note while offline.
        let updates: i64 = conn
            .query_row(
                "SELECT count(*) FROM note_updates WHERE note_id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(updates, 1, "the creator must seed its own note");

        // Carried in createNote, NOT as a second appendNoteUpdate row.
        let appends: i64 = conn
            .query_row(
                "SELECT count(*) FROM outbox WHERE name = 'appendNoteUpdate'",
                params![],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(appends, 0, "the seed must not also be enqueued as an append");

        let args: String = conn
            .query_row(
                "SELECT args FROM outbox WHERE name = 'createNote'",
                params![],
                |r| r.get(0),
            )
            .unwrap();
        let args: Value = serde_json::from_str(&args).unwrap();
        assert_eq!(args["seedUpdate"], base64::engine::general_purpose::STANDARD.encode(b"seed"));
        assert_eq!(
            args["seedStateVector"],
            base64::engine::general_purpose::STANDARD.encode(b"sv")
        );
    }

    #[test]
    fn create_note_derives_the_title_from_the_document() {
        let mut conn = memory_db();
        let mut hlc = Hlc::new("c1");
        let doc = r#"{"root":{"children":[{"type":"heading","tag":"h1","children":[{"type":"text","text":"My Note"}]}]}}"#;
        let id = create_note(&mut conn, &mut hlc, "acct-1", None, doc, b"seed", b"sv").unwrap();

        let title: String = conn
            .query_row("SELECT title FROM notes WHERE id = ?1", params![id], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "My Note", "the title must be derived at creation, not left Untitled");
    }

    #[test]
    fn cache_note_body_derives_title_from_first_h1() {
        let mut conn = memory_db();
        let mut hlc = Hlc::new("c1");
        let id = create_note(&mut conn, &mut hlc, "acct-1", None, TEST_DOC, b"seed", b"sv").unwrap();
        let doc = r#"{"root":{"children":[{"type":"heading","tag":"h1","children":[{"type":"text","text":"Hello"}]}]}}"#;
        cache_note_body(&conn, &id, doc).unwrap();

        let title: String = conn
            .query_row("SELECT title FROM notes WHERE id = ?1", params![id], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Hello");

        let outbox: i64 = conn
            .query_row("SELECT count(*) FROM outbox WHERE name = 'updateNote'", params![], |r| r.get(0))
            .unwrap();
        assert_eq!(outbox, 0, "caching the body must not enqueue updateNote");
    }

    #[test]
    fn moving_a_note_restamps_and_enqueues_move_node() {
        let mut conn = memory_db();
        let mut hlc = Hlc::new("c1");
        let id = create_note(&mut conn, &mut hlc, "acct-1", None, TEST_DOC, b"seed", b"sv").unwrap();

        move_note(&mut conn, &mut hlc, &id, Some("folder-1"), 3).unwrap();

        let (folder, sort, hlc_folder, hlc_sort): (Option<String>, i64, String, String) = conn
            .query_row(
                "SELECT folder_id, sort_order, hlc_folder_id, hlc_sort_order FROM notes WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(folder.as_deref(), Some("folder-1"));
        assert_eq!(sort, 3);
        assert!(!hlc_folder.is_empty());
        assert_eq!(hlc_folder, hlc_sort);

        let args: String = conn
            .query_row("SELECT args FROM outbox WHERE name = 'moveNode'", params![], |r| r.get(0))
            .expect("a local move must enqueue moveNode");
        let args: Value = serde_json::from_str(&args).unwrap();
        assert_eq!(args["nodeId"], id);
        assert_eq!(args["nodeType"], "note");
        assert_eq!(args["newParentFolderId"], "folder-1");
        assert_eq!(args["newSortOrder"], 3);
        assert_eq!(args["accountId"], "acct-1");
    }

    #[test]
    fn moving_a_note_to_uncategorized_sends_a_null_parent() {
        let mut conn = memory_db();
        let mut hlc = Hlc::new("c1");
        let id = create_note(&mut conn, &mut hlc, "acct-1", Some("folder-1"), TEST_DOC, b"seed", b"sv").unwrap();

        move_note(&mut conn, &mut hlc, &id, None, 0).unwrap();

        let folder: Option<String> = conn
            .query_row("SELECT folder_id FROM notes WHERE id = ?1", params![id], |r| r.get(0))
            .unwrap();
        assert!(folder.is_none());

        let args: String = conn
            .query_row("SELECT args FROM outbox WHERE name = 'moveNode'", params![], |r| r.get(0))
            .unwrap();
        let args: Value = serde_json::from_str(&args).unwrap();
        assert!(args["newParentFolderId"].is_null(), "Uncategorized is a null parent");
    }

    #[test]
    fn list_notes_excludes_deleted_and_other_accounts() {
        let mut conn = memory_db();
        let mut hlc = Hlc::new("c1");
        let a = create_note(&mut conn, &mut hlc, "acct-1", None, TEST_DOC, b"s", b"v").unwrap();
        create_note(&mut conn, &mut hlc, "acct-2", None, TEST_DOC, b"s", b"v").unwrap();
        let gone = create_note(&mut conn, &mut hlc, "acct-1", None, TEST_DOC, b"s", b"v").unwrap();
        delete_note(&mut conn, &mut hlc, &gone).unwrap();

        let notes = list_notes(&conn, "acct-1").unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, a);
    }
}
