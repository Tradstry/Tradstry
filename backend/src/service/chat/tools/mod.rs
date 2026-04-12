pub mod analytics_calc;
pub mod company_info;
pub mod create_agent;
pub mod db_query;
pub mod earnings;
pub mod edit_agent;
pub mod financials;
pub mod recall_memory;
pub mod run_agent;
pub mod save_agent;
pub mod semantic_search;
pub mod stock_news;
pub mod stock_quote;

use crate::service::agents::client::AgentsClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;
use crate::service::turso::TursoClient;
use anyhow::Result;
use std::sync::Arc;

#[allow(clippy::too_many_arguments)]
pub async fn execute_tool(
    name: &str,
    arguments: &str,
    user_id: &str,
    account_id: &str,
    turso: &Arc<TursoClient>,
    qdrant: &Arc<VectorDatabaseClient>,
    agents: Option<&Arc<AgentsClient>>,
    _checkpoint_saver: Option<&Arc<dyn langgraph::prelude::CheckpointSaver>>,
    conversation_messages: Option<&serde_json::Value>,
) -> Result<String> {
    match name {
        "db_query" => db_query::execute(arguments, user_id, account_id, turso).await,
        "semantic_search" => semantic_search::execute(arguments, user_id, account_id, qdrant).await,
        "analytics_calc" => analytics_calc::execute(arguments, user_id, account_id, turso).await,
        "recall_memory" => {
            recall_memory::execute(arguments, user_id, qdrant, conversation_messages).await
        }
        "create_agent" => create_agent::execute(arguments),
        "save_agent" => save_agent::execute(arguments, user_id, account_id, turso).await,
        "run_agent" => {
            let agents = agents
                .ok_or_else(|| anyhow::anyhow!("AgentsClient not available for run_agent"))?;
            run_agent::execute(arguments, user_id, account_id, agents, turso, qdrant).await
        }
        "edit_agent" => edit_agent::execute(arguments, user_id, account_id, turso).await,
        "stock_quote" => stock_quote::execute(arguments).await,
        "stock_news" => stock_news::execute(arguments).await,
        "financials" => financials::execute(arguments).await,
        "earnings" => earnings::execute(arguments).await,
        "company_info" => company_info::execute(arguments).await,
        _ => Ok(format!("Unknown tool: {}", name)),
    }
}

pub fn tool_schemas() -> Vec<crate::service::chat::types::GroqToolDef> {
    vec![
        db_query::schema(),
        semantic_search::schema(),
        analytics_calc::schema(),
        recall_memory::schema(),
        create_agent::schema(),
        save_agent::schema(),
        run_agent::schema(),
        edit_agent::schema(),
        stock_quote::schema(),
        stock_news::schema(),
        financials::schema(),
        earnings::schema(),
        company_info::schema(),
        crate::service::chat::subgraphs::research::tool_schema(),
        crate::service::chat::subgraphs::report::tool_schema(),
        crate::service::chat::subgraphs::comparison::tool_schema(),
    ]
}
