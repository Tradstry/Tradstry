// Local, offline-first trade store commands. Reads and writes for trades go
// to the local SQLite store (db::journal) and enqueue outbox mutations that
// the sync engine pushes; nothing here blocks on the network. `tags()` and
// `tag_categories()` read the local offline store (db::tags), synced back via
// a delta pull with LWW apply — so the trade form/table and the tag manager
// all work fully offline now.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::AppState;
use crate::db::journal::{self as store, JournalRow, JournalWriteArgs};
use crate::db::tags as tags_store;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    id: String,
    name: String,
    color: Option<String>,
    // Present on the standalone `tags` query; absent (None) when a Tag is
    // nested under a JournalEntry, which doesn't select it.
    #[serde(default)]
    category_id: Option<String>,
}

/// A user's tag category (Mistakes / Tactics / Edges + any custom).
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagCategory {
    id: String,
    name: String,
    role: Option<String>,
    color: Option<String>,
    sort_order: i64,
}

fn to_tag(row: tags_store::TagRow) -> Tag {
    Tag {
        id: row.id,
        name: row.name,
        color: row.color,
        category_id: Some(row.category_id),
    }
}

fn to_tag_category(row: tags_store::TagCategoryRow) -> TagCategory {
    TagCategory {
        id: row.id,
        name: row.name,
        role: row.role,
        color: row.color,
        sort_order: row.sort_order,
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    id: String,
    open_date: String,
    close_date: String,
    entry_price: f64,
    exit_price: f64,
    position_size: f64,
    stop_loss: f64,
    symbol: String,
    symbol_name: String,
    status: String,
    total_pl: f64,
    net_roi: f64,
    duration: i64,
    risk_reward: f64,
    trade_type: String,
    tags: Vec<Tag>,
    playbook_id: Option<String>,
    notes: Option<String>,
}

fn resolve_tags(conn: &Connection, tag_ids: &[String]) -> Vec<Tag> {
    tag_ids
        .iter()
        .filter_map(|id| {
            conn.query_row(
                "SELECT id, name, color, category_id FROM tags_cache WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| {
                    Ok(Tag {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        color: r.get(2)?,
                        category_id: r.get(3)?,
                    })
                },
            )
            .ok()
        })
        .collect()
}

fn to_journal_entry(conn: &Connection, row: JournalRow) -> JournalEntry {
    let tags = resolve_tags(conn, &row.tag_ids);
    JournalEntry {
        id: row.id,
        open_date: row.open_date,
        close_date: row.close_date,
        entry_price: row.entry_price,
        exit_price: row.exit_price,
        position_size: row.position_size,
        stop_loss: row.stop_loss.unwrap_or(0.0),
        symbol: row.symbol,
        symbol_name: row.symbol_name,
        status: row.status,
        total_pl: row.total_pl,
        net_roi: row.net_roi,
        duration: row.duration,
        risk_reward: row.risk_reward.unwrap_or(0.0),
        trade_type: row.trade_type,
        tags,
        playbook_id: row.playbook_id,
        notes: row.notes,
    }
}

#[tauri::command]
pub fn journal_entries(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<JournalEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let rows = store::list_journal_entries(&conn, &account_id)?;
    Ok(rows.into_iter().map(|r| to_journal_entry(&conn, r)).collect())
}

fn str_field(input: &Value, key: &str) -> Option<String> {
    input.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn f64_field(input: &Value, key: &str) -> Option<f64> {
    input.get(key).and_then(|v| v.as_f64())
}

fn i32_field(input: &Value, key: &str) -> Option<i32> {
    input.get(key).and_then(|v| v.as_i64()).map(|v| v as i32)
}

fn bool_field(input: &Value, key: &str) -> Option<bool> {
    input.get(key).and_then(|v| v.as_bool())
}

fn str_vec_field(input: &Value, key: &str) -> Option<Vec<String>> {
    input.get(key).and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    })
}

/// `input`: { accountId, openDate, closeDate, entryPrice, exitPrice, positionSize,
/// stopLoss, symbol, symbolName?, tradeType, tagIds[], playbookId?, notes? }.
#[tauri::command]
pub fn create_journal_entry(
    state: State<'_, AppState>,
    input: Value,
) -> Result<JournalEntry, String> {
    let args = JournalWriteArgs {
        id: String::new(),
        account_id: str_field(&input, "accountId").ok_or("accountId is required")?,
        open_date: str_field(&input, "openDate").ok_or("openDate is required")?,
        close_date: str_field(&input, "closeDate").ok_or("closeDate is required")?,
        entry_price: f64_field(&input, "entryPrice").ok_or("entryPrice is required")?,
        exit_price: f64_field(&input, "exitPrice").ok_or("exitPrice is required")?,
        position_size: f64_field(&input, "positionSize").ok_or("positionSize is required")?,
        stop_loss: f64_field(&input, "stopLoss"),
        symbol: str_field(&input, "symbol").ok_or("symbol is required")?,
        symbol_name: str_field(&input, "symbolName").unwrap_or_default(),
        trade_type: str_field(&input, "tradeType").ok_or("tradeType is required")?,
        playbook_id: str_field(&input, "playbookId"),
        notes: str_field(&input, "notes"),
        broke_30min_rule: bool_field(&input, "broke30MinRule"),
        pre_trade_conviction: i32_field(&input, "preTradeConviction"),
        market_regime: str_field(&input, "marketRegime"),
        is_planned_pre_market: bool_field(&input, "isPlannedPreMarket"),
        revenge_trade: bool_field(&input, "revengeTrade"),
        rule_adherence_score: i32_field(&input, "ruleAdherenceScore"),
        tag_ids: str_vec_field(&input, "tagIds").unwrap_or_default(),
        violated_principle_ids: str_vec_field(&input, "violatedPrincipleIds").unwrap_or_default(),
    };

    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    let id = store::create_journal_entry(&mut conn, &mut hlc, &args)?;
    let row = store::find_journal_entry(&conn, &id)?.ok_or("journal entry not found after create")?;
    Ok(to_journal_entry(&conn, row))
}

/// `input` is a partial update; merged over the current local row so
/// untouched fields (and `tagIds`) survive. `clearNotes`/`clearPlaybook`
/// blank those optional fields explicitly.
#[tauri::command]
pub fn update_journal_entry(
    state: State<'_, AppState>,
    id: String,
    input: Value,
) -> Result<JournalEntry, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;

    let current = store::find_journal_entry(&conn, &id)?.ok_or("journal entry not found")?;

    let notes = if bool_field(&input, "clearNotes").unwrap_or(false) {
        None
    } else {
        str_field(&input, "notes").or(current.notes.clone())
    };
    let playbook_id = if bool_field(&input, "clearPlaybook").unwrap_or(false) {
        None
    } else {
        str_field(&input, "playbookId").or(current.playbook_id.clone())
    };

    let args = JournalWriteArgs {
        id: id.clone(),
        account_id: current.account_id.clone(),
        open_date: str_field(&input, "openDate").unwrap_or(current.open_date.clone()),
        close_date: str_field(&input, "closeDate").unwrap_or(current.close_date.clone()),
        entry_price: f64_field(&input, "entryPrice").unwrap_or(current.entry_price),
        exit_price: f64_field(&input, "exitPrice").unwrap_or(current.exit_price),
        position_size: f64_field(&input, "positionSize").unwrap_or(current.position_size),
        stop_loss: f64_field(&input, "stopLoss").or(current.stop_loss),
        symbol: str_field(&input, "symbol").unwrap_or(current.symbol.clone()),
        symbol_name: str_field(&input, "symbolName").unwrap_or(current.symbol_name.clone()),
        trade_type: str_field(&input, "tradeType").unwrap_or(current.trade_type.clone()),
        playbook_id,
        notes,
        broke_30min_rule: bool_field(&input, "broke30MinRule").or(current.broke_30min_rule),
        pre_trade_conviction: i32_field(&input, "preTradeConviction")
            .or(current.pre_trade_conviction),
        market_regime: str_field(&input, "marketRegime").or(current.market_regime.clone()),
        is_planned_pre_market: bool_field(&input, "isPlannedPreMarket")
            .or(current.is_planned_pre_market),
        revenge_trade: bool_field(&input, "revengeTrade").or(current.revenge_trade),
        rule_adherence_score: i32_field(&input, "ruleAdherenceScore")
            .or(current.rule_adherence_score),
        tag_ids: str_vec_field(&input, "tagIds").unwrap_or(current.tag_ids.clone()),
        violated_principle_ids: str_vec_field(&input, "violatedPrincipleIds")
            .unwrap_or(current.violated_principle_ids.clone()),
    };

    store::update_journal_entry(&mut conn, &mut hlc, &args)?;
    let row = store::find_journal_entry(&conn, &id)?.ok_or("journal entry not found after update")?;
    Ok(to_journal_entry(&conn, row))
}

#[tauri::command]
pub fn delete_journal_entry(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    store::delete_journal_entry(&mut conn, &mut hlc, &id)?;
    Ok(true)
}

/// All live tags (for the trade tag pickers and the tag manager), read from
/// the local offline store `tags_cache`. Each tag carries its `categoryId` so
/// the per-category pickers can filter locally.
#[tauri::command]
pub fn tags(state: State<'_, AppState>) -> Result<Vec<Tag>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, color, category_id FROM tags_cache WHERE deleted_at IS NULL ORDER BY name ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
                category_id: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// The user's live tag categories, read from `tag_categories_cache` (the
/// backend seeds the three defaults — Mistakes / Tactics / Edges — server-
/// side; they show up here once synced).
#[tauri::command]
pub fn tag_categories(state: State<'_, AppState>) -> Result<Vec<TagCategory>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, role, color, sort_order FROM tag_categories_cache \
             WHERE deleted_at IS NULL ORDER BY sort_order ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(TagCategory {
                id: r.get(0)?,
                name: r.get(1)?,
                role: r.get(2)?,
                color: r.get(3)?,
                sort_order: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

// --- Tag & category management (local, offline-first) ---------------------
//
// Each command writes the local cache row(s) with a fresh HLC and enqueues the
// matching outbox mutation (see db::tags); the sync engine pushes it and pulls
// back deltas from every device with whole-row LWW.

/// Create a tag in a category (the inline "+ Add … tag" flow).
#[tauri::command]
pub fn create_tag(
    state: State<'_, AppState>,
    category_id: String,
    name: String,
    color: Option<String>,
) -> Result<Tag, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    let id = tags_store::create_tag(&mut conn, &mut hlc, &category_id, &name, color.as_deref())?;
    let row = tags_store::find_tag(&conn, &id)?.ok_or("tag not found after create")?;
    Ok(to_tag(row))
}

#[tauri::command]
pub fn create_tag_category(
    state: State<'_, AppState>,
    name: String,
    color: Option<String>,
) -> Result<TagCategory, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    let id = tags_store::create_category(&mut conn, &mut hlc, &name, color.as_deref())?;
    let row = tags_store::find_category(&conn, &id)?.ok_or("tag category not found after create")?;
    Ok(to_tag_category(row))
}

#[tauri::command]
pub fn rename_tag_category(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<TagCategory, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    tags_store::rename_category(&mut conn, &mut hlc, &id, &name)?;
    let row = tags_store::find_category(&conn, &id)?.ok_or("tag category not found")?;
    Ok(to_tag_category(row))
}

#[tauri::command]
pub fn set_tag_category_color(
    state: State<'_, AppState>,
    id: String,
    color: Option<String>,
) -> Result<TagCategory, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    tags_store::set_category_color(&mut conn, &mut hlc, &id, color.as_deref())?;
    let row = tags_store::find_category(&conn, &id)?.ok_or("tag category not found")?;
    Ok(to_tag_category(row))
}

/// `order`: `[{ id, sortOrder }]`, the full absolute ordering for every
/// category (matches the `reorderTagCategories` mutation shape).
#[tauri::command]
pub fn reorder_tag_categories(state: State<'_, AppState>, order: Value) -> Result<bool, String> {
    let pairs: Vec<(String, i64)> = order
        .as_array()
        .ok_or("order must be an array")?
        .iter()
        .map(|v| {
            let id = v
                .get("id")
                .and_then(|x| x.as_str())
                .ok_or("order entry missing id")?
                .to_string();
            let sort_order = v
                .get("sortOrder")
                .and_then(|x| x.as_i64())
                .ok_or("order entry missing sortOrder")?;
            Ok::<_, String>((id, sort_order))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    tags_store::reorder_categories(&mut conn, &mut hlc, &pairs)?;
    Ok(true)
}

/// Rejects (via `db::tags::delete_category`) when the category is a default
/// (`role IS NOT NULL`) — those are undeletable both locally and server-side.
#[tauri::command]
pub fn delete_tag_category(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    tags_store::delete_category(&mut conn, &mut hlc, &id)?;
    Ok(true)
}

#[tauri::command]
pub fn rename_tag(state: State<'_, AppState>, id: String, name: String) -> Result<Tag, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    tags_store::rename_tag(&mut conn, &mut hlc, &id, &name)?;
    let row = tags_store::find_tag(&conn, &id)?.ok_or("tag not found")?;
    Ok(to_tag(row))
}

#[tauri::command]
pub fn set_tag_color(
    state: State<'_, AppState>,
    id: String,
    color: Option<String>,
) -> Result<Tag, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    tags_store::set_tag_color(&mut conn, &mut hlc, &id, color.as_deref())?;
    let row = tags_store::find_tag(&conn, &id)?.ok_or("tag not found")?;
    Ok(to_tag(row))
}

#[tauri::command]
pub fn delete_tag(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    tags_store::delete_tag(&mut conn, &mut hlc, &id)?;
    Ok(true)
}

/// Soft-deletes `from_id` and repoints local trade `tag_ids` (see
/// `db::tags::merge_tags` for why the repoint is a pure cache fixup).
#[tauri::command]
pub fn merge_tags(state: State<'_, AppState>, from_id: String, into_id: String) -> Result<bool, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut hlc = state.hlc.lock().map_err(|e| e.to_string())?;
    tags_store::merge_tags(&mut conn, &mut hlc, &from_id, &into_id)?;
    Ok(true)
}
