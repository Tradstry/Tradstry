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
        command: Box<Command>,
    },
}

impl RunnerError {
    pub fn from_join_error(err: tokio::task::JoinError) -> Self {
        let message = if err.is_cancelled() {
            "task was cancelled".to_owned()
        } else {
            format!("task panicked: {err}")
        };
        RunnerError::Execution {
            task_id: "<join>".to_owned(),
            node: "<join>".to_owned(),
            attempts: 0,
            kind: NodeExecutionErrorKind::Fatal,
            message,
        }
    }
}
