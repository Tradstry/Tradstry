// Accounts API. Query lives in Rust; the frontend calls `invoke("accounts")`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    id: String,
    name: String,
    broker: Option<String>,
    currency: Option<String>,
    icon: Option<String>,
}

const ACCOUNTS_QUERY: &str = r#"
query Accounts {
  accounts {
    id
    name
    broker
    currency
    icon
  }
}
"#;

#[tauri::command]
pub async fn accounts() -> Result<Vec<Account>, String> {
    let data = crate::api::graphql(ACCOUNTS_QUERY, Value::Null).await?;
    let node = data.get("accounts").cloned().ok_or("missing accounts in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}
