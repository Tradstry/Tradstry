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
            journal::dashboard::calendar_analytics
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
