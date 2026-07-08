mod accounts;
mod api;
mod auth;
mod journal;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_liquid_glass::init())
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
            journal::entries::merge_tags
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
