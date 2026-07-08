// Journal entries (trades) API. Queries + mutations live here; the frontend
// calls invoke("journal_entries" | "create_journal_entry" | "update_journal_entry"
// | "delete_journal_entry" | "tags").

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Deserialize)]
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

const FIELDS: &str = "id openDate closeDate entryPrice exitPrice positionSize stopLoss symbol symbolName status totalPl netRoi duration riskReward tradeType tags { id name color categoryId } playbookId notes";

fn one(data: Value, field: &str) -> Result<JournalEntry, String> {
    let node = data
        .get(field)
        .cloned()
        .ok_or_else(|| format!("missing {field} in response"))?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn journal_entries() -> Result<Vec<JournalEntry>, String> {
    let query = format!("query JournalEntries {{ journalEntries {{ {FIELDS} }} }}");
    let data = crate::api::graphql(&query, Value::Null).await?;
    let node = data
        .get("journalEntries")
        .cloned()
        .ok_or("missing journalEntries in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

/// `input`: { accountId, openDate, closeDate, entryPrice, exitPrice, positionSize,
/// stopLoss, symbol, symbolName?, tradeType, tagIds[], playbookId?, notes? }.
#[tauri::command]
pub async fn create_journal_entry(input: Value) -> Result<JournalEntry, String> {
    let query = format!(
        "mutation CreateJournalEntry($input: CreateJournalEntryInput!) {{ createJournalEntry(input: $input) {{ {FIELDS} }} }}"
    );
    let data = crate::api::graphql(&query, json!({ "input": input })).await?;
    one(data, "createJournalEntry")
}

#[tauri::command]
pub async fn update_journal_entry(id: String, input: Value) -> Result<JournalEntry, String> {
    let query = format!(
        "mutation UpdateJournalEntry($id: String!, $input: UpdateJournalEntryInput!) {{ updateJournalEntry(id: $id, input: $input) {{ {FIELDS} }} }}"
    );
    let data = crate::api::graphql(&query, json!({ "id": id, "input": input })).await?;
    one(data, "updateJournalEntry")
}

#[tauri::command]
pub async fn delete_journal_entry(id: String) -> Result<bool, String> {
    let query = "mutation DeleteJournalEntry($id: String!) { deleteJournalEntry(id: $id) }";
    let data = crate::api::graphql(query, json!({ "id": id })).await?;
    data.get("deleteJournalEntry")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| "missing deleteJournalEntry in response".to_string())
}

/// All tags for the current user (for the trade tag pickers). Each tag carries
/// its `categoryId` so the per-category pickers can filter locally.
#[tauri::command]
pub async fn tags() -> Result<Vec<Tag>, String> {
    let query = "query Tags { tags { id name color categoryId } }";
    let data = crate::api::graphql(query, Value::Null).await?;
    let node = data.get("tags").cloned().ok_or("missing tags in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

/// The user's tag categories. The backend seeds the three defaults
/// (Mistakes / Tactics / Edges) on first call.
#[tauri::command]
pub async fn tag_categories() -> Result<Vec<TagCategory>, String> {
    let query = "query TagCategories { tagCategories { id name role color sortOrder } }";
    let data = crate::api::graphql(query, Value::Null).await?;
    let node = data
        .get("tagCategories")
        .cloned()
        .ok_or("missing tagCategories in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

/// Create a tag in a category (the inline "+ Add … tag" flow).
#[tauri::command]
pub async fn create_tag(
    category_id: String,
    name: String,
    color: Option<String>,
) -> Result<Tag, String> {
    let query = "mutation CreateTag($categoryId: String!, $name: String!, $color: String) { createTag(categoryId: $categoryId, name: $name, color: $color) { id name color categoryId } }";
    let data = crate::api::graphql(
        query,
        json!({ "categoryId": category_id, "name": name, "color": color }),
    )
    .await?;
    let node = data
        .get("createTag")
        .cloned()
        .ok_or("missing createTag in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

// --- Tag & category management (for the tag manager) ----------------------

const CAT_FIELDS: &str = "id name role color sortOrder";
const TAG_FIELDS: &str = "id name color categoryId";

/// Deserialize a named field of a GraphQL `data` object into `T`.
fn de<T: serde::de::DeserializeOwned>(data: Value, field: &str) -> Result<T, String> {
    let n = data
        .get(field)
        .cloned()
        .ok_or_else(|| format!("missing {field} in response"))?;
    serde_json::from_value(n).map_err(|e| e.to_string())
}

/// Read a boolean mutation result.
fn flag(data: Value, field: &str) -> Result<bool, String> {
    data.get(field)
        .and_then(|v| v.as_bool())
        .ok_or_else(|| format!("missing {field} in response"))
}

#[tauri::command]
pub async fn create_tag_category(
    name: String,
    color: Option<String>,
) -> Result<TagCategory, String> {
    let query = format!(
        "mutation CreateTagCategory($name: String!, $color: String) {{ createTagCategory(name: $name, color: $color) {{ {CAT_FIELDS} }} }}"
    );
    let data = crate::api::graphql(&query, json!({ "name": name, "color": color })).await?;
    de(data, "createTagCategory")
}

#[tauri::command]
pub async fn rename_tag_category(id: String, name: String) -> Result<TagCategory, String> {
    let query = format!(
        "mutation RenameTagCategory($id: String!, $name: String!) {{ renameTagCategory(id: $id, name: $name) {{ {CAT_FIELDS} }} }}"
    );
    let data = crate::api::graphql(&query, json!({ "id": id, "name": name })).await?;
    de(data, "renameTagCategory")
}

#[tauri::command]
pub async fn set_tag_category_color(
    id: String,
    color: Option<String>,
) -> Result<TagCategory, String> {
    let query = format!(
        "mutation SetTagCategoryColor($id: String!, $color: String) {{ setTagCategoryColor(id: $id, color: $color) {{ {CAT_FIELDS} }} }}"
    );
    let data = crate::api::graphql(&query, json!({ "id": id, "color": color })).await?;
    de(data, "setTagCategoryColor")
}

#[tauri::command]
pub async fn reorder_tag_categories(order: Value) -> Result<bool, String> {
    let query = "mutation ReorderTagCategories($order: [ReorderTagCategoryInput!]!) { reorderTagCategories(order: $order) }";
    let data = crate::api::graphql(query, json!({ "order": order })).await?;
    flag(data, "reorderTagCategories")
}

#[tauri::command]
pub async fn delete_tag_category(id: String) -> Result<bool, String> {
    let query = "mutation DeleteTagCategory($id: String!) { deleteTagCategory(id: $id) }";
    let data = crate::api::graphql(query, json!({ "id": id })).await?;
    flag(data, "deleteTagCategory")
}

#[tauri::command]
pub async fn rename_tag(id: String, name: String) -> Result<Tag, String> {
    let query = format!(
        "mutation RenameTag($id: String!, $name: String!) {{ renameTag(id: $id, name: $name) {{ {TAG_FIELDS} }} }}"
    );
    let data = crate::api::graphql(&query, json!({ "id": id, "name": name })).await?;
    de(data, "renameTag")
}

#[tauri::command]
pub async fn set_tag_color(id: String, color: Option<String>) -> Result<Tag, String> {
    let query = format!(
        "mutation SetTagColor($id: String!, $color: String) {{ setTagColor(id: $id, color: $color) {{ {TAG_FIELDS} }} }}"
    );
    let data = crate::api::graphql(&query, json!({ "id": id, "color": color })).await?;
    de(data, "setTagColor")
}

#[tauri::command]
pub async fn delete_tag(id: String) -> Result<bool, String> {
    let query = "mutation DeleteTag($id: String!) { deleteTag(id: $id) }";
    let data = crate::api::graphql(query, json!({ "id": id })).await?;
    flag(data, "deleteTag")
}

#[tauri::command]
pub async fn merge_tags(from_id: String, into_id: String) -> Result<bool, String> {
    let query = "mutation MergeTags($fromId: String!, $intoId: String!) { mergeTags(fromId: $fromId, intoId: $intoId) }";
    let data =
        crate::api::graphql(query, json!({ "fromId": from_id, "intoId": into_id })).await?;
    flag(data, "mergeTags")
}
