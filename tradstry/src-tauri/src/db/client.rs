use rusqlite::Connection;
use tauri::Manager;
use uuid::Uuid;

pub fn open(app: &tauri::AppHandle) -> Result<Connection, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let conn = Connection::open(dir.join("notebook.db")).map_err(|e| e.to_string())?;
    conn.execute_batch(include_str!("schema.sql"))
        .map_err(|e| e.to_string())?;
    add_missing_columns(&conn)?;

    let has_client: bool = conn
        .query_row("SELECT EXISTS(SELECT 1 FROM client)", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if !has_client {
        conn.execute(
            "INSERT INTO client (id) VALUES (?1)",
            [Uuid::now_v7().to_string()],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(conn)
}

/// The desktop has no migration framework: `schema.sql` is `CREATE TABLE IF NOT
/// EXISTS`, which does NOT add columns to a table that already exists. When a
/// column is added to a table that predates it (e.g. the tag caches gaining
/// `color`/`hlc`/`deleted_at`/`sync_state` once tag CRUD went offline-first), an
/// existing DB is missing it. Each `ALTER ... ADD COLUMN` here is idempotent: on a
/// fresh DB `schema.sql` already created the column, so the ALTER fails with
/// "duplicate column name", which we ignore. Only new columns on pre-existing
/// tables need to be listed (brand-new tables are handled by `CREATE IF NOT EXISTS`).
fn add_missing_columns(conn: &Connection) -> Result<(), String> {
    const ALTERS: &[&str] = &[
        "ALTER TABLE tag_categories_cache ADD COLUMN color TEXT",
        "ALTER TABLE tag_categories_cache ADD COLUMN hlc TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE tag_categories_cache ADD COLUMN deleted_at TEXT",
        "ALTER TABLE tag_categories_cache ADD COLUMN sync_state TEXT NOT NULL DEFAULT 'pending'",
        "ALTER TABLE tags_cache ADD COLUMN hlc TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE tags_cache ADD COLUMN deleted_at TEXT",
        "ALTER TABLE tags_cache ADD COLUMN sync_state TEXT NOT NULL DEFAULT 'pending'",
    ];
    for alter in ALTERS {
        match conn.execute(alter, []) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column name") => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
