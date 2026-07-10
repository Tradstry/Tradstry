//! Push/pull transport for the notebook sync protocol. Pure network plumbing:
//! no SQLite access, no merge logic — those are the caller's job (Task 11).

// First consumer lands in Task 11; until then this module holds no callers.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const PUSH: &str = r#"
mutation PushNotebook($input: NotebookPushInput!) {
  pushNotebook(input: $input) { lastMutationId }
}
"#;

const PULL: &str = r#"
query PullNotebook($cookie: String, $accountId: String!, $clientId: String!) {
  pullNotebook(cookie: $cookie, accountId: $accountId, clientId: $clientId) {
    cookie
    lastMutationId
    notes   { id folderId title documentJson sortOrder tradeIds hlc deletedAt updatedAt }
    folders { id parentFolderId name sortOrder hlc deletedAt updatedAt }
  }
}
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxRow {
    #[serde(rename = "id")]
    pub mutation_id: i64,
    pub name: String,
    pub args: String,
    pub hlc: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireNote {
    pub id: String,
    pub folder_id: Option<String>,
    pub title: String,
    pub document_json: String,
    pub sort_order: i64,
    pub trade_ids: Vec<String>,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireFolder {
    pub id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullResult {
    // Opaque: store it and send it back verbatim on the next pull. Never
    // parse or compare it — the server owns its meaning and may change it.
    pub cookie: Option<String>,
    pub last_mutation_id: i64,
    pub notes: Vec<WireNote>,
    pub folders: Vec<WireFolder>,
}

/// Pushes `mutations` (ordered by `mutation_id` ascending) and returns the
/// server-acknowledged `lastMutationId`. An empty slice is a no-op: it never
/// touches the network and returns `Ok(0)`, which is not a real ack — callers
/// must not persist it as a new watermark, only skip the round-trip.
pub async fn push(
    client_id: &str,
    account_id: &str,
    mutations: &[OutboxRow],
) -> Result<i64, String> {
    if mutations.is_empty() {
        return Ok(0);
    }

    let mut ordered: Vec<&OutboxRow> = mutations.iter().collect();
    ordered.sort_by_key(|m| m.mutation_id);

    let variables = json!({
        "input": {
            "clientId": client_id,
            "accountId": account_id,
            "mutations": ordered,
        }
    });

    let data = crate::api::graphql(PUSH, variables).await?;
    data.get("pushNotebook")
        .and_then(|v| v.get("lastMutationId"))
        .and_then(Value::as_i64)
        .ok_or_else(|| "missing lastMutationId in response".to_string())
}

/// Pulls notes/folders changed since `cookie` (`None` for a first sync).
/// Tombstoned rows (non-null `deletedAt`) are included, not filtered — the
/// caller needs them to distinguish "deleted remotely" from "not yet pushed".
///
/// `client_id` scopes the returned `lastMutationId` to THIS device. A user-wide
/// watermark would over-report once a second device exists.
pub async fn pull(
    client_id: &str,
    account_id: &str,
    cookie: Option<&str>,
) -> Result<PullResult, String> {
    let variables = json!({
        "cookie": cookie,
        "accountId": account_id,
        "clientId": client_id,
    });

    let data = crate::api::graphql(PULL, variables).await?;
    let node = data
        .get("pullNotebook")
        .cloned()
        .ok_or("missing pullNotebook in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

const PULL_UPDATES: &str = r#"
query NotebookAccountUpdatesSince($accountId: String!, $sinceSeq: Int!) {
  notebookAccountUpdatesSince(accountId: $accountId, sinceSeq: $sinceSeq) {
    noteId
    seq
    update
  }
}
"#;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteUpdate {
    pub note_id: String,
    pub seq: i64,
    /// base64; decoded by the caller straight into a SQLite BLOB.
    pub update: String,
}

/// Every Yjs update in the account with `seq > since_seq`, oldest first. One
/// round-trip covers all notes: `seq` is a single global sequence.
pub async fn pull_updates(account_id: &str, since_seq: i64) -> Result<Vec<RemoteUpdate>, String> {
    let variables = json!({
        "accountId": account_id,
        "sinceSeq": since_seq,
    });

    let data = crate::api::graphql(PULL_UPDATES, variables).await?;
    let node = data
        .get("notebookAccountUpdatesSince")
        .cloned()
        .ok_or("missing notebookAccountUpdatesSince in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}
