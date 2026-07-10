use base64::Engine;
use rusqlite::{params, Connection};

use crate::db::notebook::notes::enqueue;

/// The Yjs blob and its outbox row in ONE transaction. The blob crosses this
/// boundary as `&[u8]` and is bound as a SQLite BLOB; it only becomes text as
/// base64 inside the outbox args, never as a lossy plain `String`.
pub fn append_update(conn: &mut Connection, note_id: &str, update: &[u8]) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO note_updates (note_id, \"update\") VALUES (?1, ?2)",
        params![note_id, update],
    )
    .map_err(|e| e.to_string())?;

    let update_b64 = base64::engine::general_purpose::STANDARD.encode(update);
    enqueue(
        &tx,
        "appendNoteUpdate",
        &serde_json::json!({ "noteId": note_id, "update": update_b64 }),
        "",
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())
}

/// Stores an update pulled from the server. Deliberately writes NO outbox row: it
/// came from the server, and sending it back would append it a second time, pull it
/// again, and grow the note's log without bound.
pub fn insert_remote_update(
    tx: &rusqlite::Transaction,
    note_id: &str,
    update: &[u8],
) -> Result<(), String> {
    tx.execute(
        "INSERT INTO note_updates (note_id, \"update\", synced) VALUES (?1, ?2, 1)",
        params![note_id, update],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

pub fn read_updates(conn: &Connection, note_id: &str) -> Result<Vec<Vec<u8>>, String> {
    let mut stmt = conn
        .prepare("SELECT \"update\" FROM note_updates WHERE note_id = ?1 ORDER BY seq ASC")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![note_id], |row| row.get::<_, Vec<u8>>(0))
        .map_err(|e| e.to_string())?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

pub fn update_cursor(conn: &Connection, account_id: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT last_seq FROM update_cursor WHERE account_id = ?1",
        params![account_id],
        |r| r.get(0),
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(0),
        other => Err(other.to_string()),
    })
}

pub fn set_update_cursor(
    tx: &rusqlite::Transaction,
    account_id: &str,
    last_seq: i64,
) -> Result<(), String> {
    tx.execute(
        "INSERT INTO update_cursor (account_id, last_seq) VALUES (?1, ?2)
         ON CONFLICT(account_id) DO UPDATE SET last_seq = MAX(last_seq, excluded.last_seq)",
        params![account_id, last_seq],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn memory_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        conn
    }

    // The bytes that break naive String handling: an interior NUL, high bytes,
    // and a lone 0x80 continuation byte that is not valid UTF-8.
    const NASTY: &[u8] = &[0x00, 0xff, 0xfe, 0x41, 0x00, 0x80, 0x7f];

    #[test]
    fn update_bytes_survive_the_sqlite_round_trip() {
        let mut conn = memory_db();
        append_update(&mut conn, "note-1", NASTY).unwrap();

        let blobs = read_updates(&conn, "note-1").unwrap();
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0], NASTY);

        // The outbox carries the bytes as standard base64, decodable back to NASTY.
        let args: String = conn
            .query_row(
                "SELECT args FROM outbox WHERE name = 'appendNoteUpdate'",
                params![],
                |r| r.get(0),
            )
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&args).unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(v["update"].as_str().unwrap())
            .unwrap();
        assert_eq!(decoded, NASTY);
    }

    #[test]
    fn a_pulled_update_stores_without_an_outbox_row() {
        let mut conn = memory_db();
        let tx = conn.transaction().unwrap();
        insert_remote_update(&tx, "note-1", NASTY).unwrap();
        set_update_cursor(&tx, "acct-1", 42).unwrap();
        tx.commit().unwrap();

        assert_eq!(read_updates(&conn, "note-1").unwrap()[0], NASTY);
        let outbox: i64 = conn
            .query_row("SELECT count(*) FROM outbox", params![], |r| r.get(0))
            .unwrap();
        assert_eq!(outbox, 0, "a pulled update must not be echoed back");
        assert_eq!(update_cursor(&conn, "acct-1").unwrap(), 42);
    }
}
