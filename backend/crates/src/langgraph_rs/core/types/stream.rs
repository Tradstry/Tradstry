use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamMode {
    Values,
    Updates,
    Checkpoints,
    Tasks,
    Debug,
    Messages,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamEvent {
    pub mode: StreamMode,
    pub payload: Value,
}

impl StreamEvent {
    pub fn new(mode: StreamMode, payload: Value) -> Self {
        Self { mode, payload }
    }
}
