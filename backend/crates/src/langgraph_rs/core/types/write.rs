use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type ChannelName = String;
pub type NodeName = String;
pub type TaskId = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelWrite {
    pub channel: ChannelName,
    pub value: Value,
}

impl ChannelWrite {
    pub fn new(channel: impl Into<ChannelName>, value: Value) -> Self {
        Self {
            channel: channel.into(),
            value,
        }
    }
}
