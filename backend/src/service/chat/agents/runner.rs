use std::sync::Arc;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::service::agents::client::AgentsClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;
use crate::service::turso::TursoClient;
use crate::service::turso::schema::tables::user_agents_table;

use super::compiler::compile_agent;
use super::definition::AgentDefinition;

// ---------------------------------------------------------------------------
// run_user_agent — loads agent by ID, compiles, runs, returns synthesis text
// ---------------------------------------------------------------------------
pub async fn run_user_agent(
    agent_id: &str,
    user_id: &str,
    _account_id: &str,
    overrides: &Value,
    agents: &Arc<AgentsClient>,
    turso: &Arc<TursoClient>,
    qdrant: &Arc<VectorDatabaseClient>,
) -> Result<String> {
    let conn = turso.get_connection()?;

    let db_agent = user_agents_table::find_user_agent(&conn, agent_id, user_id)
        .await?
        .ok_or_else(|| anyhow!("Agent '{}' not found", agent_id))?;

    let def = AgentDefinition::from_db(&db_agent)?.with_overrides(overrides);

    let compiled = compile_agent(
        &def,
        Arc::clone(agents),
        Arc::clone(turso),
        Arc::clone(qdrant),
    )
    .map_err(|e| anyhow!("Failed to compile agent: {e:?}"))?;

    let config = langgraph::prelude::LoopConfig::new(langgraph::prelude::CheckpointConfig::new(
        format!("agent-{}", agent_id),
    ))
    .with_recursion_limit(20);

    let summary = compiled
        .run_raw(None, config, json!({}))
        .map_err(|e| anyhow!("Agent execution failed: {e:?}"))?;

    let synthesis = summary
        .checkpoint
        .channel_values
        .get("synthesis")
        .and_then(|v| v.as_str())
        .unwrap_or("Agent completed but produced no synthesis.")
        .to_owned();

    Ok(synthesis)
}

// ---------------------------------------------------------------------------
// run_user_agent_by_name — finds agent by name then delegates to run_user_agent
// ---------------------------------------------------------------------------
pub async fn run_user_agent_by_name(
    name: &str,
    user_id: &str,
    account_id: &str,
    overrides: &Value,
    agents: &Arc<AgentsClient>,
    turso: &Arc<TursoClient>,
    qdrant: &Arc<VectorDatabaseClient>,
) -> Result<String> {
    let conn = turso.get_connection()?;

    let db_agent = user_agents_table::find_user_agent_by_name(&conn, name, user_id, account_id)
        .await?
        .ok_or_else(|| anyhow!("Agent '{}' not found", name))?;

    run_user_agent(
        &db_agent.id,
        user_id,
        account_id,
        overrides,
        agents,
        turso,
        qdrant,
    )
    .await
}

// ---------------------------------------------------------------------------
// list_agent_names — returns Vec<String> of agent names for the user
// ---------------------------------------------------------------------------
pub async fn list_agent_names(
    turso: &Arc<TursoClient>,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<String>> {
    let conn = turso.get_connection()?;
    let agents = user_agents_table::list_user_agents(&conn, user_id, account_id).await?;
    Ok(agents.into_iter().map(|a| a.name).collect())
}
