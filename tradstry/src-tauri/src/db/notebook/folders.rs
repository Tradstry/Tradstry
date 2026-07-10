use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::db::notebook::notes::enqueue;
use crate::sync::hlc::Hlc;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
}

pub fn list_folders(conn: &Connection, account_id: &str) -> Result<Vec<Folder>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, parent_folder_id, name, sort_order
             FROM folders
             WHERE account_id = ?1 AND deleted_at IS NULL
             ORDER BY sort_order ASC, name ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![account_id], |r| {
            Ok(Folder {
                id: r.get(0)?,
                parent_folder_id: r.get(1)?,
                name: r.get(2)?,
                sort_order: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

/// Creates a root folder. The three per-field HLCs are stamped together so the
/// server's per-field LWW merge compares against one coherent time.
pub fn create_folder(
    conn: &mut Connection,
    hlc: &mut Hlc,
    account_id: &str,
    name: &str,
) -> Result<String, String> {
    let id = Uuid::now_v7().to_string();
    let stamp = hlc.now();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO folders
         (id, account_id, parent_folder_id, name, sort_order, hlc_name, hlc_parent, hlc_sort_order, sync_state)
         VALUES (?1, ?2, NULL, ?3, 0, ?4, ?4, ?4, 'pending')",
        params![id, account_id, name, stamp],
    )
    .map_err(|e| e.to_string())?;

    enqueue(
        &tx,
        "createFolder",
        &json!({
            "id": id,
            "accountId": account_id,
            "name": name,
            "parentFolderId": serde_json::Value::Null,
            "sortOrder": 0,
        }),
        &stamp,
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

pub fn rename_folder(
    conn: &mut Connection,
    hlc: &mut Hlc,
    id: &str,
    name: &str,
) -> Result<(), String> {
    let stamp = hlc.now();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE folders SET name = ?1, hlc_name = ?2, sync_state = 'pending' WHERE id = ?3",
        params![name, stamp, id],
    )
    .map_err(|e| e.to_string())?;

    enqueue(&tx, "renameFolder", &json!({ "id": id, "name": name }), &stamp)
        .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Tombstones the folder AND its whole subtree (descendant folders + every note
/// inside them) locally, so the UI clears immediately, then enqueues ONE
/// `deleteFolder`. The server performs the same subtree cascade authoritatively;
/// the local tombstones just avoid a one-pull delay. No per-note/child outbox rows:
/// the server cascade covers them, and delete-wins means the returning tombstones
/// never resurrect anything.
pub fn delete_folder(conn: &mut Connection, hlc: &mut Hlc, id: &str) -> Result<(), String> {
    let stamp = hlc.now();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // The folder subtree: this folder plus every descendant, however deep.
    const SUBTREE: &str = "WITH RECURSIVE subtree(id) AS (
            SELECT id FROM folders WHERE id = ?1
            UNION
            SELECT f.id FROM folders f JOIN subtree s ON f.parent_folder_id = s.id
        )";

    tx.execute(
        &format!(
            "{SUBTREE}
             UPDATE folders SET deleted_at = datetime('now'), sync_state = 'pending'
             WHERE id IN (SELECT id FROM subtree)"
        ),
        params![id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        &format!(
            "{SUBTREE}
             UPDATE notes SET deleted_at = datetime('now'), sync_state = 'pending'
             WHERE folder_id IN (SELECT id FROM subtree)"
        ),
        params![id],
    )
    .map_err(|e| e.to_string())?;

    enqueue(&tx, "deleteFolder", &json!({ "id": id }), &stamp).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DOC: &str = r#"{"root":{"children":[],"type":"root","version":1}}"#;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        conn
    }

    #[test]
    fn create_folder_persists_and_enqueues() {
        let mut conn = memory_db();
        let mut hlc = Hlc::new("c1");
        let id = create_folder(&mut conn, &mut hlc, "acct-1", "Setups").unwrap();

        let folders = list_folders(&conn, "acct-1").unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].id, id);
        assert_eq!(folders[0].name, "Setups");
        assert!(folders[0].parent_folder_id.is_none());

        let args: String = conn
            .query_row(
                "SELECT args FROM outbox WHERE name = 'createFolder'",
                params![],
                |r| r.get(0),
            )
            .unwrap();
        let args: serde_json::Value = serde_json::from_str(&args).unwrap();
        assert_eq!(args["accountId"], "acct-1");
        assert_eq!(args["name"], "Setups");
        assert!(args["parentFolderId"].is_null());
        assert_eq!(args["sortOrder"], 0);
    }

    #[test]
    fn rename_folder_restamps_name_and_enqueues() {
        let mut conn = memory_db();
        let mut hlc = Hlc::new("c1");
        let id = create_folder(&mut conn, &mut hlc, "acct-1", "Old").unwrap();
        rename_folder(&mut conn, &mut hlc, &id, "New").unwrap();

        let folders = list_folders(&conn, "acct-1").unwrap();
        assert_eq!(folders[0].name, "New");

        let args: String = conn
            .query_row(
                "SELECT args FROM outbox WHERE name = 'renameFolder'",
                params![],
                |r| r.get(0),
            )
            .unwrap();
        let args: serde_json::Value = serde_json::from_str(&args).unwrap();
        assert_eq!(args["id"], id);
        assert_eq!(args["name"], "New");
    }

    #[test]
    fn delete_folder_cascades_to_its_notes_locally_but_enqueues_one_mutation() {
        let mut conn = memory_db();
        let mut hlc = Hlc::new("c1");
        let fid = create_folder(&mut conn, &mut hlc, "acct-1", "Trash me").unwrap();
        let n = |c: &mut Connection, h: &mut Hlc, folder: Option<&str>| {
            crate::db::notebook::notes::create_note(c, h, "acct-1", folder, TEST_DOC, b"s", b"v")
                .unwrap()
        };
        n(&mut conn, &mut hlc, Some(&fid));
        n(&mut conn, &mut hlc, Some(&fid));
        let outside = n(&mut conn, &mut hlc, None);

        delete_folder(&mut conn, &mut hlc, &fid).unwrap();

        // Folder gone, its notes gone locally, the outside note untouched.
        assert!(list_folders(&conn, "acct-1").unwrap().is_empty());
        let live = crate::db::notebook::notes::list_notes(&conn, "acct-1").unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].id, outside);

        // Exactly one deleteFolder mutation — the server cascades the rest. No per-note rows.
        let delete_folders: i64 = conn
            .query_row("SELECT count(*) FROM outbox WHERE name = 'deleteFolder'", params![], |r| r.get(0))
            .unwrap();
        assert_eq!(delete_folders, 1);
        let delete_notes: i64 = conn
            .query_row("SELECT count(*) FROM outbox WHERE name = 'deleteNote'", params![], |r| r.get(0))
            .unwrap();
        assert_eq!(delete_notes, 0, "cascaded notes must not enqueue their own deletes");
    }

    #[test]
    fn delete_folder_tombstones_nested_descendants() {
        let mut conn = memory_db();
        let mut hlc = Hlc::new("c1");
        let parent = create_folder(&mut conn, &mut hlc, "acct-1", "Parent").unwrap();
        // A child folder only arrives with a parent via sync, so insert it directly.
        conn.execute(
            "INSERT INTO folders (id, account_id, parent_folder_id, name, sync_state)
             VALUES ('child', 'acct-1', ?1, 'Child', 'synced')",
            params![parent],
        )
        .unwrap();
        crate::db::notebook::notes::create_note(
            &mut conn, &mut hlc, "acct-1", Some("child"), TEST_DOC, b"s", b"v",
        )
        .unwrap();

        delete_folder(&mut conn, &mut hlc, &parent).unwrap();

        assert!(list_folders(&conn, "acct-1").unwrap().is_empty(), "the nested child must be tombstoned too");
        assert!(
            crate::db::notebook::notes::list_notes(&conn, "acct-1").unwrap().is_empty(),
            "a note in a nested folder must be tombstoned"
        );
    }

    #[test]
    fn a_note_can_be_created_inside_a_folder() {
        let mut conn = memory_db();
        let mut hlc = Hlc::new("c1");
        let fid = create_folder(&mut conn, &mut hlc, "acct-1", "Box").unwrap();
        crate::db::notebook::notes::create_note(
            &mut conn, &mut hlc, "acct-1", Some(&fid), TEST_DOC, b"seed", b"sv",
        )
        .unwrap();

        let notes = crate::db::notebook::notes::list_notes(&conn, "acct-1").unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].folder_id.as_deref(), Some(fid.as_str()));
    }
}
