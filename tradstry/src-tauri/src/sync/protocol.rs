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
    folders { id parentFolderId name sortOrder isSystem hlc deletedAt updatedAt }
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
    #[serde(default)]
    pub is_system: bool,
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

const PULL_PLAYBOOK: &str = r#"
query PullPlaybook($cookie: String, $clientId: String!) {
  pullPlaybook(cookie: $cookie, clientId: $clientId) {
    cookie
    lastMutationId
    playbooks {
      id name edgeName entryRules exitRules positionSizingRules additionalRules
      hlc deletedAt updatedAt
    }
  }
}
"#;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WirePlaybook {
    pub id: String,
    pub name: String,
    pub edge_name: String,
    pub entry_rules: String,
    pub exit_rules: String,
    pub position_sizing_rules: String,
    pub additional_rules: Option<String>,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookPullResult {
    pub cookie: Option<String>,
    pub last_mutation_id: i64,
    pub playbooks: Vec<WirePlaybook>,
}

/// User-scoped pull of playbook deltas changed since `cookie`. Playbooks have no
/// account, so this is a separate channel from `pull` with its own cursor.
pub async fn pull_playbook(
    client_id: &str,
    cookie: Option<&str>,
) -> Result<PlaybookPullResult, String> {
    let variables = json!({ "cookie": cookie, "clientId": client_id });
    let data = crate::api::graphql(PULL_PLAYBOOK, variables).await?;
    let node = data
        .get("pullPlaybook")
        .cloned()
        .ok_or("missing pullPlaybook in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

const PULL_JOURNAL: &str = r#"
query PullJournal($cookie: String, $accountId: String!, $clientId: String!) {
  pullJournal(cookie: $cookie, accountId: $accountId, clientId: $clientId) {
    cookie
    lastMutationId
    entries {
      id openDate closeDate entryPrice exitPrice positionSize stopLoss symbol symbolName
      status totalPl netRoi duration riskReward tradeType playbookId notes broke30MinRule
      preTradeConviction marketRegime isPlannedPreMarket revengeTrade ruleAdherenceScore
      tagIds hlc deletedAt updatedAt
    }
  }
}
"#;

// NOTE: the server's `JournalEntryDeltaGql` (backend/src/graphql/journal.rs) has no
// `accountId` field — the pull is already account-scoped by the `accountId` argument,
// so each row doesn't repeat it. `apply_journal` takes `account_id` as a separate
// parameter (like `apply_folder` does) instead of reading it off the wire type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireJournalEntry {
    pub id: String,
    pub open_date: String,
    pub close_date: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub position_size: f64,
    pub stop_loss: Option<f64>,
    pub symbol: String,
    pub symbol_name: String,
    pub status: String,
    pub total_pl: f64,
    pub net_roi: f64,
    pub duration: i64,
    pub risk_reward: Option<f64>,
    pub trade_type: String,
    pub playbook_id: Option<String>,
    pub notes: Option<String>,
    pub broke_30min_rule: Option<bool>,
    pub pre_trade_conviction: Option<i32>,
    pub market_regime: Option<String>,
    pub is_planned_pre_market: Option<bool>,
    pub revenge_trade: Option<bool>,
    pub rule_adherence_score: Option<i32>,
    pub tag_ids: Vec<String>,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalPullResult {
    pub cookie: Option<String>,
    pub last_mutation_id: i64,
    pub entries: Vec<WireJournalEntry>,
}

/// Account-scoped pull of journal-entry deltas changed since `cookie`. Journal
/// entries belong to one account (unlike playbooks), so this carries `account_id`
/// and gets its own per-account cursor (`journal_sync`), never the notebook cookie.
pub async fn pull_journal(
    client_id: &str,
    account_id: &str,
    cookie: Option<&str>,
) -> Result<JournalPullResult, String> {
    let variables = json!({
        "cookie": cookie,
        "accountId": account_id,
        "clientId": client_id,
    });
    let data = crate::api::graphql(PULL_JOURNAL, variables).await?;
    let node = data
        .get("pullJournal")
        .cloned()
        .ok_or("missing pullJournal in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

const PULL_PRINCIPLE: &str = r#"
query PullPrinciple($cookie: String, $accountId: String!, $clientId: String!) {
  pullPrinciple(cookie: $cookie, accountId: $accountId, clientId: $clientId) {
    cookie
    lastMutationId
    principles {
      id accountId playbookId evidenceNoteId title theRule why intervention priority isActive
      hlc deletedAt updatedAt
    }
  }
}
"#;

// NOTE: unlike `WireJournalEntry`, the server's `PrincipleDeltaGql` DOES repeat
// `accountId` on every row (the plan's server contract). `apply_principle`
// still takes `account_id` as a separate parameter (matching `apply_journal`),
// so this field is present for wire parity but not read by the apply path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WirePrinciple {
    pub id: String,
    pub account_id: String,
    pub playbook_id: Option<String>,
    pub evidence_note_id: Option<String>,
    pub title: String,
    pub the_rule: String,
    pub why: String,
    pub intervention: Option<String>,
    pub priority: i64,
    pub is_active: bool,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinciplePullResult {
    pub cookie: Option<String>,
    pub last_mutation_id: i64,
    pub principles: Vec<WirePrinciple>,
}

/// Account-scoped pull of principle deltas changed since `cookie`. Principles
/// are account-scoped like journal entries, so this carries `account_id` and
/// gets its own per-account cursor (`principle_sync`), never the notebook or
/// journal cookie.
pub async fn pull_principle(
    client_id: &str,
    account_id: &str,
    cookie: Option<&str>,
) -> Result<PrinciplePullResult, String> {
    let variables = json!({
        "cookie": cookie,
        "accountId": account_id,
        "clientId": client_id,
    });
    let data = crate::api::graphql(PULL_PRINCIPLE, variables).await?;
    let node = data
        .get("pullPrinciple")
        .cloned()
        .ok_or("missing pullPrinciple in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

const PULL_TAGS_SYNC: &str = r#"
query PullTags($cookie: String, $clientId: String!) {
  pullTags(cookie: $cookie, clientId: $clientId) {
    cookie
    lastMutationId
    categories { id name role color sortOrder hlc deletedAt updatedAt }
    tags { id categoryId name color hlc deletedAt updatedAt }
  }
}
"#;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireTagCategoryDelta {
    pub id: String,
    pub name: String,
    pub role: Option<String>,
    pub color: Option<String>,
    pub sort_order: i64,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireTagDelta {
    pub id: String,
    pub category_id: String,
    pub name: String,
    pub color: Option<String>,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagsPullResult {
    pub cookie: Option<String>,
    pub last_mutation_id: i64,
    pub categories: Vec<WireTagCategoryDelta>,
    pub tags: Vec<WireTagDelta>,
}

/// User-scoped delta pull of tag/category changes since `cookie`. Tags have no
/// account (like playbooks), so this carries its own cursor (`tag_sync`),
/// separate from the notebook/journal/playbook cursors.
pub async fn pull_tags_sync(
    client_id: &str,
    cookie: Option<&str>,
) -> Result<TagsPullResult, String> {
    let variables = json!({ "cookie": cookie, "clientId": client_id });
    let data = crate::api::graphql(PULL_TAGS_SYNC, variables).await?;
    let node = data
        .get("pullTags")
        .cloned()
        .ok_or("missing pullTags in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

const PULL_CALCULATOR: &str = r#"
query PullCalculator($cookie: String, $clientId: String!) {
  pullCalculator(cookie: $cookie, clientId: $clientId) {
    cookie
    lastMutationId
    rules { id accountId accountBalance accountRisk maxStopLossPct hlc deletedAt updatedAt }
    plans {
      id symbol positionType entryPrice stopLoss accountBalance accountRisk totalShares
      positionValue status tranchesJson notes hlc deletedAt updatedAt
    }
    history {
      id symbol positionType entryPrice stopLoss accountBalance accountRisk shares
      positionValue accountPct stopLossPct hlc deletedAt updatedAt
    }
  }
}
"#;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireRule {
    pub id: String,
    pub account_id: String,
    pub account_balance: f64,
    pub account_risk: f64,
    pub max_stop_loss_pct: f64,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WirePlan {
    pub id: String,
    pub symbol: String,
    pub position_type: String,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub account_balance: f64,
    pub account_risk: f64,
    pub total_shares: f64,
    pub position_value: f64,
    pub status: String,
    pub tranches_json: String,
    pub notes: Option<String>,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireHistory {
    pub id: String,
    pub symbol: String,
    pub position_type: String,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub account_balance: f64,
    pub account_risk: f64,
    pub shares: f64,
    pub position_value: f64,
    pub account_pct: f64,
    pub stop_loss_pct: f64,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalculatorPullResult {
    pub cookie: Option<String>,
    pub last_mutation_id: i64,
    pub rules: Vec<WireRule>,
    pub plans: Vec<WirePlan>,
    pub history: Vec<WireHistory>,
}

/// User-scoped delta pull of calculator rules/plans/history since `cookie`.
/// All three entities share one cursor/cookie (`calculator_sync`), like tags'
/// categories+tags — none of them are account-scoped in the pull itself, even
/// though each `WireRule` carries its own `accountId`.
pub async fn pull_calculator(
    client_id: &str,
    cookie: Option<&str>,
) -> Result<CalculatorPullResult, String> {
    let variables = json!({ "cookie": cookie, "clientId": client_id });
    let data = crate::api::graphql(PULL_CALCULATOR, variables).await?;
    let node = data
        .get("pullCalculator")
        .cloned()
        .ok_or("missing pullCalculator in response")?;
    serde_json::from_value(node).map_err(|e| e.to_string())
}

const PULL_ACCOUNTS: &str = r#"
query PullAccounts {
  accounts { id name broker currency icon totalValue riskProfile }
}
"#;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireAccount {
    pub id: String,
    pub name: String,
    pub broker: Option<String>,
    pub currency: Option<String>,
    pub icon: Option<String>,
    pub total_value: Option<f64>,
    pub risk_profile: Option<String>,
}

/// Pull-only cache refresh: the full account list, refreshed wholesale each
/// cycle (no cursor — the server has no delta query for accounts).
pub async fn pull_accounts() -> Result<Vec<WireAccount>, String> {
    let data = crate::api::graphql(PULL_ACCOUNTS, json!({})).await?;
    let node = data.get("accounts").cloned().ok_or("missing accounts in response")?;
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
