use async_graphql::{Context, Object, Result};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::db::Db;
use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::position_calculator_history_table::{
    self, CreatePositionCalculatorHistoryInput, PositionCalculatorHistoryEntry,
};
use crate::service::db::schema::tables::position_calculator_plans_table::{
    self, CreatePositionCalculatorPlanInput, PositionCalculatorPlan,
    UpdatePositionCalculatorPlanInput,
};
use crate::service::db::schema::tables::position_calculator_rule_table::{
    self, PositionCalculatorRule, UpsertPositionCalculatorRuleInput,
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

#[derive(Default)]
pub struct PositionCalculatorQuery;

#[Object]
impl PositionCalculatorQuery {
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
