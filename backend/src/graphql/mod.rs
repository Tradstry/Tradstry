mod ai;
mod accounts;
mod analytics;
pub mod chat;
mod journal;
mod notebook;
mod playbook;
mod users;

use async_graphql::{MergedObject, MergedSubscription, Schema};

#[derive(MergedObject, Default)]
pub struct Query(
    ai::AiQuery,
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
    chat::ChatMutation,
    accounts::AccountMutation,
    playbook::PlaybookMutation,
    journal::JournalMutation,
    notebook::NotebookMutation,
);

#[derive(MergedSubscription, Default)]
pub struct Subscription(ai::AiSubscription, chat::ChatSubscription);

pub fn build_schema() -> AppSchema {
    Schema::build(Query::default(), Mutation::default(), Subscription::default()).finish()
}

pub type AppSchema = Schema<Query, Mutation, Subscription>;
