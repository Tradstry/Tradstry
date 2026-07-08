// Playbook API for the Journal app. Queries + mutations live here; the frontend
// calls invoke("playbooks" | "create_playbook" | "update_playbook" | "delete_playbook").

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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

const FIELDS: &str = "id name edgeName entryRules exitRules positionSizingRules additionalRules winRate cumulativeProfit averageGain averageLoss tradeCount";

fn one(data: Value, field: &str) -> Result<Playbook, String> {
    let node = data
        .get(field)
        .cloned()
        .ok_or_else(|| format!("missing {field} in response"))?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn playbooks() -> Result<Vec<Playbook>, String> {
    let query = format!("query Playbooks {{ playbooks {{ {FIELDS} }} }}");
    let data = crate::api::graphql(&query, Value::Null).await?;
    let node = data
        .get("playbooks")
        .cloned()
        .ok_or("missing playbooks in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

/// `input` is passed through: { name, edgeName, entryRules, exitRules,
/// positionSizingRules, additionalRules? }.
#[tauri::command]
pub async fn create_playbook(input: Value) -> Result<Playbook, String> {
    let query = format!(
        "mutation CreatePlaybook($input: CreatePlaybookInput!) {{ createPlaybook(input: $input) {{ {FIELDS} }} }}"
    );
    let data = crate::api::graphql(&query, json!({ "input": input })).await?;
    one(data, "createPlaybook")
}

/// `input`: any subset of the create fields, plus optional `clearAdditionalRules`.
#[tauri::command]
pub async fn update_playbook(id: String, input: Value) -> Result<Playbook, String> {
    let query = format!(
        "mutation UpdatePlaybook($id: String!, $input: UpdatePlaybookInput!) {{ updatePlaybook(id: $id, input: $input) {{ {FIELDS} }} }}"
    );
    let data = crate::api::graphql(&query, json!({ "id": id, "input": input })).await?;
    one(data, "updatePlaybook")
}

#[tauri::command]
pub async fn delete_playbook(id: String) -> Result<bool, String> {
    let query = "mutation DeletePlaybook($id: String!) { deletePlaybook(id: $id) }";
    let data = crate::api::graphql(query, json!({ "id": id })).await?;
    data.get("deletePlaybook")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| "missing deletePlaybook in response".to_string())
}
