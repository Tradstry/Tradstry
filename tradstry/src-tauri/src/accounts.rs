// Accounts API. Offline via the pull-only `accounts_cache` (refreshed by
// `sync::refresh_accounts_cache`); the frontend calls `invoke("accounts")`.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    id: String,
    name: String,
    broker: Option<String>,
    currency: Option<String>,
    icon: Option<String>,
}

/// Reads the local `accounts_cache` mirror — same shape the frontend's
/// `Account` type expects (`total_value`/`risk_profile` stay internal, used by
/// `dashboard::advanced_analytics` for equity, not returned here).
#[tauri::command]
pub fn accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, broker, currency, icon FROM accounts_cache ORDER BY name ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Account {
                id: r.get(0)?,
                name: r.get(1)?,
                broker: r.get(2)?,
                currency: r.get(3)?,
                icon: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
