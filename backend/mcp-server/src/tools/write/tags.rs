//! Tag writes, and the trade↔tag link.
//!
//! Two things shape this file.
//!
//! **Idempotency.** `tags` is uniquely indexed on `(user_id, category_id, lower(name))` and
//! `tag_categories` on `(user_id, lower(name))`. A model that retries — and they all retry —
//! would otherwise get a constraint violation for asking twice. So create means
//! create-or-return, and a duplicate is a no-op rather than a failure.
//!
//! **The link table cannot defend itself.** `trade_tags` has no `user_id` and no clock:
//! `tags_table::set_trade_tags` is what validates both sides and bumps the trade so the
//! change reaches the desktop. Everything here goes through it.

use rmcp::{
    ErrorData, RoleServer, handler::server::wrapper::Parameters, model::*, service::RequestContext,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use tradstry_backend::service::db::schema::tables::journal_table;
use tradstry_backend::service::db::schema::tables::tags_table;

use crate::server::TradstryMcp;
use crate::tools::write::{internal, not_found, ok};

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LinkMode {
    /// Replace the trade's tags with exactly this set.
    Set,
    /// Keep the existing tags and add these.
    Add,
    /// Keep the existing tags except these.
    Remove,
}

/// Parameters for `create_tag`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreateTagParams {
    /// Workspace that owns the tag. Obtain it from `list_workspaces`.
    pub workspace_id: String,
    /// The category to file the tag under, by name (case-insensitive) — e.g. "Mistakes",
    /// "Entry Tactics". The category is created if it does not exist. To make a tag that
    /// marks a trade as flawed, use the category whose role is `mistake` (see `list_tags`).
    pub category: String,
    /// The tag itself, e.g. "chased entry". Creating one that already exists returns the
    /// existing tag rather than failing.
    pub name: String,
    /// Optional hex colour, e.g. "#e11d48".
    pub color: Option<String>,
}

/// Parameters for `tag_trade`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct TagTradeParams {
    /// The trade to tag. Obtain ids from `query_trades`.
    pub trade_id: String,
    /// Tag ids, from `list_tags` or `create_tag`.
    pub tag_ids: Vec<String>,
    /// `set` replaces the trade's tags entirely; `add` keeps the existing ones; `remove`
    /// detaches only these. Prefer `add` unless you mean to discard the user's own tags.
    pub mode: LinkMode,
}

/// Parameters for `delete_tag`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeleteTagParams {
    /// The tag to delete. It is detached from every trade that carried it.
    pub tag_id: String,
}

/// Parameters for `merge_tags`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct MergeTagsParams {
    /// The duplicate to fold away. It is deleted.
    pub from_tag_id: String,
    /// The tag to keep. Every trade tagged `from` ends up tagged `into`.
    pub into_tag_id: String,
}

#[tool_router(router = tags_write_router, vis = "pub")]
impl TradstryMcp {
    #[tool(
        description = "Create a tag, or return it if it already exists. Tags are how a trade's \
                       qualities become queryable and priced: a tag in the `mistake`-role \
                       category marks the trade as flawed, which is what the clean-vs-flawed \
                       and mistake-cost analytics are computed from. Call `list_tags` first to \
                       see the existing taxonomy — reuse a tag rather than inventing a synonym."
    )]
    pub async fn create_tag(
        &self,
        Parameters(params): Parameters<CreateTagParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;
        let pool = user_db.pool();
        let user_id = user_db.user_id();

        let category_name = params.category.trim();
        let tag_name = params.name.trim();
        if category_name.is_empty() || tag_name.is_empty() {
            return Err(ErrorData::invalid_params(
                "category and name are required",
                None,
            ));
        }

        // Create-or-return, both levels. The unique indexes would otherwise turn a retry
        // into a hard error, and models retry.
        let categories = tags_table::list_categories(pool, user_id, &params.workspace_id)
            .await
            .map_err(internal)?;
        let category = match categories
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(category_name))
        {
            Some(c) => c.clone(),
            None => tags_table::create_category(
                pool,
                user_id,
                &params.workspace_id,
                category_name,
                None,
            )
            .await
            .map_err(internal)?,
        };

        let existing =
            tags_table::list_tags(pool, user_id, &params.workspace_id, Some(&category.id))
                .await
                .map_err(internal)?;
        if let Some(t) = existing
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(tag_name))
        {
            return ok(format!(
                "Tag \"{}\" already exists in \"{}\" (id {}).",
                t.name, category.name, t.id
            ));
        }

        let tag = tags_table::create_tag(
            pool,
            user_id,
            &params.workspace_id,
            &category.id,
            tag_name,
            params.color.as_deref(),
        )
        .await
        .map_err(internal)?;

        ok(format!(
            "Created tag {} \"{}\" in \"{}\".",
            tag.id, tag.name, category.name
        ))
    }

    #[tool(
        description = "Attach tags to a trade, or detach them. mode=\"add\" keeps the tags the \
                       trade already has; mode=\"set\" replaces them entirely (this discards \
                       the user's own tags, so only use it when you mean to); \
                       mode=\"remove\" detaches just the ones you name."
    )]
    pub async fn tag_trade(
        &self,
        Parameters(params): Parameters<TagTradeParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;
        let pool = user_db.pool();
        let user_id = user_db.user_id();

        // `set_trade_tags` checks both sides too, but a foreign trade must not even be
        // confirmable, and `add`/`remove` have to read the current set first.
        journal_table::find_journal_entry(pool, &params.trade_id, user_id)
            .await
            .map_err(internal)?
            .ok_or_else(|| not_found("trade"))?;

        let current: Vec<String> = tags_table::tags_for_trade(pool, user_id, &params.trade_id)
            .await
            .map_err(internal)?
            .into_iter()
            .map(|t| t.id)
            .collect();

        let mut next = match params.mode {
            LinkMode::Set => params.tag_ids.clone(),
            LinkMode::Add => {
                let mut v = current.clone();
                for id in &params.tag_ids {
                    if !v.contains(id) {
                        v.push(id.clone());
                    }
                }
                v
            }
            LinkMode::Remove => current
                .iter()
                .filter(|id| !params.tag_ids.contains(id))
                .cloned()
                .collect(),
        };
        next.dedup();

        tags_table::set_trade_tags(pool, user_id, &params.trade_id, &next)
            .await
            .map_err(internal)?;

        ok(format!(
            "Trade {} now has {} tag(s).",
            params.trade_id,
            next.len()
        ))
    }

    #[tool(
        description = "Delete a tag. It is detached from every trade that carried it, and \
                       those trades keep their history."
    )]
    pub async fn delete_tag(
        &self,
        Parameters(params): Parameters<DeleteTagParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let deleted = tags_table::delete_tag(user_db.pool(), user_db.user_id(), &params.tag_id)
            .await
            .map_err(internal)?;
        if !deleted {
            return Err(not_found("tag"));
        }
        ok(format!("Deleted tag {}.", params.tag_id))
    }

    #[tool(
        description = "Fold a duplicate tag into another: every trade tagged with `from` ends \
                       up tagged with `into`, and `from` is deleted. Use this to clean up \
                       synonyms (\"chased\" and \"chasing\") so the analytics stop splitting \
                       one behaviour across two tags."
    )]
    pub async fn merge_tags(
        &self,
        Parameters(params): Parameters<MergeTagsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        tags_table::merge_tags(
            user_db.pool(),
            user_db.user_id(),
            &params.from_tag_id,
            &params.into_tag_id,
        )
        .await
        .map_err(internal)?;

        ok(format!(
            "Merged tag {} into {}.",
            params.from_tag_id, params.into_tag_id
        ))
    }
}
