mod ai;
mod accounts;
mod analytics;
mod brokerage;
pub mod chat;
mod journal;
mod notebook;
mod playbook;
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
    journal::JournalMutation,
    notebook::NotebookMutation,
);

#[derive(MergedSubscription, Default)]
pub struct Subscription(ai::AiSubscription, chat::ChatSubscription);

pub fn build_schema(brokerage_client: std::sync::Arc<crate::service::brokerage::client::BrokerageClient>) -> AppSchema {
    Schema::build(Query::default(), Mutation::default(), Subscription::default())
        .data(brokerage_client)
        .finish()
}

pub type AppSchema = Schema<Query, Mutation, Subscription>;
