mod accounts;
mod api;
mod auth;
mod db;
mod journal;
mod sync;

use std::sync::Mutex;

use rusqlite::Connection;
use tauri::Manager;

use crate::sync::hlc::Hlc;

/// Shared, locked handles to the local store and the Hybrid Logical Clock. Every
/// notebook command locks these; the outbox write and the HLC advance must be
/// serialized so stamps stay monotonic and the transaction stays atomic.
pub struct AppState {
    pub db: Mutex<Connection>,
    pub hlc: Mutex<Hlc>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_liquid_glass::init())
        .setup(|app| {
            let conn = db::client::open(app.handle())?;
            let client_id: String =
                conn.query_row("SELECT id FROM client LIMIT 1", [], |r| r.get(0))?;
            app.manage(AppState {
                db: Mutex::new(conn),
                hlc: Mutex::new(Hlc::new(client_id)),
            });
            sync::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth::sign_in,
            auth::auth_status,
            auth::sign_out,
            accounts::accounts,
            journal::dashboard::journal_analytics,
            journal::dashboard::calendar_analytics,
            journal::dashboard::advanced_analytics,
            journal::playbook::playbooks,
            journal::playbook::create_playbook,
            journal::playbook::update_playbook,
            journal::playbook::delete_playbook,
            journal::playbook::playbook_stats,
            journal::entries::journal_entries,
            journal::entries::create_journal_entry,
            journal::entries::update_journal_entry,
            journal::entries::delete_journal_entry,
            journal::entries::tags,
            journal::entries::tag_categories,
            journal::entries::create_tag,
            journal::entries::create_tag_category,
            journal::entries::rename_tag_category,
            journal::entries::set_tag_category_color,
            journal::entries::reorder_tag_categories,
            journal::entries::delete_tag_category,
            journal::entries::rename_tag,
            journal::entries::set_tag_color,
            journal::entries::delete_tag,
            journal::entries::merge_tags,
            journal::principle::principles,
            journal::principle::create_principle,
            journal::principle::update_principle,
            journal::principle::delete_principle,
            journal::principle::reorder_principles,
            journal::calculator::position_calculator_rule,
            journal::calculator::upsert_position_calculator_rule,
            journal::calculator::position_calculator_plans,
            journal::calculator::create_position_calculator_plan,
            journal::calculator::update_position_calculator_plan,
            journal::calculator::delete_position_calculator_plan,
            journal::calculator::position_calculator_history,
            journal::calculator::create_position_calculator_history,
            journal::calculator::delete_position_calculator_history,
            journal::notebook::notebook_notes,
            journal::notebook::notebook_folders,
            journal::notebook::create_note,
            journal::notebook::cache_note_body,
            journal::notebook::append_note_update,
            journal::notebook::note_updates,
            journal::notebook::move_note,
            journal::notebook::delete_note,
            journal::notebook::create_folder,
            journal::notebook::rename_folder,
            journal::notebook::delete_folder,
            journal::media::store_media,
            journal::media::resolve_media,
            journal::media::ensure_media,
            journal::media::delete_media,
            journal::media::save_media,
            sync::sync_now
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            sync::flush_on_quit(app_handle);
        }
    });
}
