pub mod db_query;
pub mod semantic_search;
pub mod analytics_calc;
pub mod recall_memory;

use anyhow::Result;
use crate::service::turso::TursoClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;
use std::sync::Arc;

pub async fn execute_tool(
    name: &str,
    arguments: &str,
    user_id: &str,
    account_id: &str,
    turso: &Arc<TursoClient>,
    qdrant: &Arc<VectorDatabaseClient>,
) -> Result<String> {
    match name {
        "db_query" => db_query::execute(arguments, user_id, account_id, turso).await,
        "semantic_search" => semantic_search::execute(arguments, user_id, account_id, qdrant).await,
        "analytics_calc" => analytics_calc::execute(arguments, user_id, account_id, turso).await,
        "recall_memory" => recall_memory::execute(arguments, user_id, qdrant).await,
        _ => Ok(format!("Unknown tool: {}", name)),
    }
}

pub fn tool_schemas() -> Vec<crate::service::chat::types::GroqToolDef> {
    vec![db_query::schema(), semantic_search::schema(), analytics_calc::schema(), recall_memory::schema()]
}
