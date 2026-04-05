use async_graphql::{Context, Object, Result};
use std::sync::Arc;

use crate::service::agents::client::AgentsClient;
use crate::service::chat::assistance::{autocomplete, summary};

#[derive(Default)]
pub struct NotebookAssistanceMutation;

#[Object]
impl NotebookAssistanceMutation {
    async fn notebook_autocomplete(
        &self,
        ctx: &Context<'_>,
        text: String,
        cursor_position: i32,
    ) -> Result<String> {
        let agents = ctx.data::<Arc<AgentsClient>>()?;
        let completion = autocomplete::complete(agents, &text, cursor_position as usize).await?;
        Ok(completion)
    }

    async fn notebook_transform(
        &self,
        ctx: &Context<'_>,
        text: String,
        action: String,
    ) -> Result<String> {
        let agents = ctx.data::<Arc<AgentsClient>>()?;
        let result = summary::transform(agents, &text, &action).await?;
        Ok(result)
    }
}
