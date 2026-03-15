use thiserror::Error;

use crate::langgraph_rs::core::types::{Command, NodeExecutionErrorKind};

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error(
        "task execution failed for task '{task_id}' in node '{node}' after {attempts} attempt(s) ({kind:?}): {message}"
    )]
    Execution {
        task_id: String,
        node: String,
        attempts: u32,
        kind: NodeExecutionErrorKind,
        message: String,
    },
    #[error("parent command bubbled from task '{task_id}' in node '{node}'")]
    ParentCommand {
        task_id: String,
        node: String,
        command: Command,
    },
}
