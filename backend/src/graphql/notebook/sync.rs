//! Replicache-style push/pull for the notebook.
//!
//! Push is idempotent by per-client sequential mutation id: the server ignores
//! any mutation whose id <= last_mutation_id. That converts an at-least-once
//! delivery channel into exactly-once *application*. Exactly-once *delivery* is
//! impossible over an unreliable channel (Two Generals).

use anyhow::{Result, bail};
use async_graphql::{Context, InputObject, Object, SimpleObject};
use serde::Deserialize;
use sqlx::{PgConnection, PgPool};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

use crate::service::db::schema::tables::journal_table;
use crate::service::db::schema::tables::notebook::crdt;
use crate::service::db::schema::tables::notebook::folders::{
    self, CreateNotebookFolderInput, MoveNotebookNodeInput, NotebookNodeType,
};
use crate::service::db::schema::tables::notebook::notes::{self, CreateNotebookNoteInput};
use crate::service::db::schema::tables::notebook::sync::{
    self, NotebookFolderDelta, NotebookNoteDelta,
};
use crate::service::db::schema::tables::playbook_table;
use crate::service::db::schema::tables::position_calculator_history_table;
use crate::service::db::schema::tables::position_calculator_plans_table;
use crate::service::db::schema::tables::position_calculator_rule_table;
use crate::service::db::schema::tables::tags_table;
use crate::service::db::schema::tables::trading_principle_table;

/// `graphql(name)` is required: `notebook::NotebookMutation` is the notebook's
/// mutation root, and async-graphql panics at schema build on a duplicate type name.
#[derive(InputObject, Clone)]
#[graphql(name = "NotebookMutationInput")]
pub struct NotebookMutation {
    pub id: i64,
    pub name: String,
    /// JSON payload. Every generated id and timestamp is inside here, because
    /// mutations are replayed during rebase and must be deterministic.
    pub args: String,
    pub hlc: String,
}

#[derive(InputObject)]
pub struct NotebookPushInput {
    pub client_id: String,
    pub account_id: String,
    pub mutations: Vec<NotebookMutation>,
}

#[derive(SimpleObject)]
pub struct NotebookPushResult {
    pub last_mutation_id: i64,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotebookNoteDeltaGql {
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

impl From<NotebookNoteDelta> for NotebookNoteDeltaGql {
    fn from(d: NotebookNoteDelta) -> Self {
        Self {
            id: d.id,
            folder_id: d.folder_id,
            title: d.title,
            document_json: d.document_json,
            sort_order: d.sort_order,
            trade_ids: d.trade_ids,
            hlc: d.hlc,
            deleted_at: d.deleted_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotebookFolderDeltaGql {
    pub id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

impl From<NotebookFolderDelta> for NotebookFolderDeltaGql {
    fn from(d: NotebookFolderDelta) -> Self {
        Self {
            id: d.id,
            parent_folder_id: d.parent_folder_id,
            name: d.name,
            sort_order: d.sort_order,
            hlc: d.hlc,
            deleted_at: d.deleted_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotebookPullResult {
    pub cookie: String,
    pub last_mutation_id: i64,
    pub notes: Vec<NotebookNoteDeltaGql>,
    pub folders: Vec<NotebookFolderDeltaGql>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateNoteArgs {
    id: String,
    account_id: String,
    document_json: String,
    #[serde(default)]
    trade_ids: Vec<String>,
    folder_id: Option<String>,
    /// base64 of the Y.Doc update the creating device minted for this note. The
    /// creator seeds; a note carrying one must not be seeded again by the server.
    seed_update: Option<String>,
    seed_state_vector: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppendNoteUpdateArgs {
    note_id: String,
    /// Standard padded base64. The desktop encodes with the same engine.
    update: String,
}

#[derive(Deserialize)]
struct IdArgs {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFolderArgs {
    id: String,
    account_id: String,
    name: String,
    parent_folder_id: Option<String>,
    sort_order: i64,
}

#[derive(Deserialize)]
struct RenameFolderArgs {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveNodeArgs {
    account_id: String,
    node_id: String,
    node_type: String,
    new_parent_folder_id: Option<String>,
    new_sort_order: i64,
}

/// Applies one mutation and returns the client's new `last_mutation_id`.
///
/// Both the effect and the cursor advance inside ONE transaction. Committing a
/// dedupe key separately from the operation leaves a TOCTOU window in which a
/// concurrent retry double-applies.
pub async fn apply_mutation(
    pool: &PgPool,
    user_id: &str,
    client_id: &str,
    m: &NotebookMutation,
) -> Result<i64> {
    let mut tx = pool.begin().await?;

    let last = sync::last_mutation_id(&mut tx, client_id, user_id).await?;
    if m.id <= last {
        tx.commit().await?;
        return Ok(last); // Already applied. Idempotent replay of an at-least-once delivery.
    }

    // A mutation that can never succeed must still be acknowledged, or the client
    // retries it forever and deadlocks its queue behind it. A SAVEPOINT isolates a
    // failed effect so the watermark can still advance in this same transaction.
    sqlx::query("SAVEPOINT apply_effect")
        .execute(&mut *tx)
        .await?;
    let effect_ok = match apply_effect(&mut tx, user_id, m).await {
        Ok(()) => {
            sqlx::query("RELEASE SAVEPOINT apply_effect")
                .execute(&mut *tx)
                .await?;
            true
        }
        Err(err) => {
            log::warn!(
                "dropping permanently invalid notebook mutation (client={client_id}, id={}, name={}): {err}",
                m.id,
                m.name
            );
            sqlx::query("ROLLBACK TO SAVEPOINT apply_effect")
                .execute(&mut *tx)
                .await?;
            false
        }
    };

    sync::advance_mutation_id(&mut tx, client_id, user_id, m.id).await?;
    tx.commit().await?;

    // Only seed here when the creator did not: the web resolver's notes arrive with
    // no seed, the desktop's carry their own. Seeding a note twice concatenates the
    // two Y.Docs, silently duplicating every paragraph.
    if effect_ok && m.name == "createNote" {
        let a: CreateNoteArgs = serde_json::from_str(&m.args)?;
        if a.seed_update.is_none() {
            seed_new_note(pool, &a.id).await;
        }
    }
    Ok(m.id)
}

/// Every note is born `crdt`. Seeding runs after the note's transaction commits,
/// because the projector is a subprocess and would pin a connection for the whole
/// ~130ms it takes.
///
/// A failure here is logged, not returned: the note row already exists, so failing
/// the caller would strand it. `seed_note` claims `seeding` before it can fail, and
/// `sweep_stale_seeding` re-drives anything left in that state.
pub async fn seed_new_note(pool: &PgPool, note_id: &str) {
    if let Err(error) = crdt::seed_note(pool, note_id).await {
        log::error!("note {note_id}: seeding failed, sweeper will retry: {error:#}");
    }
}

async fn apply_effect(conn: &mut PgConnection, user_id: &str, m: &NotebookMutation) -> Result<()> {
    match m.name.as_str() {
        "createNote" => {
            let a: CreateNoteArgs = serde_json::from_str(&m.args)?;
            let note_id = a.id.clone();
            notes::create_notebook_note_tx(
                conn,
                user_id,
                CreateNotebookNoteInput {
                    id: Some(a.id),
                    account_id: a.account_id,
                    document_json: a.document_json,
                    trade_ids: a.trade_ids,
                    folder_id: a.folder_id,
                },
                &m.hlc,
            )
            .await?;

            // The creator seeds. Installing it in this same transaction means the
            // note is never briefly `crdt` with an empty update log, which another
            // client would read as a legacy note.
            if let Some(seed) = a.seed_update {
                let update = BASE64.decode(seed.trim())?;
                let state_vector = match a.seed_state_vector {
                    Some(sv) => BASE64.decode(sv.trim())?,
                    None => bail!("createNote carried a seed update with no state vector"),
                };
                crdt::install_seed_tx(conn, &note_id, &update, &state_vector).await?;
            }
        }
        "appendNoteUpdate" => {
            let a: AppendNoteUpdateArgs = serde_json::from_str(&m.args)?;
            use anyhow::Context as _;
            use base64::Engine as _;
            let update = base64::engine::general_purpose::STANDARD
                .decode(&a.update)
                .context("appendNoteUpdate.update must be standard base64")?;
            super::crdt::append_updates_tx(
                conn,
                user_id,
                &a.note_id,
                std::slice::from_ref(&update),
            )
            .await?;
        }
        "deleteNote" => {
            let a: IdArgs = serde_json::from_str(&m.args)?;
            notes::delete_notebook_note_tx(conn, &a.id, user_id, &m.hlc).await?;
        }
        "createFolder" => {
            let a: CreateFolderArgs = serde_json::from_str(&m.args)?;
            folders::create_notebook_folder_tx(
                conn,
                CreateNotebookFolderInput {
                    id: Some(a.id),
                    user_id: user_id.to_string(),
                    account_id: a.account_id,
                    parent_folder_id: a.parent_folder_id,
                    name: a.name,
                },
                a.sort_order,
                &m.hlc,
            )
            .await?;
        }
        "renameFolder" => {
            let a: RenameFolderArgs = serde_json::from_str(&m.args)?;
            folders::rename_notebook_folder_tx(conn, &a.id, &a.name, &m.hlc).await?;
        }
        "deleteFolder" => {
            let a: IdArgs = serde_json::from_str(&m.args)?;
            folders::delete_notebook_folder_subtree_tx(conn, &a.id, &m.hlc).await?;
        }
        "moveNode" => {
            let a: MoveNodeArgs = serde_json::from_str(&m.args)?;
            let node_type = match a.node_type.as_str() {
                "folder" | "Folder" => NotebookNodeType::Folder,
                "note" | "Note" => NotebookNodeType::Note,
                other => anyhow::bail!("unknown node type '{other}'"),
            };
            folders::move_notebook_node_tx(
                conn,
                MoveNotebookNodeInput {
                    account_id: a.account_id,
                    node_id: a.node_id,
                    node_type,
                    new_parent_folder_id: a.new_parent_folder_id,
                    new_sort_order: a.new_sort_order,
                },
                &m.hlc,
            )
            .await?;
        }
        "createPlaybook" => {
            let a: PlaybookArgs = serde_json::from_str(&m.args)?;
            playbook_table::create_playbook_tx(conn, user_id, &a.into_write_args(), &m.hlc).await?;
        }
        "updatePlaybook" => {
            let a: PlaybookArgs = serde_json::from_str(&m.args)?;
            playbook_table::update_playbook_tx(conn, user_id, &a.into_write_args(), &m.hlc).await?;
        }
        "deletePlaybook" => {
            let a: IdArgs = serde_json::from_str(&m.args)?;
            playbook_table::soft_delete_playbook_tx(conn, user_id, &a.id, &m.hlc).await?;
        }
        "createJournalEntry" => {
            let a: JournalArgs = serde_json::from_str(&m.args)?;
            journal_table::create_journal_entry_tx(conn, user_id, &a.into_write_args(), &m.hlc)
                .await?;
        }
        "updateJournalEntry" => {
            let a: JournalArgs = serde_json::from_str(&m.args)?;
            journal_table::update_journal_entry_tx(conn, user_id, &a.into_write_args(), &m.hlc)
                .await?;
        }
        "deleteJournalEntry" => {
            let a: IdArgs = serde_json::from_str(&m.args)?;
            journal_table::soft_delete_journal_entry_tx(conn, user_id, &a.id, &m.hlc).await?;
        }
        "createPrinciple" => {
            let a: PrincipleArgs = serde_json::from_str(&m.args)?;
            trading_principle_table::create_principle_tx(
                conn,
                user_id,
                &a.into_write_args(),
                &m.hlc,
            )
            .await?;
        }
        "updatePrinciple" => {
            let a: PrincipleArgs = serde_json::from_str(&m.args)?;
            trading_principle_table::update_principle_tx(
                conn,
                user_id,
                &a.into_write_args(),
                &m.hlc,
            )
            .await?;
        }
        "deletePrinciple" => {
            let a: IdArgs = serde_json::from_str(&m.args)?;
            trading_principle_table::soft_delete_principle_tx(conn, user_id, &a.id, &m.hlc).await?;
        }
        "reorderPrinciples" => {
            let a: ReorderPrinciplesArgs = serde_json::from_str(&m.args)?;
            trading_principle_table::reorder_principles_tx(conn, user_id, &a.ordered_ids, &m.hlc)
                .await?;
        }
        "createTagCategory" => {
            let a: TagCategoryArgs = serde_json::from_str(&m.args)?;
            tags_table::create_category_tx(
                conn,
                user_id,
                &a.id,
                &a.name,
                a.color.as_deref(),
                a.sort_order.unwrap_or(0),
                &m.hlc,
            )
            .await?;
        }
        "renameTagCategory" => {
            let a: RenameArgs = serde_json::from_str(&m.args)?;
            tags_table::rename_category_tx(conn, user_id, &a.id, &a.name, &m.hlc).await?;
        }
        "setTagCategoryColor" => {
            let a: SetColorArgs = serde_json::from_str(&m.args)?;
            tags_table::set_category_color_tx(conn, user_id, &a.id, a.color.as_deref(), &m.hlc)
                .await?;
        }
        "reorderTagCategories" => {
            let a: ReorderArgs = serde_json::from_str(&m.args)?;
            let pairs: Vec<(String, i64)> =
                a.order.into_iter().map(|o| (o.id, o.sort_order)).collect();
            tags_table::reorder_categories_tx(conn, user_id, &pairs, &m.hlc).await?;
        }
        "deleteTagCategory" => {
            let a: IdArgs = serde_json::from_str(&m.args)?;
            tags_table::soft_delete_category_tx(conn, user_id, &a.id, &m.hlc).await?;
        }
        "createTag" => {
            let a: TagArgs = serde_json::from_str(&m.args)?;
            tags_table::create_tag_tx(
                conn,
                user_id,
                &a.id,
                &a.category_id,
                &a.name,
                a.color.as_deref(),
                &m.hlc,
            )
            .await?;
        }
        "renameTag" => {
            let a: RenameArgs = serde_json::from_str(&m.args)?;
            tags_table::rename_tag_tx(conn, user_id, &a.id, &a.name, &m.hlc).await?;
        }
        "setTagColor" => {
            let a: SetColorArgs = serde_json::from_str(&m.args)?;
            tags_table::set_tag_color_tx(conn, user_id, &a.id, a.color.as_deref(), &m.hlc).await?;
        }
        "deleteTag" => {
            let a: IdArgs = serde_json::from_str(&m.args)?;
            tags_table::soft_delete_tag_tx(conn, user_id, &a.id, &m.hlc).await?;
        }
        "mergeTags" => {
            let a: MergeArgs = serde_json::from_str(&m.args)?;
            tags_table::merge_tags_tx(conn, user_id, &a.from_id, &a.into_id, &m.hlc).await?;
        }
        "upsertPositionCalculatorRule" => {
            let a: CalculatorRuleArgs = serde_json::from_str(&m.args)?;
            position_calculator_rule_table::upsert_rule_tx(
                conn,
                user_id,
                &a.into_write_args(),
                &m.hlc,
            )
            .await?;
        }
        "createPositionCalculatorPlan" => {
            let a: CreateCalculatorPlanArgs = serde_json::from_str(&m.args)?;
            position_calculator_plans_table::create_plan_tx(
                conn,
                user_id,
                &a.into_write_args(),
                &m.hlc,
            )
            .await?;
        }
        "updatePositionCalculatorPlan" => {
            let a: UpdateCalculatorPlanArgs = serde_json::from_str(&m.args)?;
            position_calculator_plans_table::update_plan_tx(
                conn,
                user_id,
                &a.into_write_args(),
                &m.hlc,
            )
            .await?;
        }
        "deletePositionCalculatorPlan" => {
            let a: IdArgs = serde_json::from_str(&m.args)?;
            position_calculator_plans_table::soft_delete_plan_tx(conn, user_id, &a.id, &m.hlc)
                .await?;
        }
        "createPositionCalculatorHistory" => {
            let a: CreateCalculatorHistoryArgs = serde_json::from_str(&m.args)?;
            position_calculator_history_table::create_history_tx(
                conn,
                user_id,
                &a.into_write_args(),
                &m.hlc,
            )
            .await?;
        }
        "deletePositionCalculatorHistory" => {
            let a: IdArgs = serde_json::from_str(&m.args)?;
            position_calculator_history_table::soft_delete_history_tx(conn, user_id, &a.id, &m.hlc)
                .await?;
        }
        other => anyhow::bail!("unknown mutation '{other}'"),
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaybookArgs {
    id: String,
    name: String,
    edge_name: String,
    entry_rules: String,
    exit_rules: String,
    position_sizing_rules: String,
    additional_rules: Option<String>,
}

impl PlaybookArgs {
    fn into_write_args(self) -> playbook_table::PlaybookWriteArgs {
        playbook_table::PlaybookWriteArgs {
            id: self.id,
            name: self.name,
            edge_name: self.edge_name,
            entry_rules: self.entry_rules,
            exit_rules: self.exit_rules,
            position_sizing_rules: self.position_sizing_rules,
            additional_rules: self.additional_rules,
        }
    }
}

fn is_playbook_mutation(name: &str) -> bool {
    matches!(name, "createPlaybook" | "updatePlaybook" | "deletePlaybook")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TagCategoryArgs {
    id: String,
    name: String,
    color: Option<String>,
    sort_order: Option<i64>,
}

#[derive(Deserialize)]
struct RenameArgs {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetColorArgs {
    id: String,
    color: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReorderOrderItem {
    id: String,
    sort_order: i64,
}

#[derive(Deserialize)]
struct ReorderArgs {
    order: Vec<ReorderOrderItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TagArgs {
    id: String,
    category_id: String,
    name: String,
    color: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergeArgs {
    from_id: String,
    into_id: String,
}

/// Whole-row payload for `upsertPositionCalculatorRule`. The client mints
/// `id`; the server keys the actual upsert on `(user_id, account_id)`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CalculatorRuleArgs {
    id: String,
    account_id: String,
    account_balance: f64,
    account_risk: f64,
    max_stop_loss_pct: f64,
}

impl CalculatorRuleArgs {
    fn into_write_args(self) -> position_calculator_rule_table::RuleWriteArgs {
        position_calculator_rule_table::RuleWriteArgs {
            id: self.id,
            account_id: self.account_id,
            account_balance: self.account_balance,
            account_risk: self.account_risk,
            max_stop_loss_pct: self.max_stop_loss_pct,
        }
    }
}

/// Whole-row payload for `createPositionCalculatorPlan`. `tranches_json`
/// travels as an opaque, already-serialized string — the client owns tranche
/// shape/ids (mirrors the desktop `calc_plans.tranches_json` column).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCalculatorPlanArgs {
    id: String,
    symbol: String,
    position_type: String,
    entry_price: f64,
    stop_loss: f64,
    account_balance: f64,
    account_risk: f64,
    total_shares: f64,
    position_value: f64,
    #[serde(default = "default_plan_status")]
    status: String,
    tranches_json: String,
    notes: Option<String>,
}

fn default_plan_status() -> String {
    "active".to_string()
}

impl CreateCalculatorPlanArgs {
    fn into_write_args(self) -> position_calculator_plans_table::CreatePlanWriteArgs {
        position_calculator_plans_table::CreatePlanWriteArgs {
            id: self.id,
            symbol: self.symbol,
            position_type: self.position_type,
            entry_price: self.entry_price,
            stop_loss: self.stop_loss,
            account_balance: self.account_balance,
            account_risk: self.account_risk,
            total_shares: self.total_shares,
            position_value: self.position_value,
            status: self.status,
            tranches_json: self.tranches_json,
            notes: self.notes,
        }
    }
}

/// Partial-update payload for `updatePositionCalculatorPlan`: only `status`,
/// `tranches` (as `tranchesJson`), and `notes` are mutable post-create.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCalculatorPlanArgs {
    id: String,
    status: String,
    tranches_json: String,
    notes: Option<String>,
}

impl UpdateCalculatorPlanArgs {
    fn into_write_args(self) -> position_calculator_plans_table::UpdatePlanWriteArgs {
        position_calculator_plans_table::UpdatePlanWriteArgs {
            id: self.id,
            status: self.status,
            tranches_json: self.tranches_json,
            notes: self.notes,
        }
    }
}

/// Whole-row payload for `createPositionCalculatorHistory`. History is
/// insert + soft-delete only — no update mutation exists.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCalculatorHistoryArgs {
    id: String,
    symbol: String,
    position_type: String,
    entry_price: f64,
    stop_loss: f64,
    account_balance: f64,
    account_risk: f64,
    shares: f64,
    position_value: f64,
    account_pct: f64,
    stop_loss_pct: f64,
}

impl CreateCalculatorHistoryArgs {
    fn into_write_args(self) -> position_calculator_history_table::HistoryWriteArgs {
        position_calculator_history_table::HistoryWriteArgs {
            id: self.id,
            symbol: self.symbol,
            position_type: self.position_type,
            entry_price: self.entry_price,
            stop_loss: self.stop_loss,
            account_balance: self.account_balance,
            account_risk: self.account_risk,
            shares: self.shares,
            position_value: self.position_value,
            account_pct: self.account_pct,
            stop_loss_pct: self.stop_loss_pct,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JournalArgs {
    id: String,
    account_id: String,
    open_date: String,
    close_date: String,
    entry_price: f64,
    exit_price: f64,
    position_size: f64,
    stop_loss: Option<f64>,
    symbol: String,
    symbol_name: String,
    trade_type: String,
    playbook_id: Option<String>,
    notes: Option<String>,
    broke_30min_rule: Option<bool>,
    pre_trade_conviction: Option<i32>,
    market_regime: Option<String>,
    is_planned_pre_market: Option<bool>,
    revenge_trade: Option<bool>,
    rule_adherence_score: Option<i32>,
    #[serde(default)]
    tag_ids: Vec<String>,
    #[serde(default)]
    violated_principle_ids: Vec<String>,
}

impl JournalArgs {
    fn into_write_args(self) -> journal_table::JournalWriteArgs {
        journal_table::JournalWriteArgs {
            id: self.id,
            account_id: self.account_id,
            open_date: self.open_date,
            close_date: self.close_date,
            entry_price: self.entry_price,
            exit_price: self.exit_price,
            position_size: self.position_size,
            stop_loss: self.stop_loss,
            symbol: self.symbol,
            symbol_name: self.symbol_name,
            trade_type: self.trade_type,
            playbook_id: self.playbook_id,
            notes: self.notes,
            broke_30min_rule: self.broke_30min_rule,
            pre_trade_conviction: self.pre_trade_conviction,
            market_regime: self.market_regime,
            is_planned_pre_market: self.is_planned_pre_market,
            revenge_trade: self.revenge_trade,
            rule_adherence_score: self.rule_adherence_score,
            tag_ids: self.tag_ids,
            violated_principle_ids: self.violated_principle_ids,
        }
    }
}

fn is_journal_mutation(name: &str) -> bool {
    matches!(
        name,
        "createJournalEntry" | "updateJournalEntry" | "deleteJournalEntry"
    )
}

/// Whole-row payload for `createPrinciple`/`updatePrinciple`. No AI reindex
/// hook here (unlike playbooks/journal entries): principles aren't indexed
/// into the vector store.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrincipleArgs {
    id: String,
    account_id: String,
    playbook_id: Option<String>,
    evidence_note_id: Option<String>,
    title: String,
    the_rule: String,
    why: String,
    intervention: Option<String>,
    is_active: bool,
    priority: i64,
}

impl PrincipleArgs {
    fn into_write_args(self) -> trading_principle_table::PrincipleWriteArgs {
        trading_principle_table::PrincipleWriteArgs {
            id: self.id,
            account_id: self.account_id,
            playbook_id: self.playbook_id,
            evidence_note_id: self.evidence_note_id,
            title: self.title,
            the_rule: self.the_rule,
            why: self.why,
            intervention: self.intervention,
            is_active: self.is_active,
            priority: self.priority,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReorderPrinciplesArgs {
    ordered_ids: Vec<String>,
}

/// The account a journal mutation touched, for the account-scoped AI reindex.
/// create/update carry `accountId` in their args; delete only carries the
/// entry `id`, so its account is looked up from the (soft-deleted, still
/// present) row.
async fn journal_account_id_for_mutation(
    pool: &PgPool,
    user_id: &str,
    m: &NotebookMutation,
) -> Result<Option<String>> {
    match m.name.as_str() {
        "createJournalEntry" | "updateJournalEntry" => {
            let a: JournalArgs = serde_json::from_str(&m.args)?;
            Ok(Some(a.account_id))
        }
        "deleteJournalEntry" => {
            let a: IdArgs = serde_json::from_str(&m.args)?;
            let account_id: Option<String> = sqlx::query_scalar(
                "SELECT account_id FROM journal_entries WHERE id = $1 AND user_id = $2",
            )
            .bind(&a.id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            Ok(account_id)
        }
        _ => Ok(None),
    }
}

#[derive(Default)]
pub struct NotebookSyncMutation;

#[Object]
impl NotebookSyncMutation {
    async fn push_notebook(
        &self,
        ctx: &Context<'_>,
        input: NotebookPushInput,
    ) -> async_graphql::Result<NotebookPushResult> {
        let user_db = super::base::get_user_db(ctx).await?;
        let pool = user_db.pool();
        let user_id = user_db.user_id();

        let mut sorted = input.mutations.clone();
        sorted.sort_by_key(|m| m.id);

        let mut conn = pool.acquire().await?;
        let mut last = sync::last_mutation_id(&mut conn, &input.client_id, user_id).await?;
        drop(conn);

        let mut applied_playbook = false;
        let mut touched_journal_accounts: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for m in &sorted {
            if m.id <= last {
                continue; // Already applied; apply_mutation would no-op anyway.
            }
            // Never skip a gap: applying id N when N-1 was never seen would advance
            // the watermark past a lost mutation, silently dropping it forever. Stop
            // and let the client resend the contiguous tail.
            if m.id > last + 1 {
                break;
            }
            last = apply_mutation(pool, user_id, &input.client_id, m).await?;
            if is_playbook_mutation(&m.name) {
                applied_playbook = true;
            }
            if is_journal_mutation(&m.name)
                && let Some(account_id) = journal_account_id_for_mutation(pool, user_id, m).await?
            {
                touched_journal_accounts.insert(account_id);
            }
        }

        // Offline-created/edited playbooks must still enter the AI search index,
        // exactly like the GraphQL playbook mutations do. Fire once per push.
        if applied_playbook && let Ok(db) = ctx.data::<std::sync::Arc<crate::service::db::Db>>() {
            crate::service::ai::jobs::enqueue_all_account_reindex(db.as_ref(), user_id).await?;
        }

        // Same for offline-created/edited journal entries, but account-scoped
        // (unlike playbooks, journal entries belong to one account).
        if !touched_journal_accounts.is_empty()
            && let Ok(db) = ctx.data::<std::sync::Arc<crate::service::db::Db>>()
        {
            for account_id in &touched_journal_accounts {
                crate::service::ai::jobs::enqueue_account_reindex(db.as_ref(), user_id, account_id)
                    .await?;
            }
        }

        Ok(NotebookPushResult {
            last_mutation_id: last,
        })
    }
}

#[derive(Default)]
pub struct NotebookSyncQuery;

#[Object]
impl NotebookSyncQuery {
    async fn pull_notebook(
        &self,
        ctx: &Context<'_>,
        cookie: Option<String>,
        account_id: String,
        client_id: String,
    ) -> async_graphql::Result<NotebookPullResult> {
        let user_db = super::base::get_user_db(ctx).await?;
        let pool = user_db.pool();
        let user_id = user_db.user_id();

        let notes = sync::notes_since(pool, user_id, &account_id, cookie.as_deref()).await?;
        let folders = sync::folders_since(pool, user_id, &account_id, cookie.as_deref()).await?;

        // Opaque cursor: the max updated_at across returned rows, or the incoming
        // cookie echoed back when nothing changed. Fixed-width ISO microsecond
        // strings sort chronologically, so lexicographic max is correct.
        let mut next = cookie.unwrap_or_default();
        for updated_at in notes
            .iter()
            .map(|n| &n.updated_at)
            .chain(folders.iter().map(|f| &f.updated_at))
        {
            if *updated_at > next {
                next = updated_at.clone();
            }
        }

        let last_mutation_id = sync::last_mutation_id_for_client(pool, &client_id, user_id).await?;

        Ok(NotebookPullResult {
            cookie: next,
            last_mutation_id,
            notes: notes.into_iter().map(Into::into).collect(),
            folders: folders.into_iter().map(Into::into).collect(),
        })
    }
}
