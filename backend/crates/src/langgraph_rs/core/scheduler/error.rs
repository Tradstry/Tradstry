use thiserror::Error;

use crate::langgraph_rs::core::channels::ChannelError;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("channel operation failed: {0}")]
    Channel(#[from] ChannelError),
    #[error("failed to serialize send packet for task '{task_id}': {message}")]
    SendSerialization { task_id: String, message: String },
    #[error("invalid tasks channel payload for channel '{channel}': {message}")]
    InvalidTasksChannelPayload { channel: String, message: String },
}
