use async_graphql::{Context, Object, Result, SimpleObject, Subscription};
use chrono::{DateTime, Utc};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::service::db::Db;
use crate::service::db::client::UserDb;
use crate::service::notifications::{preferences, store, subscriptions};
use crate::service::read_service::users::ensure_user;

#[derive(Debug, Clone)]
pub struct NotificationPushed {
    pub user_id: String,
    pub notification_id: String,
}

pub type NotificationEventBus = tokio::sync::broadcast::Sender<NotificationPushed>;

async fn get_user_db(ctx: &Context<'_>) -> Result<UserDb> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let db = ctx.data::<Arc<Db>>()?;
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
    let user = ensure_user(db.pool(), &jwt.sub, full_name, email).await?;
    Ok(db.get_user_db(&user.id))
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotificationGql {
    pub id: String,
    pub event_type: String,
    pub title: String,
    pub body: String,
    pub deep_link: Option<String>,
    pub group_count: i64,
    pub read: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<store::FeedRow> for NotificationGql {
    fn from(r: store::FeedRow) -> Self {
        Self {
            id: r.id,
            event_type: r.event_type,
            title: r.title,
            body: r.body,
            deep_link: r.deep_link,
            group_count: r.group_count,
            read: r.read_at.is_some(),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotificationPreferenceGql {
    pub event_type: String,
    pub enabled: bool,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct PushSubscriptionGql {
    pub endpoint: String,
}

#[derive(Default)]
pub struct NotificationQuery;

#[Object]
impl NotificationQuery {
    async fn notifications(
        &self,
        ctx: &Context<'_>,
        limit: Option<i64>,
        before: Option<String>,
    ) -> Result<Vec<NotificationGql>> {
        let user_db = get_user_db(ctx).await?;
        let before = match before.as_deref() {
            Some(s) => Some(
                DateTime::parse_from_rfc3339(s)
                    .map_err(|_| async_graphql::Error::new("before must be RFC3339"))?
                    .with_timezone(&Utc),
            ),
            None => None,
        };
        let rows = store::feed(
            user_db.pool(),
            user_db.user_id(),
            limit.unwrap_or(50).clamp(1, 100),
            before,
        )
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn unread_notification_count(&self, ctx: &Context<'_>) -> Result<i64> {
        let user_db = get_user_db(ctx).await?;
        Ok(store::unread_count(user_db.pool(), user_db.user_id()).await?)
    }

    async fn notification_preferences(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<NotificationPreferenceGql>> {
        let user_db = get_user_db(ctx).await?;
        Ok(preferences::list(user_db.pool(), user_db.user_id())
            .await?
            .into_iter()
            .map(|r| NotificationPreferenceGql {
                event_type: r.event_type,
                enabled: r.enabled,
            })
            .collect())
    }

    /// Served from the server so the key lives in one place instead of being
    /// duplicated into frontend build config.
    async fn web_push_public_key(&self, _ctx: &Context<'_>) -> Result<Option<String>> {
        Ok(std::env::var("VAPID_PUBLIC_KEY")
            .ok()
            .filter(|k| !k.is_empty()))
    }

    async fn push_subscriptions(&self, ctx: &Context<'_>) -> Result<Vec<PushSubscriptionGql>> {
        let user_db = get_user_db(ctx).await?;
        Ok(
            subscriptions::list_for_user(user_db.pool(), user_db.user_id())
                .await?
                .into_iter()
                .map(|s| PushSubscriptionGql {
                    endpoint: s.endpoint,
                })
                .collect(),
        )
    }
}

#[derive(Default)]
pub struct NotificationMutation;

#[Object]
impl NotificationMutation {
    async fn mark_notification_read(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(store::mark_read(user_db.pool(), user_db.user_id(), &id).await?)
    }

    async fn mark_all_notifications_read(&self, ctx: &Context<'_>) -> Result<i64> {
        let user_db = get_user_db(ctx).await?;
        Ok(store::mark_all_read(user_db.pool(), user_db.user_id()).await? as i64)
    }

    async fn set_notification_preference(
        &self,
        ctx: &Context<'_>,
        event_type: String,
        enabled: bool,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        preferences::set(user_db.pool(), user_db.user_id(), &event_type, enabled).await?;
        Ok(enabled)
    }

    async fn register_push_subscription(
        &self,
        ctx: &Context<'_>,
        endpoint: String,
        p256dh: String,
        auth: String,
        user_agent: Option<String>,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        subscriptions::upsert(
            user_db.pool(),
            user_db.user_id(),
            &endpoint,
            &p256dh,
            &auth,
            user_agent.as_deref(),
        )
        .await?;
        Ok(true)
    }

    async fn delete_push_subscription(&self, ctx: &Context<'_>, endpoint: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(subscriptions::delete_by_endpoint(user_db.pool(), user_db.user_id(), &endpoint).await?)
    }
}

#[derive(Default)]
pub struct NotificationSubscription;

#[Subscription]
impl NotificationSubscription {
    /// A UI convenience only. The feed query stays the source of truth, so a
    /// dropped broadcast message costs nothing but a delayed badge.
    async fn notification_events(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl futures_util::Stream<Item = String>> {
        let user_db = get_user_db(ctx).await?;
        let user_id = user_db.user_id().to_string();
        let bus = ctx.data::<NotificationEventBus>()?.clone();
        Ok(
            BroadcastStream::new(bus.subscribe()).filter_map(move |item| {
                let user_id = user_id.clone();
                match item.ok() {
                    Some(event) if event.user_id == user_id => Some(event.notification_id),
                    _ => None,
                }
            }),
        )
    }
}
