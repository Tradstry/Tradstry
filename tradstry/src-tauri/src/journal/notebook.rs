// Notebook commands for the desktop app. Local-first: every read and write goes
// to the on-device SQLite mirror and the transactional outbox. No GraphQL here —
// the sync loop (a later task) is the only thing that touches the network.

use base64::Engine;
use tauri::State;

use crate::db::notebook::folders::{self, Folder};
use crate::db::notebook::notes::{self, Note};
use crate::db::notebook::updates;
use crate::AppState;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

#[tauri::command]
pub fn notebook_notes(state: State<'_, AppState>, account_id: String) -> Result<Vec<Note>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    notes::list_notes(&conn, &account_id)
}

#[tauri::command]
pub fn notebook_folders(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<Folder>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    folders::list_folders(&conn, &account_id)
}

/// The webview mints the seed: it owns Lexical and Yjs, and the same shared builder
/// the server-side projector uses, so both produce identical bytes for a document.
#[tauri::command]
pub fn create_note(
    state: State<'_, AppState>,
    account_id: String,
    folder_id: Option<String>,
    document_json: String,
    seed_update_b64: String,
    seed_state_vector_b64: String,
) -> Result<String, String> {
    let seed = B64.decode(&seed_update_b64).map_err(|e| e.to_string())?;
    let state_vector = B64.decode(&seed_state_vector_b64).map_err(|e| e.to_string())?;
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    notes::create_note(
        &mut conn,
        &mut hlc,
        &account_id,
        folder_id.as_deref(),
        &document_json,
        &seed,
        &state_vector,
    )
}

/// Local cache only: the body syncs as CRDT update blobs, never as a document.
#[tauri::command]
pub fn cache_note_body(
    state: State<'_, AppState>,
    id: String,
    document_json: String,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    notes::cache_note_body(&conn, &id, &document_json)
}

#[tauri::command]
pub fn append_note_update(
    state: State<'_, AppState>,
    note_id: String,
    update_b64: String,
) -> Result<(), String> {
    let update = B64.decode(&update_b64).map_err(|e| e.to_string())?;
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    updates::append_update(&mut conn, &note_id, &update)
}

#[tauri::command]
pub fn note_updates(state: State<'_, AppState>, note_id: String) -> Result<Vec<String>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let blobs = updates::read_updates(&conn, &note_id)?;
    Ok(blobs.iter().map(|b| B64.encode(b)).collect())
}

#[tauri::command]
pub fn move_note(
    state: State<'_, AppState>,
    id: String,
    folder_id: Option<String>,
    sort_order: i64,
) -> Result<(), String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    notes::move_note(&mut conn, &mut hlc, &id, folder_id.as_deref(), sort_order)
}

#[tauri::command]
pub fn delete_note(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    notes::delete_note(&mut conn, &mut hlc, &id)
}

#[tauri::command]
pub fn create_folder(
    state: State<'_, AppState>,
    account_id: String,
    name: String,
) -> Result<String, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    folders::create_folder(&mut conn, &mut hlc, &account_id, &name)
}

#[tauri::command]
pub fn rename_folder(state: State<'_, AppState>, id: String, name: String) -> Result<(), String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    folders::rename_folder(&mut conn, &mut hlc, &id, &name)
}

#[tauri::command]
pub fn delete_folder(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    folders::delete_folder(&mut conn, &mut hlc, &id)
}
