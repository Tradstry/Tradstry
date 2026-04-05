mod ai;
mod accounts;
mod analytics;
mod brokerage;
pub mod chat;
mod journal;
mod notebook;
mod playbook;
mod user_agents;
mod notebook_assistance;
mod user_prompts;
mod users;

use async_graphql::{MergedObject, MergedSubscription, Schema};

#[derive(MergedObject, Default)]
pub struct Query(
    ai::AiQuery,
    brokerage::BrokerageQuery,
    chat::ChatQuery,
    users::UserQuery,
    accounts::AccountQuery,
    analytics::AnalyticsQuery,
    playbook::PlaybookQuery,
    user_agents::UserAgentQuery,
    user_prompts::UserPromptQuery,
    journal::JournalQuery,
    notebook::NotebookQuery,
);

#[derive(MergedObject, Default)]
pub struct Mutation(
    ai::AiMutation,
    brokerage::BrokerageMutation,
    chat::ChatMutation,
    accounts::AccountMutation,
    playbook::PlaybookMutation,
    user_agents::UserAgentMutation,
    user_prompts::UserPromptMutation,
    journal::JournalMutation,
    notebook::NotebookMutation,
    notebook_assistance::NotebookAssistanceMutation,
);

#[derive(MergedSubscription, Default)]
pub struct Subscription(ai::AiSubscription, chat::ChatSubscription);

pub fn build_schema(
    brokerage_client: std::sync::Arc<crate::service::brokerage::client::BrokerageClient>,
    checkpoint_saver: std::sync::Arc<dyn langgraph::prelude::CheckpointSaver>,
    memory_store: Option<std::sync::Arc<dyn langgraph::prelude::Store>>,
) -> AppSchema {
    let mut builder = Schema::build(Query::default(), Mutation::default(), Subscription::default())
        .data(brokerage_client)
        .data(checkpoint_saver);

    if let Some(store) = memory_store {
        builder = builder.data(store);
    }

    builder.finish()
}

pub type AppSchema = Schema<Query, Mutation, Subscription>;
