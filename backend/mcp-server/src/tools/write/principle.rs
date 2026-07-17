//! Trading-principle writes, and the trade↔violation link.
//!
//! Principles are the rules the user wrote for themselves; a violation link says a given
//! trade broke one. Principles are account-scoped and so are trades, so a violation only
//! means anything when both sit in the same account — `set_trade_principle_violations`
//! enforces that, and bumps the trade so the link reaches the desktop.

use rmcp::{
    ErrorData, RoleServer, handler::server::wrapper::Parameters, model::*, service::RequestContext,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use tradstry_backend::service::db::schema::tables::journal_table;
use tradstry_backend::service::db::schema::tables::trading_principle_table::{
    self as tp, CreatePrincipleInput, UpdatePrincipleInput,
};

use crate::server::TradstryMcp;
use crate::tools::write::tags::LinkMode;
use crate::tools::write::{internal, not_found, ok};

/// Parameters for `create_principle`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreatePrincipleParams {
    /// The account this rule governs. A principle can only be violated by trades in the same
    /// account, so this must be the account whose trades it is about.
    pub account_id: String,
    /// Short name, 80 characters or less, e.g. "No chasing".
    pub title: String,
    /// The rule itself, stated so a trade can be judged against it — e.g. "No entry more
    /// than 2% above the trigger".
    pub the_rule: String,
    /// Why it exists. Ground this in the user's own history: what breaking it has cost them.
    pub why: String,
    /// What to do instead, when tempted.
    pub intervention: Option<String>,
    /// Optionally tie the rule to one playbook, when it only applies to that setup.
    pub playbook_id: Option<String>,
    /// Optionally point at a notebook note holding the evidence. Must be in the same account.
    pub evidence_note_id: Option<String>,
}

/// Parameters for `update_principle`. Omitted fields are left unchanged.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdatePrincipleParams {
    /// The principle to edit. Obtain ids from `get_principles`.
    pub principle_id: String,
    pub title: Option<String>,
    pub the_rule: Option<String>,
    pub why: Option<String>,
    pub intervention: Option<String>,
    /// Retire a rule without destroying its violation history.
    pub is_active: Option<bool>,
}

/// Parameters for `delete_principle`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeletePrincipleParams {
    /// The principle to delete. Its violation links go with it.
    pub principle_id: String,
}

/// Parameters for `flag_violation`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct FlagViolationParams {
    /// The trade that broke the rule(s).
    pub trade_id: String,
    /// Principle ids, from `get_principles`. They must govern the trade's own account.
    pub principle_ids: Vec<String>,
    /// `add` keeps the violations already recorded; `set` replaces them; `remove` clears
    /// just the ones you name.
    pub mode: LinkMode,
}

#[tool_router(router = principle_write_router, vis = "pub")]
impl TradstryMcp {
    #[tool(
        description = "Create a trading principle: a rule the user holds themselves to. State \
                       `the_rule` so a trade can actually be judged against it, and ground \
                       `why` in what breaking it has cost them. Once it exists, flag_violation \
                       ties trades to it and the discipline analytics start pricing it."
    )]
    pub async fn create_principle(
        &self,
        Parameters(params): Parameters<CreatePrincipleParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let p = tp::create_principle(
            user_db.pool(),
            user_db.user_id(),
            CreatePrincipleInput {
                account_id: params.account_id,
                title: params.title,
                the_rule: params.the_rule,
                why: params.why,
                intervention: params.intervention,
                playbook_id: params.playbook_id,
                evidence_note_id: params.evidence_note_id,
            },
        )
        .await
        .map_err(internal)?;

        ok(format!("Created principle {} \"{}\".", p.id, p.title))
    }

    #[tool(
        description = "Edit a principle. Only the fields you pass change. Set is_active=false \
                       to retire a rule while keeping its violation history intact."
    )]
    pub async fn update_principle(
        &self,
        Parameters(params): Parameters<UpdatePrincipleParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let p = tp::update_principle(
            user_db.pool(),
            &params.principle_id,
            user_db.user_id(),
            UpdatePrincipleInput {
                title: params.title,
                the_rule: params.the_rule,
                why: params.why,
                intervention: params.intervention,
                clear_intervention: false,
                playbook_id: None,
                clear_playbook: false,
                evidence_note_id: None,
                clear_evidence_note: false,
                is_active: params.is_active,
            },
        )
        .await
        .map_err(internal)?;

        ok(format!("Updated principle {} \"{}\".", p.id, p.title))
    }

    #[tool(
        description = "Delete a principle. Every violation link to it is removed, and the \
                       trades that carried them are updated accordingly."
    )]
    pub async fn delete_principle(
        &self,
        Parameters(params): Parameters<DeletePrincipleParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let deleted = tp::delete_principle(user_db.pool(), &params.principle_id, user_db.user_id())
            .await
            .map_err(internal)?;
        if !deleted {
            return Err(not_found("principle"));
        }
        ok(format!("Deleted principle {}.", params.principle_id))
    }

    #[tool(
        description = "Record that a trade broke one or more of the user's principles — or \
                       clear such a record. This is what turns a written rule into a measured \
                       one: violations feed the discipline analytics and the per-principle \
                       cost. A principle can only be violated by a trade in the account it \
                       governs. Prefer mode=\"add\" so you never erase the user's own judgment."
    )]
    pub async fn flag_violation(
        &self,
        Parameters(params): Parameters<FlagViolationParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;
        let pool = user_db.pool();
        let user_id = user_db.user_id();

        journal_table::find_journal_entry(pool, &params.trade_id, user_id)
            .await
            .map_err(internal)?
            .ok_or_else(|| not_found("trade"))?;

        let current = tp::principles_for_trade(pool, user_id, &params.trade_id)
            .await
            .map_err(internal)?;

        let mut next = match params.mode {
            LinkMode::Set => params.principle_ids.clone(),
            LinkMode::Add => {
                let mut v = current.clone();
                for id in &params.principle_ids {
                    if !v.contains(id) {
                        v.push(id.clone());
                    }
                }
                v
            }
            LinkMode::Remove => current
                .iter()
                .filter(|id| !params.principle_ids.contains(id))
                .cloned()
                .collect(),
        };
        next.dedup();

        // Validates every principle belongs to the caller AND governs the trade's account,
        // then bumps the trade so the link actually reaches the desktop.
        tp::set_trade_principle_violations(pool, user_id, &params.trade_id, &next)
            .await
            .map_err(internal)?;

        ok(format!(
            "Trade {} now records {} violation(s).",
            params.trade_id,
            next.len()
        ))
    }
}
