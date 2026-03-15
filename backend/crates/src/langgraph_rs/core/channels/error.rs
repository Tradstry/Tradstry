use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChannelError {
    #[error("channel is empty")]
    EmptyChannel,
    #[error("invalid update at key '{key}': {message}")]
    InvalidUpdate { key: String, message: String },
    #[error("invalid checkpoint at key '{key}': {message}")]
    InvalidCheckpoint { key: String, message: String },
}

impl ChannelError {
    pub fn invalid_update(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidUpdate {
            key: key.into(),
            message: message.into(),
        }
    }

    pub fn invalid_checkpoint(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidCheckpoint {
            key: key.into(),
            message: message.into(),
        }
    }
}
