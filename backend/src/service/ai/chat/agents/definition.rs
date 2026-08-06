use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::service::db::schema::tables::user_agents_table::UserAgent;

// ---------------------------------------------------------------------------
// AgentStep
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub tool: String,
    pub args: Value,
    pub output_channel: Option<String>,
}

// ---------------------------------------------------------------------------
// AgentDefinition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AgentDefinition {
    pub goal: String,
    pub steps: Vec<AgentStep>,
    pub output_style: String,
    pub user_id: String,
    pub workspace_id: String,
}

impl AgentDefinition {
    /// Parse an `AgentDefinition` from the database `UserAgent` row.
    pub fn from_db(agent: &UserAgent) -> Result<Self> {
        let steps: Vec<AgentStep> = serde_json::from_str(&agent.steps_json)
            .with_context(|| format!("Failed to parse steps_json for agent '{}'", agent.id))?;

        Ok(Self {
            goal: agent.goal.clone(),
            steps,
            output_style: agent.output_style.clone(),
            user_id: agent.user_id.clone(),
            workspace_id: agent.workspace_id.clone(),
        })
    }

    /// Apply runtime parameter overrides to step args.
    ///
    /// Supported override keys: `symbol`, `date_from`, `date_to`, `query`.
    /// Each matching key is merged into every step's `args` object.
    pub fn with_overrides(mut self, overrides: &Value) -> Self {
        let override_keys = ["symbol", "date_from", "date_to", "query"];

        for step in self.steps.iter_mut() {
            if let Some(args_obj) = step.args.as_object_mut() {
                for key in &override_keys {
                    if let Some(val) = overrides.get(key) {
                        args_obj.insert(key.to_string(), val.clone());
                    }
                }
            }
        }

        self
    }
}
