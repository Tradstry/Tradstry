//! Plan and usage for the Settings screen, plus the two Paddle handoffs.
//!
//! Read-only apart from `createBillingPortalSession`, which mints a URL and
//! changes nothing here — Paddle owns cards, invoices and cancellation, so we
//! never build that UI ourselves.

use std::sync::Arc;

use async_graphql::{Context, Object, Result, SimpleObject};

use super::billing_guard;
use crate::service::billing::entitlements::{self, Entitlements, Meter};
use crate::service::db::schema::tables::billing_table;
use crate::service::redis::client::RedisClient;

/// One metered resource. `limit: None` renders as unlimited.
#[derive(SimpleObject)]
pub struct MeterInfo {
    pub used: i64,
    pub limit: Option<i64>,
}

impl From<&Meter> for MeterInfo {
    fn from(meter: &Meter) -> Self {
        MeterInfo {
            used: meter.used,
            limit: meter.limit,
        }
    }
}

#[derive(SimpleObject)]
pub struct BillingMeters {
    pub ai: MeterInfo,
    pub connections: MeterInfo,
    pub data: MeterInfo,
    pub media: MeterInfo,
}

#[derive(SimpleObject)]
pub struct BillingInfo {
    /// The plan actually in force, which during a grace or cancellation window
    /// is not the same as the subscribed tier.
    pub plan: String,
    pub status: Option<String>,
    /// End of the current quota window — the date usage resets.
    pub period_end: String,
    pub meters: BillingMeters,
    /// True while a canceled subscription is still inside its paid period.
    pub cancels_at_period_end: bool,
}

impl From<&Entitlements> for BillingInfo {
    fn from(ent: &Entitlements) -> Self {
        BillingInfo {
            plan: ent.plan.clone(),
            status: ent.status.clone(),
            period_end: ent.period_end.to_rfc3339(),
            meters: BillingMeters {
                ai: (&ent.ai).into(),
                connections: (&ent.connections).into(),
                data: (&ent.data).into(),
                media: (&ent.media).into(),
            },
            cancels_at_period_end: ent.status.as_deref() == Some("canceled"),
        }
    }
}

/// What Paddle.js needs to open a checkout for a tier.
#[derive(SimpleObject)]
pub struct CheckoutInfo {
    pub price_id: String,
    /// Pre-fills the checkout for a returning customer.
    pub paddle_customer_id: Option<String>,
    /// Passed back as `custom_data.user_id` at checkout — this is what the
    /// webhook matches the resulting subscription to a user with.
    pub user_id: String,
}

#[derive(Default)]
pub struct BillingQuery;

#[Object]
impl BillingQuery {
    /// Current plan and usage. Resolved fresh per request from the cache the
    /// webhook invalidates, so an upgrade shows up on the next call.
    async fn billing(&self, ctx: &Context<'_>) -> Result<BillingInfo> {
        let (db, user_id) = billing_guard::current_user(ctx).await?;
        let redis = ctx.data::<Arc<RedisClient>>().ok().cloned();

        let ent = entitlements::resolve(db.pool(), redis.as_deref(), &user_id).await?;
        Ok((&ent).into())
    }

    /// Price id for a tier plus the caller's Paddle customer id, if any.
    async fn checkout_info(&self, ctx: &Context<'_>, plan: String) -> Result<CheckoutInfo> {
        let (db, user_id) = billing_guard::current_user(ctx).await?;

        let price_id = match plan.as_str() {
            entitlements::PRO => std::env::var("PADDLE_PRICE_PRO").ok(),
            entitlements::PRO_PLUS => std::env::var("PADDLE_PRICE_PRO_PLUS").ok(),
            other => return Err(async_graphql::Error::new(format!("Unknown plan {other}"))),
        }
        .filter(|id| !id.is_empty())
        .ok_or_else(|| {
            async_graphql::Error::new(format!("No Paddle price configured for plan {plan}"))
        })?;

        let paddle_customer_id = billing_table::plan_state(db.pool(), &user_id)
            .await?
            .and_then(|state| state.paddle_customer_id);

        Ok(CheckoutInfo {
            price_id,
            paddle_customer_id,
            user_id,
        })
    }
}

#[derive(Default)]
pub struct BillingMutation;

#[Object]
impl BillingMutation {
    /// Mint a Paddle-hosted portal URL for managing payment method, invoices
    /// and cancellation. Returns null when the caller has no Paddle customer
    /// yet (i.e. has never subscribed) — the frontend shows upgrade instead.
    async fn create_billing_portal_session(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        let (db, user_id) = billing_guard::current_user(ctx).await?;

        let Some(state) = billing_table::plan_state(db.pool(), &user_id).await? else {
            return Ok(None);
        };
        let Some(customer_id) = state.paddle_customer_id else {
            return Ok(None);
        };

        let url = crate::service::billing::portal::create_portal_session(
            &customer_id,
            state.paddle_subscription_id.as_deref(),
        )
        .await
        .map_err(|e| {
            log::error!("[billing] portal session failed for {user_id}: {e:#}");
            async_graphql::Error::new("Could not open the billing portal. Please try again.")
        })?;

        Ok(Some(url))
    }
}
