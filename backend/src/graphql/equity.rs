use async_graphql::{Context, Object, Result, SimpleObject};
use chrono::NaiveDate;
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::db::Db;
use crate::service::db::schema::tables::equity_table;
use crate::service::equity::rebuild;
use crate::service::equity::replay::{EquityPoint, ReplayHealth};
use crate::service::read_service::users::ensure_user;

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct EquityHistoryPointGql {
    pub date: String,
    pub cash: f64,
    pub positions_value: f64,
    pub equity: f64,
    pub net_contributions: f64,
    pub funding_adjusted_equity: f64,
}

impl From<EquityPoint> for EquityHistoryPointGql {
    fn from(p: EquityPoint) -> Self {
        Self {
            date: p.date.to_string(),
            cash: p.cash,
            positions_value: p.positions_value,
            equity: p.equity,
            net_contributions: p.net_contributions,
            funding_adjusted_equity: p.funding_adjusted_equity,
        }
    }
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct EquityHistoryHealthGql {
    pub rebuilt_at: String,
    pub reconstructed_equity: Option<f64>,
    pub reported_equity: Option<f64>,
    pub drift: Option<f64>,
    pub unclassified_types: Vec<String>,
    pub excluded_option_txns: i64,
    pub foreign_currency_txns: i64,
    pub missing_price_days: i64,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct AccountEquityHistoryGql {
    pub workspace_id: String,
    pub points: Vec<EquityHistoryPointGql>,
    pub health: Option<EquityHistoryHealthGql>,
}

fn health_gql(
    rebuilt_at: String,
    reconstructed: Option<f64>,
    reported: Option<f64>,
    drift: Option<f64>,
    h: ReplayHealth,
) -> EquityHistoryHealthGql {
    EquityHistoryHealthGql {
        rebuilt_at,
        reconstructed_equity: reconstructed,
        reported_equity: reported,
        drift,
        unclassified_types: h.unclassified_types,
        excluded_option_txns: h.excluded_option_txns as i64,
        foreign_currency_txns: h.foreign_currency_txns as i64,
        missing_price_days: h.missing_price_days as i64,
    }
}

async fn get_user_db(ctx: &Context<'_>) -> Result<crate::service::db::client::UserDb> {
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
pub struct EquityQuery;

#[Object]
impl EquityQuery {
    async fn account_equity_history(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        from: Option<String>,
    ) -> Result<AccountEquityHistoryGql> {
        let user_db = get_user_db(ctx).await?;
        let from = match from.as_deref() {
            Some(s) => Some(
                NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map_err(|_| async_graphql::Error::new("from must be YYYY-MM-DD"))?,
            ),
            None => None,
        };

        rebuild::rebuild_if_stale(user_db.pool(), user_db.user_id(), &workspace_id).await?;

        let points =
            equity_table::equity_history(user_db.pool(), user_db.user_id(), &workspace_id, from)
                .await?;
        let health = equity_table::rebuild_health(user_db.pool(), user_db.user_id(), &workspace_id)
            .await?
            .map(|r| {
                health_gql(
                    r.rebuilt_at,
                    r.reconstructed_equity,
                    r.reported_equity,
                    r.drift,
                    r.health,
                )
            });

        Ok(AccountEquityHistoryGql {
            workspace_id,
            points: points.into_iter().map(Into::into).collect(),
            health,
        })
    }
}

#[derive(Default)]
pub struct EquityMutation;

#[Object]
impl EquityMutation {
    async fn rebuild_account_equity_history(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<AccountEquityHistoryGql> {
        let user_db = get_user_db(ctx).await?;
        let report =
            rebuild::rebuild_account_equity(user_db.pool(), user_db.user_id(), &workspace_id)
                .await?;

        let points =
            equity_table::equity_history(user_db.pool(), user_db.user_id(), &workspace_id, None)
                .await?;

        Ok(AccountEquityHistoryGql {
            workspace_id,
            points: points.into_iter().map(Into::into).collect(),
            health: Some(health_gql(
                chrono::Utc::now().to_rfc3339(),
                report.reconstructed_equity,
                report.reported_equity,
                match (report.reconstructed_equity, report.reported_equity) {
                    (Some(r), Some(p)) => Some(r - p),
                    _ => None,
                },
                report.health,
            )),
        })
    }
}
