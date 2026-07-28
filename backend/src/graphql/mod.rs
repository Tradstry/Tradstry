mod accounts;
mod ai;
mod analytics;
mod brokerage;
pub mod chat;
pub mod equity;
mod journal;
pub mod notifications;
mod playbook;
mod position_calculator;
mod principle;
pub(crate) mod tags;
mod user_agents;
mod user_prompts;
mod users;

pub mod notebook;

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
    principle::PrincipleQuery,
    user_agents::UserAgentQuery,
    user_prompts::UserPromptQuery,
    journal::JournalQuery,
    notebook::base::NotebookQuery,
    notebook::sync::NotebookSyncQuery,
    notebook::crdt::NotebookCrdtQuery,
    position_calculator::PositionCalculatorQuery,
    tags::TagQuery,
    equity::EquityQuery,
    notifications::NotificationQuery,
);

#[derive(MergedObject, Default)]
pub struct Mutation(
    ai::AiMutation,
    brokerage::BrokerageMutation,
    chat::ChatMutation,
    accounts::AccountMutation,
    playbook::PlaybookMutation,
    principle::PrincipleMutation,
    user_agents::UserAgentMutation,
    user_prompts::UserPromptMutation,
    journal::JournalMutation,
    notebook::base::NotebookMutation,
    notebook::sync::NotebookSyncMutation,
    notebook::crdt::NotebookCrdtMutation,
    notebook::assistance::NotebookAssistanceMutation,
    position_calculator::PositionCalculatorMutation,
    tags::TagMutation,
    equity::EquityMutation,
    notifications::NotificationMutation,
);

#[derive(MergedSubscription, Default)]
pub struct Subscription(
    ai::AiSubscription,
    chat::ChatSubscription,
    notifications::NotificationSubscription,
);

pub fn build_schema(
    brokerage_client: std::sync::Arc<crate::service::brokerage::client::BrokerageClient>,
    checkpoint_saver: std::sync::Arc<dyn langgraph::prelude::CheckpointSaver>,
    memory_store: Option<std::sync::Arc<dyn langgraph::prelude::Store>>,
    redis_client: Option<std::sync::Arc<crate::service::redis::client::RedisClient>>,
    notification_events: notifications::NotificationEventBus,
) -> AppSchema {
    let mut builder = Schema::build(
        Query::default(),
        Mutation::default(),
        Subscription::default(),
    )
    .data(brokerage_client)
    .data(checkpoint_saver)
    .data(notification_events);

    if let Some(store) = memory_store {
        builder = builder.data(store);
    }

    if let Some(redis) = redis_client {
        builder = builder.data(redis);
    }

    builder.finish()
}

pub type AppSchema = Schema<Query, Mutation, Subscription>;
