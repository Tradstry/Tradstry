//! Playbook writes, and the trade→playbook link.
//!
//! Playbooks are user-scoped — they carry no `account_id` — so a trade in any account may
//! reference any of the caller's playbooks. What must never happen is referencing someone
//! else's, so every id is resolved by `(id, caller)` rather than trusted.

use rmcp::{
    ErrorData, RoleServer, handler::server::wrapper::Parameters, model::*, service::RequestContext,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use tradstry_backend::service::db::schema::tables::journal_table::{self, UpdateJournalEntryInput};
use tradstry_backend::service::db::schema::tables::playbook_table::{
    self, CreatePlaybookInput, UpdatePlaybookInput,
};

use crate::server::TradstryMcp;
use crate::tools::write::{internal, not_found, ok};

/// Parameters for `create_playbook`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreatePlaybookParams {
    /// Short name, e.g. "Relative strength".
    pub name: String,
    /// The edge this playbook trades, e.g. "Inside day".
    pub edge_name: String,
    /// When to enter. Free text; the user's own words are the point.
    pub entry_rules: String,
    /// When to exit.
    pub exit_rules: String,
    /// How much to risk / how to size.
    pub position_sizing_rules: String,
    /// Anything else — filters, market conditions, checklists.
    pub additional_rules: Option<String>,
}

/// Parameters for `update_playbook`. Omitted fields are left unchanged.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdatePlaybookParams {
    /// The playbook to edit. Obtain ids from `get_playbook`.
    pub playbook_id: String,
    pub name: Option<String>,
    pub edge_name: Option<String>,
    pub entry_rules: Option<String>,
    pub exit_rules: Option<String>,
    pub position_sizing_rules: Option<String>,
    pub additional_rules: Option<String>,
}

/// Parameters for `delete_playbook`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeletePlaybookParams {
    /// The playbook to delete. Refused while any principle still references it.
    pub playbook_id: String,
}

/// Parameters for `set_trade_playbook`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SetTradePlaybookParams {
    /// The trade to attribute. Obtain ids from `query_trades`.
    pub trade_id: String,
    /// The playbook this trade was taken from. Omit to detach the trade from any playbook
    /// (which is what `query_trades`' `untagged_only` filter finds).
    pub playbook_id: Option<String>,
}

#[tool_router(router = playbook_write_router, vis = "pub")]
impl TradstryMcp {
    #[tool(
        description = "Create a trading playbook: a named setup with its entry, exit and \
                       position-sizing rules. Playbooks are how trades get attributed to a \
                       strategy, and `get_playbook` reports each one's realized win rate and \
                       P&L. Returns the playbook id."
    )]
    pub async fn create_playbook(
        &self,
        Parameters(params): Parameters<CreatePlaybookParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let pb = playbook_table::create_playbook(
            user_db.pool(),
            user_db.user_id(),
            CreatePlaybookInput {
                name: params.name,
                edge_name: params.edge_name,
                entry_rules: params.entry_rules,
                exit_rules: params.exit_rules,
                position_sizing_rules: params.position_sizing_rules,
                additional_rules: params.additional_rules,
            },
        )
        .await
        .map_err(internal)?;

        ok(format!("Created playbook {} \"{}\".", pb.id, pb.name))
    }

    #[tool(
        description = "Edit a playbook's rules. Only the fields you pass are changed; omit \
                       the rest. Use this to refine a setup as the user learns what works."
    )]
    pub async fn update_playbook(
        &self,
        Parameters(params): Parameters<UpdatePlaybookParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let pb = playbook_table::update_playbook(
            user_db.pool(),
            &params.playbook_id,
            user_db.user_id(),
            UpdatePlaybookInput {
                name: params.name,
                edge_name: params.edge_name,
                entry_rules: params.entry_rules,
                exit_rules: params.exit_rules,
                position_sizing_rules: params.position_sizing_rules,
                additional_rules: params.additional_rules,
                clear_additional_rules: false,
            },
        )
        .await
        .map_err(internal)?;

        ok(format!("Updated playbook {} \"{}\".", pb.id, pb.name))
    }

    #[tool(
        description = "Delete a playbook. Refused while any trading principle still \
                       references it — detach or delete those principles first. Trades that \
                       used the playbook keep their history and become unattributed."
    )]
    pub async fn delete_playbook(
        &self,
        Parameters(params): Parameters<DeletePlaybookParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let deleted =
            playbook_table::delete_playbook(user_db.pool(), &params.playbook_id, user_db.user_id())
                .await
                .map_err(internal)?;

        if !deleted {
            return Err(not_found("playbook"));
        }
        ok(format!("Deleted playbook {}.", params.playbook_id))
    }

    #[tool(
        description = "Attribute a trade to a playbook, or detach it by omitting playbook_id. \
                       This is what makes `get_playbook` and the by-playbook analytics \
                       breakdown meaningful, so attributing untagged trades is high value."
    )]
    pub async fn set_trade_playbook(
        &self,
        Parameters(params): Parameters<SetTradePlaybookParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;
        let pool = user_db.pool();

        // Both sides resolved against the caller. `update_journal_entry` validates the
        // playbook too, but a foreign trade id must not even be confirmable.
        journal_table::find_journal_entry(pool, &params.trade_id, user_db.user_id())
            .await
            .map_err(internal)?
            .ok_or_else(|| not_found("trade"))?;

        journal_table::update_journal_entry(
            pool,
            &params.trade_id,
            user_db.user_id(),
            UpdateJournalEntryInput {
                playbook_id: params.playbook_id.clone(),
                clear_playbook: params.playbook_id.is_none(),
                ..Default::default()
            },
        )
        .await
        .map_err(internal)?;

        ok(match params.playbook_id {
            Some(pb) => format!("Trade {} now attributed to playbook {pb}.", params.trade_id),
            None => format!("Trade {} detached from its playbook.", params.trade_id),
        })
    }
}
