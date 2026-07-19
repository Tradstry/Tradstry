//! Resolving what a user's plan currently permits.
//!
//! One `Entitlements` object is resolved per request and passed down; individual
//! resolvers never re-derive plan logic. Cached in Redis and invalidated by the
//! webhook worker on any plan change — the TTL is only a backstop, because
//! waiting for it to expire would let a downgraded user keep a paid plan.

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::service::db::schema::tables::billing_table::{self, PlanState};
use crate::service::redis::client::RedisClient;

pub const FREE: &str = "free";
pub const PRO: &str = "pro";
pub const PRO_PLUS: &str = "pro_plus";
pub const AI_ACTIONS: &str = "ai_actions";

/// Cache TTL is a safety net only; correctness comes from explicit invalidation.
const CACHE_TTL: u64 = 300;

fn cache_key(user_id: &str) -> String {
    format!("billing:ent:{user_id}")
}

/// One metered resource. `limit: None` means unlimited.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meter {
    pub used: i64,
    pub limit: Option<i64>,
}

impl Meter {
    /// True when one more unit would exceed the limit.
    pub fn is_exhausted(&self) -> bool {
        self.limit.is_some_and(|l| self.used >= l)
    }

    /// True when `additional` more units would exceed the limit.
    pub fn would_exceed(&self, additional: i64) -> bool {
        self.limit.is_some_and(|l| self.used + additional > l)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entitlements {
    pub plan: String,
    pub status: Option<String>,
    pub ai: Meter,
    pub connections: Meter,
    pub data: Meter,
    pub media: Meter,
    /// Current quota window — billing period when paid, signup anniversary when free.
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

// ── plan resolution ─────────────────────────────────────────────────────────

/// The plan actually in force right now, which is not always `users.plan`.
///
/// A subscription that is canceled or past_due still confers its plan for a
/// while: Paddle keeps a canceling subscription `active` until the period ends,
/// and a failed payment gets a grace window before losing access.
pub fn effective_plan(state: &PlanState, now: DateTime<Utc>) -> String {
    if state.plan == FREE {
        return FREE.to_string();
    }

    // No subscription but a non-free plan means a manual/comp grant; honour it.
    if state.paddle_subscription_id.is_none() {
        return state.plan.clone();
    }

    match state.subscription_status.as_deref() {
        Some("active") | Some("trialing") | Some("resumed") => state.plan.clone(),
        Some("past_due") => match state.grace_until {
            Some(until) if now < until => state.plan.clone(),
            _ => FREE.to_string(),
        },
        Some("canceled") => match state.current_period_end {
            // Already paid through the end of the period.
            Some(end) if now < end => state.plan.clone(),
            _ => FREE.to_string(),
        },
        // paused, unknown, or missing: no entitlement.
        _ => FREE.to_string(),
    }
}

// ── quota windows ───────────────────────────────────────────────────────────

fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

fn prev_month(year: i32, month: u32) -> (i32, u32) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = next_month(year, month);
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("valid first of month");
    let next_first = NaiveDate::from_ymd_opt(ny, nm, 1).expect("valid first of next month");
    next_first.signed_duration_since(first).num_days() as u32
}

/// Anchor day clamped into a month that may be shorter — someone who signed up
/// on the 31st rolls on the 28th/29th/30th in shorter months.
fn clamped(year: i32, month: u32, day: u32) -> NaiveDate {
    let day = day.clamp(1, days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).expect("clamped day is valid")
}

fn start_of_day(date: NaiveDate) -> DateTime<Utc> {
    date.and_hms_opt(0, 0, 0)
        .expect("midnight is valid")
        .and_utc()
}

/// Monthly window anchored on the user's signup day-of-month.
pub fn anniversary_window(now: DateTime<Utc>, anchor_day: u32) -> (DateTime<Utc>, DateTime<Utc>) {
    let today = now.date_naive();
    let this_anchor = clamped(today.year(), today.month(), anchor_day);

    let start = if today >= this_anchor {
        this_anchor
    } else {
        let (py, pm) = prev_month(today.year(), today.month());
        clamped(py, pm, anchor_day)
    };

    let (ny, nm) = next_month(start.year(), start.month());
    let end = clamped(ny, nm, anchor_day);

    (start_of_day(start), start_of_day(end))
}

/// The window AI actions are counted in: the real billing period when the user
/// is on a paid plan, otherwise their signup anniversary month.
pub fn current_window(
    state: &PlanState,
    plan: &str,
    now: DateTime<Utc>,
) -> (DateTime<Utc>, DateTime<Utc>) {
    if plan != FREE
        && let (Some(start), Some(end)) = (state.current_period_start, state.current_period_end)
        && now >= start
        && now < end
    {
        return (start, end);
    }

    let anchor = state
        .quota_anchor_day
        .map(|d| d as u32)
        .unwrap_or_else(|| state.created_at.day());
    anniversary_window(now, anchor)
}

// ── resolution ──────────────────────────────────────────────────────────────

/// Build the entitlements for a user, preferring the Redis cache.
pub async fn resolve(
    pool: &PgPool,
    redis: Option<&RedisClient>,
    user_id: &str,
) -> Result<Entitlements> {
    if let Some(redis) = redis
        && let Some(raw) = redis.get(&cache_key(user_id)).await
        && let Ok(cached) = serde_json::from_str::<Entitlements>(&raw)
    {
        return Ok(cached);
    }

    let entitlements = load(pool, user_id).await?;

    if let Some(redis) = redis
        && let Ok(json) = serde_json::to_string(&entitlements)
    {
        redis.set_ex(&cache_key(user_id), &json, CACHE_TTL).await;
    }

    Ok(entitlements)
}

async fn load(pool: &PgPool, user_id: &str) -> Result<Entitlements> {
    let state = billing_table::plan_state(pool, user_id)
        .await?
        .context("user not found while resolving entitlements")?;

    let now = Utc::now();
    let plan = effective_plan(&state, now);
    let (period_start, period_end) = current_window(&state, &plan, now);

    // A plan with no catalog row must not silently grant unlimited access.
    let limits = billing_table::plan_limits(pool, &plan)
        .await?
        .with_context(|| format!("no plan_limits row for plan '{plan}'"))?;

    let ai_used = billing_table::counter_used(pool, user_id, AI_ACTIONS, period_start).await?;
    let connections_used = billing_table::active_connection_count(pool, user_id).await?;

    Ok(Entitlements {
        plan,
        status: state.subscription_status.clone(),
        ai: Meter {
            used: ai_used as i64,
            limit: limits.ai_actions_per_month.map(|v| v as i64),
        },
        connections: Meter {
            used: connections_used,
            limit: limits.brokerage_connections.map(|v| v as i64),
        },
        data: Meter {
            used: state.data_bytes_used,
            limit: limits.data_bytes,
        },
        media: Meter {
            used: state.media_bytes_used,
            limit: limits.media_bytes,
        },
        period_start,
        period_end,
    })
}

/// Drop the cached entitlements for a user. Called by the webhook worker on any
/// plan change, and after usage writes that must be reflected immediately.
pub async fn invalidate(redis: Option<&RedisClient>, user_id: &str) {
    if let Some(redis) = redis {
        redis.delete_by_prefix(&cache_key(user_id)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(plan: &str, created: &str) -> PlanState {
        PlanState {
            plan: plan.to_string(),
            paddle_customer_id: None,
            paddle_subscription_id: Some("sub_1".into()),
            subscription_status: Some("active".into()),
            subscription_updated_at: None,
            current_period_start: None,
            current_period_end: None,
            grace_until: None,
            quota_anchor_day: None,
            data_bytes_used: 0,
            media_bytes_used: 0,
            created_at: created.parse::<DateTime<Utc>>().expect("created_at"),
        }
    }

    fn at(ts: &str) -> DateTime<Utc> {
        ts.parse::<DateTime<Utc>>().expect("timestamp")
    }

    #[test]
    fn anchor_31_clamps_into_short_months() {
        // February: rolls on the 28th in a non-leap year.
        let (start, end) = anniversary_window(at("2026-02-15T12:00:00Z"), 31);
        assert_eq!(start.date_naive().to_string(), "2026-01-31");
        assert_eq!(end.date_naive().to_string(), "2026-02-28");

        // April has 30 days.
        let (start, end) = anniversary_window(at("2026-04-15T12:00:00Z"), 31);
        assert_eq!(start.date_naive().to_string(), "2026-03-31");
        assert_eq!(end.date_naive().to_string(), "2026-04-30");
    }

    #[test]
    fn anniversary_window_rolls_at_the_anchor() {
        // Before the anchor: still in the window that began last month.
        let (start, end) = anniversary_window(at("2026-07-10T00:00:00Z"), 14);
        assert_eq!(start.date_naive().to_string(), "2026-06-14");
        assert_eq!(end.date_naive().to_string(), "2026-07-14");

        // On the anchor: a fresh window starts.
        let (start, end) = anniversary_window(at("2026-07-14T00:00:00Z"), 14);
        assert_eq!(start.date_naive().to_string(), "2026-07-14");
        assert_eq!(end.date_naive().to_string(), "2026-08-14");
    }

    #[test]
    fn december_window_rolls_the_year() {
        let (start, end) = anniversary_window(at("2026-12-20T00:00:00Z"), 5);
        assert_eq!(start.date_naive().to_string(), "2026-12-05");
        assert_eq!(end.date_naive().to_string(), "2027-01-05");
    }

    #[test]
    fn past_due_keeps_plan_until_grace_expires() {
        let mut s = state("pro", "2026-01-14T00:00:00Z");
        s.subscription_status = Some("past_due".into());
        s.grace_until = Some(at("2026-07-20T00:00:00Z"));

        assert_eq!(effective_plan(&s, at("2026-07-19T00:00:00Z")), "pro");
        assert_eq!(effective_plan(&s, at("2026-07-21T00:00:00Z")), FREE);
    }

    #[test]
    fn cancel_keeps_plan_until_period_end() {
        let mut s = state("pro_plus", "2026-01-14T00:00:00Z");
        s.subscription_status = Some("canceled".into());
        s.current_period_end = Some(at("2026-08-01T00:00:00Z"));

        // Already paid through the period.
        assert_eq!(effective_plan(&s, at("2026-07-31T00:00:00Z")), "pro_plus");
        assert_eq!(effective_plan(&s, at("2026-08-02T00:00:00Z")), FREE);
    }

    #[test]
    fn paused_and_unknown_status_fall_back_to_free() {
        let mut s = state("pro", "2026-01-14T00:00:00Z");
        s.subscription_status = Some("paused".into());
        assert_eq!(effective_plan(&s, at("2026-07-19T00:00:00Z")), FREE);

        s.subscription_status = None;
        assert_eq!(effective_plan(&s, at("2026-07-19T00:00:00Z")), FREE);
    }

    #[test]
    fn manual_grant_without_a_subscription_is_honoured() {
        let mut s = state("pro", "2026-01-14T00:00:00Z");
        s.paddle_subscription_id = None;
        s.subscription_status = None;
        assert_eq!(effective_plan(&s, at("2026-07-19T00:00:00Z")), "pro");
    }

    #[test]
    fn paid_window_uses_the_billing_period() {
        let mut s = state("pro", "2026-01-14T00:00:00Z");
        s.current_period_start = Some(at("2026-07-03T00:00:00Z"));
        s.current_period_end = Some(at("2026-08-03T00:00:00Z"));

        let (start, end) = current_window(&s, "pro", at("2026-07-19T00:00:00Z"));
        assert_eq!(start, at("2026-07-03T00:00:00Z"));
        assert_eq!(end, at("2026-08-03T00:00:00Z"));
    }

    #[test]
    fn free_window_ignores_a_stale_billing_period() {
        let mut s = state("pro", "2026-01-14T00:00:00Z");
        s.quota_anchor_day = Some(14);
        // Period that has already elapsed must not be used.
        s.current_period_start = Some(at("2026-05-03T00:00:00Z"));
        s.current_period_end = Some(at("2026-06-03T00:00:00Z"));

        let (start, _) = current_window(&s, FREE, at("2026-07-19T00:00:00Z"));
        assert_eq!(start.date_naive().to_string(), "2026-07-14");
    }

    #[test]
    fn meter_limits() {
        let unlimited = Meter {
            used: 9_999,
            limit: None,
        };
        assert!(!unlimited.is_exhausted());
        assert!(!unlimited.would_exceed(1_000));

        let capped = Meter {
            used: 25,
            limit: Some(25),
        };
        assert!(capped.is_exhausted());

        let room = Meter {
            used: 24,
            limit: Some(25),
        };
        assert!(!room.is_exhausted());
        assert!(room.would_exceed(2));
    }
}
