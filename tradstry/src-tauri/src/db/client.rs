use rusqlite::Connection;
use tauri::Manager;
use uuid::Uuid;

pub fn open(app: &tauri::AppHandle) -> Result<Connection, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let conn = Connection::open(dir.join("notebook.db")).map_err(|e| e.to_string())?;
    conn.execute_batch(include_str!("schema.sql"))
        .map_err(|e| e.to_string())?;

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
