use async_graphql::{Context, Object, Result, SimpleObject};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::db::Db;
use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::notebook::sync as notebook_sync;
use crate::service::db::schema::tables::position_calculator_history_table::{
    self, CreatePositionCalculatorHistoryInput, HistoryDelta, PositionCalculatorHistoryEntry,
};
use crate::service::db::schema::tables::position_calculator_plans_table::{
    self, CreatePositionCalculatorPlanInput, PlanDelta, PositionCalculatorPlan,
    UpdatePositionCalculatorPlanInput,
};
use crate::service::db::schema::tables::position_calculator_rule_table::{
    self, PositionCalculatorRule, RuleDelta, UpsertPositionCalculatorRuleInput,
};
use crate::service::read_service::users::ensure_user;

async fn get_user_db(ctx: &Context<'_>) -> Result<UserDb> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let db = ctx.data::<Arc<Db>>()?;
    let pool = db.pool();

    let full_name = jwt
        .other
        .get("full_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let email = jwt
        .other
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let user = ensure_user(pool, &jwt.sub, full_name, email).await?;
    Ok(db.get_user_db(&user.id))
}

// ---------------------------------------------------------------------------
// Offline-first sync (whole-row LWW + soft-delete)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct RuleDeltaGql {
    pub id: String,
    pub account_id: String,
    pub account_balance: f64,
    pub account_risk: f64,
    pub max_stop_loss_pct: f64,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

impl From<RuleDelta> for RuleDeltaGql {
    fn from(d: RuleDelta) -> Self {
        Self {
            id: d.id,
            account_id: d.account_id,
            account_balance: d.account_balance,
            account_risk: d.account_risk,
            max_stop_loss_pct: d.max_stop_loss_pct,
            hlc: d.hlc,
            deleted_at: d.deleted_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PlanDeltaGql {
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

impl From<PlanDelta> for PlanDeltaGql {
    fn from(d: PlanDelta) -> Self {
        Self {
            id: d.id,
            symbol: d.symbol,
            position_type: d.position_type,
            entry_price: d.entry_price,
            stop_loss: d.stop_loss,
            account_balance: d.account_balance,
            account_risk: d.account_risk,
            total_shares: d.total_shares,
            position_value: d.position_value,
            status: d.status,
            tranches_json: d.tranches_json,
            notes: d.notes,
            hlc: d.hlc,
            deleted_at: d.deleted_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct HistoryDeltaGql {
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

impl From<HistoryDelta> for HistoryDeltaGql {
    fn from(d: HistoryDelta) -> Self {
        Self {
            id: d.id,
            symbol: d.symbol,
            position_type: d.position_type,
            entry_price: d.entry_price,
            stop_loss: d.stop_loss,
            account_balance: d.account_balance,
            account_risk: d.account_risk,
            shares: d.shares,
            position_value: d.position_value,
            account_pct: d.account_pct,
            stop_loss_pct: d.stop_loss_pct,
            hlc: d.hlc,
            deleted_at: d.deleted_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct CalculatorPullResult {
    pub cookie: String,
    pub last_mutation_id: i64,
    pub rules: Vec<RuleDeltaGql>,
    pub plans: Vec<PlanDeltaGql>,
    pub history: Vec<HistoryDeltaGql>,
}

#[derive(Default)]
pub struct PositionCalculatorQuery;

#[Object]
impl PositionCalculatorQuery {
    /// Offline-first pull for the desktop. User-scoped, with one cursor
    /// spanning all three tables (rules + plans + history) — the desktop's
    /// calculator store syncs all three in a single cycle, exactly like
    /// `pull_tags`. Rules are pulled user-wide even though each rule is
    /// scoped to one account.
    async fn pull_calculator(
        &self,
        ctx: &Context<'_>,
        cookie: Option<String>,
        client_id: String,
    ) -> Result<CalculatorPullResult> {
        let user_db = get_user_db(ctx).await?;
        let pool = user_db.pool();
        let user_id = user_db.user_id();

        let rules =
            position_calculator_rule_table::rules_since(pool, user_id, cookie.as_deref()).await?;
        let plans =
            position_calculator_plans_table::plans_since(pool, user_id, cookie.as_deref()).await?;
        let history =
            position_calculator_history_table::history_since(pool, user_id, cookie.as_deref())
                .await?;

        let mut next = cookie.unwrap_or_default();
        for updated_at in rules
            .iter()
            .map(|r| &r.updated_at)
            .chain(plans.iter().map(|p| &p.updated_at))
            .chain(history.iter().map(|h| &h.updated_at))
        {
            if *updated_at > next {
                next = updated_at.clone();
            }
        }

        let last_mutation_id =
            notebook_sync::last_mutation_id_for_client(pool, &client_id, user_id).await?;

        Ok(CalculatorPullResult {
            cookie: next,
            last_mutation_id,
            rules: rules.into_iter().map(Into::into).collect(),
            plans: plans.into_iter().map(Into::into).collect(),
            history: history.into_iter().map(Into::into).collect(),
        })
    }

    async fn position_calculator_rule(
        &self,
        ctx: &Context<'_>,
        account_id: String,
    ) -> Result<Option<PositionCalculatorRule>> {
        let user_db = get_user_db(ctx).await?;
        Ok(
            position_calculator_rule_table::get_rule(
                user_db.pool(),
                user_db.user_id(),
                &account_id,
            )
            .await?,
        )
    }

    async fn position_calculator_history(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<PositionCalculatorHistoryEntry>> {
        let user_db = get_user_db(ctx).await?;
        Ok(
            position_calculator_history_table::list_history(user_db.pool(), user_db.user_id())
                .await?,
        )
    }

    async fn position_calculator_plans(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<PositionCalculatorPlan>> {
        let user_db = get_user_db(ctx).await?;
        Ok(position_calculator_plans_table::list_plans(user_db.pool(), user_db.user_id()).await?)
    }
}

#[derive(Default)]
pub struct PositionCalculatorMutation;

#[Object]
impl PositionCalculatorMutation {
    async fn upsert_position_calculator_rule(
        &self,
        ctx: &Context<'_>,
        input: UpsertPositionCalculatorRuleInput,
    ) -> Result<PositionCalculatorRule> {
        let user_db = get_user_db(ctx).await?;
        Ok(
            position_calculator_rule_table::upsert_rule(user_db.pool(), user_db.user_id(), input)
                .await?,
        )
    }

    async fn create_position_calculator_history(
        &self,
        ctx: &Context<'_>,
        input: CreatePositionCalculatorHistoryInput,
    ) -> Result<PositionCalculatorHistoryEntry> {
        let user_db = get_user_db(ctx).await?;
        Ok(position_calculator_history_table::create_history_entry(
            user_db.pool(),
            user_db.user_id(),
            input,
        )
        .await?)
    }

    async fn delete_position_calculator_history(
        &self,
        ctx: &Context<'_>,
        id: String,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(position_calculator_history_table::delete_history_entry(
            user_db.pool(),
            &id,
            user_db.user_id(),
        )
        .await?)
    }

    async fn create_position_calculator_plan(
        &self,
        ctx: &Context<'_>,
        input: CreatePositionCalculatorPlanInput,
    ) -> Result<PositionCalculatorPlan> {
        let user_db = get_user_db(ctx).await?;
        Ok(
            position_calculator_plans_table::create_plan(user_db.pool(), user_db.user_id(), input)
                .await?,
        )
    }

    async fn update_position_calculator_plan(
        &self,
        ctx: &Context<'_>,
        id: String,
        input: UpdatePositionCalculatorPlanInput,
    ) -> Result<PositionCalculatorPlan> {
        let user_db = get_user_db(ctx).await?;
        Ok(position_calculator_plans_table::update_plan(
            user_db.pool(),
            &id,
            user_db.user_id(),
            input,
        )
        .await?)
    }

    async fn delete_position_calculator_plan(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(
            position_calculator_plans_table::delete_plan(user_db.pool(), &id, user_db.user_id())
                .await?,
        )
    }
}
