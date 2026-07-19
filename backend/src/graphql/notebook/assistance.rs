use async_graphql::{Context, Object, Result};
use std::sync::Arc;

use crate::graphql::billing_guard;
use crate::service::ai::chat::assistance::{autocomplete, summary};
use crate::service::ai::client::AgentsClient;

#[derive(Default)]
pub struct NotebookAssistanceMutation;

#[Object]
impl NotebookAssistanceMutation {
    /// Deliberately unmetered: autocomplete fires on a keystroke pause, so
    /// charging it an AI action would drain a month's quota in an afternoon.
    /// It is bounded by a per-minute rate limit instead.
    async fn notebook_autocomplete(
        &self,
        ctx: &Context<'_>,
        title: String,
        text: String,
    ) -> Result<String> {
        let (_, user_id) = billing_guard::current_user(ctx).await?;
        billing_guard::check_autocomplete_rate(ctx, &user_id).await?;

        let agents = ctx.data::<Arc<AgentsClient>>()?;
        let completion = autocomplete::complete(agents, &title, &text).await?;
        Ok(completion)
    }

    async fn notebook_transform(
        &self,
        ctx: &Context<'_>,
        text: String,
        action: String,
    ) -> Result<String> {
        let (_, user_id) = billing_guard::current_user(ctx).await?;
        billing_guard::reserve_ai(ctx, &user_id).await?;

        let agents = ctx.data::<Arc<AgentsClient>>()?;
        let result = summary::transform(agents, &text, &action).await?;
        Ok(result)
    }
}
