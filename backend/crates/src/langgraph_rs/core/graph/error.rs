use thiserror::Error;

use crate::langgraph_rs::runtime::r#loop::LoopError;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("duplicate channel '{channel}'")]
    DuplicateChannel { channel: String },
    #[error("duplicate node '{node}'")]
    DuplicateNode { node: String },
    #[error("unknown channel '{channel}'")]
    UnknownChannel { channel: String },
    #[error("unknown node '{node}'")]
    UnknownNode { node: String },
    #[error("invalid branch '{branch}': {message}")]
    InvalidBranch { branch: String, message: String },
    #[error(
        "conditional branch conflict in node '{from}': branch '{branch}' already maps to another edge"
    )]
    ConflictingConditionalRoute { from: String, branch: String },
    #[error("graph validation failed: {message}")]
    Validation { message: String },
    #[error("unknown branch target '{target}' from node '{from_node}'")]
    UnknownBranchTarget { from_node: String, target: String },
    #[error("invalid conditional path result for node '{from_node}': {message}")]
    InvalidConditionalPathResult { from_node: String, message: String },
    #[error("invalid managed field '{field}': {message}")]
    InvalidManagedField { field: String, message: String },
    #[error("invalid schema field '{field}': {message}")]
    InvalidSchemaField { field: String, message: String },
    #[error("graph must have an entrypoint")]
    MissingEntryPoint,
    #[error("runtime execution failed: {0}")]
    Loop(#[from] LoopError),
}

impl GraphError {
    pub fn unknown_node(node: impl Into<String>) -> Self {
        Self::UnknownNode { node: node.into() }
    }

    pub fn unknown_channel(channel: impl Into<String>) -> Self {
        Self::UnknownChannel {
            channel: channel.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }
}
