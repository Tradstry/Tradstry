mod accounts;
mod analytics;
mod ai_chat;
mod playbook;
mod journal;
mod notebook;
mod subscriptions;
mod users;

use async_graphql::{MergedObject, MergedSubscription, Schema};

#[derive(MergedObject, Default)]
pub struct Query(
    users::UserQuery,
    accounts::AccountQuery,
    analytics::AnalyticsQuery,
    ai_chat::AiChatQuery,
    playbook::PlaybookQuery,
    journal::JournalQuery,
    notebook::NotebookQuery,
);

#[derive(MergedObject, Default)]
pub struct Mutation(
    accounts::AccountMutation,
    playbook::PlaybookMutation,
    journal::JournalMutation,
    notebook::NotebookMutation,
    ai_chat::AiChatMutation,
);

#[derive(MergedSubscription, Default)]
pub struct Subscription(
    subscriptions::AiChatSubscription,
);

pub type AppSchema = Schema<Query, Mutation, Subscription>;

pub fn build_schema() -> AppSchema {
    Schema::build(Query::default(), Mutation::default(), Subscription::default()).finish()
}
