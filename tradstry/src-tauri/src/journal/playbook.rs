// Offline-first playbook commands. Reads and writes go to the local SQLite store
// (db::playbook) and enqueue outbox mutations that the sync engine pushes; nothing
// here blocks on the network. `playbook_stats` is the one exception: the derived
// win-rate/P&L figures are computed from trades server-side, so they stay a
// best-effort ONLINE overlay that degrades to empty when offline.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::State;

use crate::AppState;
use crate::db::playbook::{self as store, PlaybookRow};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playbook {
    id: String,
    name: String,
    edge_name: String,
    entry_rules: String,
    exit_rules: String,
    position_sizing_rules: String,
    additional_rules: Option<String>,
    win_rate: f64,
    cumulative_profit: f64,
    average_gain: f64,
    average_loss: f64,
    trade_count: i64,
}

/// Local rows carry no stats (those need trades). Zero them; the UI overlays real
/// stats from `playbook_stats` when online.
fn to_playbook(row: PlaybookRow) -> Playbook {
    Playbook {
        id: row.id,
        name: row.name,
        edge_name: row.edge_name,
        entry_rules: row.entry_rules,
        exit_rules: row.exit_rules,
        position_sizing_rules: row.position_sizing_rules,
        additional_rules: row.additional_rules,
        win_rate: 0.0,
        cumulative_profit: 0.0,
        average_gain: 0.0,
        average_loss: 0.0,
        trade_count: 0,
    }
}

#[tauri::command]
pub fn playbooks(state: State<'_, AppState>) -> Result<Vec<Playbook>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(store::list_playbooks(&conn)?
        .into_iter()
        .map(to_playbook)
        .collect())
}

fn str_field(input: &Value, key: &str, fallback: &str) -> String {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| fallback.to_string())
}

#[tauri::command]
pub fn create_playbook(state: State<'_, AppState>, input: Value) -> Result<Playbook, String> {
    let mut row: PlaybookRow = serde_json::from_value(input).map_err(|e| e.to_string())?;
    row.id = String::new();
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    let id = store::create_playbook(&mut conn, &mut hlc, &row)?;
    row.id = id;
    Ok(to_playbook(row))
}

/// `input` may be a full form submit or a partial update (with optional
/// `clearAdditionalRules`). Merge it over the current local row so a partial
/// update never blanks untouched fields.
#[tauri::command]
pub fn update_playbook(
    state: State<'_, AppState>,
    id: String,
    input: Value,
) -> Result<Playbook, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;

    let current = store::find_playbook(&conn, &id)?.ok_or("playbook not found")?;
    let additional_rules = if input
        .get("clearAdditionalRules")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        None
    } else if let Some(v) = input.get("additionalRules") {
        v.as_str().map(|s| s.to_string()).filter(|s| !s.is_empty())
    } else {
        current.additional_rules.clone()
    };

    let merged = PlaybookRow {
        id: id.clone(),
        name: str_field(&input, "name", &current.name),
        edge_name: str_field(&input, "edgeName", &current.edge_name),
        entry_rules: str_field(&input, "entryRules", &current.entry_rules),
        exit_rules: str_field(&input, "exitRules", &current.exit_rules),
        position_sizing_rules: str_field(
            &input,
            "positionSizingRules",
            &current.position_sizing_rules,
        ),
        additional_rules,
    };
    store::update_playbook(&mut conn, &mut hlc, &merged)?;
    Ok(to_playbook(merged))
}

#[tauri::command]
pub fn delete_playbook(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    store::delete_playbook(&mut conn, &mut hlc, &id)?;
    Ok(true)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookStat {
    id: String,
    win_rate: f64,
    cumulative_profit: f64,
    average_gain: f64,
    average_loss: f64,
    trade_count: i64,
}

/// Best-effort online overlay. Returns an empty list (never an error) when
/// offline or signed out, so the UI just hides stats instead of breaking.
#[tauri::command]
pub async fn playbook_stats() -> Result<Vec<PlaybookStat>, String> {
    let query = "query PlaybookStats { playbooks { id winRate cumulativeProfit averageGain averageLoss tradeCount } }";
    match crate::api::graphql(query, json!({})).await {
        Ok(data) => {
            let node = data.get("playbooks").cloned().unwrap_or(Value::Null);
            Ok(serde_json::from_value(node).unwrap_or_default())
        }
        Err(_) => Ok(Vec::new()),
    }
}
