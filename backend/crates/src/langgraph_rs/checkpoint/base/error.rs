use thiserror::Error;

use crate::langgraph_rs::core::channels::ChannelError;

#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("channel operation failed: {0}")]
    Channel(#[from] ChannelError),
    #[error("invalid checkpoint config: {message}")]
    InvalidConfig { message: String },
    #[error("operation '{operation}' is not implemented")]
    NotImplemented { operation: &'static str },
    #[error("capability '{capability}' is not supported")]
    UnsupportedCapability { capability: &'static str },
    #[error("serialization error: {message}")]
    Serialization { message: String },
    #[error("storage error: {message}")]
    Storage { message: String },
}

impl CheckpointError {
    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::InvalidConfig {
            message: message.into(),
        }
    }

    pub fn not_implemented(operation: &'static str) -> Self {
        Self::NotImplemented { operation }
    }

    pub fn unsupported_capability(capability: &'static str) -> Self {
        Self::UnsupportedCapability { capability }
    }

    pub fn serialization(message: impl Into<String>) -> Self {
        Self::Serialization {
            message: message.into(),
        }
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage {
            message: message.into(),
        }
    }
}
