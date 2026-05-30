use serde::{Deserialize, Serialize};

use crate::langgraph_rs::core::types::ExecutionContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContext {
    pub node_name: String,
    pub task_id: String,
    pub task_name: String,
    pub step: u64,
    pub recursion_limit: u64,
}

impl AdapterContext {
    pub fn from_execution_context(node_name: impl Into<String>, ctx: &ExecutionContext) -> Self {
        Self {
            node_name: node_name.into(),
            task_id: ctx.task.id.clone(),
            task_name: ctx.task.name.clone(),
            step: ctx.step,
            recursion_limit: ctx.recursion_limit,
        }
    }
}
