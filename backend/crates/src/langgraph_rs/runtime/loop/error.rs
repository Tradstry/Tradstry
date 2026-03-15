use thiserror::Error;

use crate::langgraph_rs::{
    checkpoint::base::CheckpointError,
    core::{
        channels::ChannelError,
        scheduler::SchedulerError,
        types::{Command, NodeExecutionError, NodeExecutionErrorKind},
    },
    runtime::runner::RunnerError,
};

#[derive(Debug, Error)]
pub enum LoopError {
    #[error("scheduler error: {0}")]
    Scheduler(#[from] SchedulerError),
    #[error("checkpoint error: {0}")]
    Checkpoint(#[from] CheckpointError),
    #[error("channel error: {0}")]
    Channel(#[from] ChannelError),
    #[error("runner error: {0}")]
    Runner(#[from] RunnerError),
    #[error("parent command bubbled from task '{task_id}' in node '{node}'")]
    ParentCommand {
        task_id: String,
        node: String,
        command: Command,
    },
    #[error("received no input")]
    EmptyInput,
    #[error("there is no parent graph")]
    InvalidCommandGraph,
    #[error("invalid resume usage: {message}")]
    InvalidResumeUsage { message: String },
    #[error("missing schedule spec for node '{node}'")]
    MissingNodeSpec { node: String },
    #[error("task execution failed for task '{task_id}' in node '{node}' ({kind:?}): {message}")]
    TaskExecution {
        task_id: String,
        node: String,
        kind: NodeExecutionErrorKind,
        message: String,
    },
}

impl LoopError {
    pub fn task_execution(
        task_id: impl Into<String>,
        node: impl Into<String>,
        err: NodeExecutionError,
    ) -> Self {
        Self::TaskExecution {
            task_id: task_id.into(),
            node: node.into(),
            kind: err.kind,
            message: err.message,
        }
    }
}
