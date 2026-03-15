use serde::{Deserialize, Serialize};
use serde_json::to_value;

use crate::langgraph_rs::core::types::{StreamEvent, StreamMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RuntimeEvent {
    InputApplied {
        step: u64,
        writes: usize,
        updated_channels: Vec<String>,
    },
    ResumeApplied {
        step: u64,
        resumes: usize,
    },
    StepStarted {
        step: u64,
        planned_tasks: Vec<String>,
    },
    InterruptBefore {
        step: u64,
        task_ids: Vec<String>,
        nodes: Vec<String>,
    },
    InterruptAfter {
        step: u64,
        task_ids: Vec<String>,
        nodes: Vec<String>,
    },
    TaskStarted {
        step: u64,
        task_id: String,
        node: String,
        attempt: u32,
    },
    TaskCacheHit {
        step: u64,
        task_id: String,
        node: String,
    },
    TaskCacheMiss {
        step: u64,
        task_id: String,
        node: String,
    },
    TaskCacheStored {
        step: u64,
        task_id: String,
        node: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ttl_millis: Option<u64>,
    },
    TaskRetrying {
        step: u64,
        task_id: String,
        node: String,
        attempt: u32,
        message: String,
    },
    TaskSucceeded {
        step: u64,
        task_id: String,
        node: String,
        attempts: u32,
        writes: usize,
        sends: usize,
        has_return_value: bool,
    },
    TaskFailed {
        step: u64,
        task_id: String,
        node: String,
        attempts: u32,
        kind: String,
        message: String,
    },
    WritesApplied {
        step: u64,
        updated_channels: Vec<String>,
    },
    CheckpointSaved {
        step: u64,
        checkpoint_id: String,
        source: String,
    },
    LoopFinished {
        status: String,
        steps_executed: u64,
        tasks_executed: usize,
    },
}

impl RuntimeEvent {
    pub fn mode(&self) -> StreamMode {
        match self {
            Self::InputApplied { .. } => StreamMode::Updates,
            Self::ResumeApplied { .. } => StreamMode::Updates,
            Self::StepStarted { .. } => StreamMode::Debug,
            Self::InterruptBefore { .. } => StreamMode::Debug,
            Self::InterruptAfter { .. } => StreamMode::Debug,
            Self::TaskStarted { .. } => StreamMode::Tasks,
            Self::TaskCacheHit { .. } => StreamMode::Tasks,
            Self::TaskCacheMiss { .. } => StreamMode::Tasks,
            Self::TaskCacheStored { .. } => StreamMode::Tasks,
            Self::TaskRetrying { .. } => StreamMode::Debug,
            Self::TaskSucceeded { .. } => StreamMode::Tasks,
            Self::TaskFailed { .. } => StreamMode::Tasks,
            Self::WritesApplied { .. } => StreamMode::Updates,
            Self::CheckpointSaved { .. } => StreamMode::Checkpoints,
            Self::LoopFinished { .. } => StreamMode::Debug,
        }
    }

    pub fn into_stream_event(self) -> StreamEvent {
        let mode = self.mode();
        let payload =
            to_value(self).unwrap_or_else(|_| serde_json::json!({"event":"serialization_error"}));
        StreamEvent::new(mode, payload)
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeEvent;
    use crate::langgraph_rs::core::types::StreamMode;

    #[test]
    fn maps_runtime_event_to_stream_event() {
        let event = RuntimeEvent::TaskStarted {
            step: 3,
            task_id: "t1".to_owned(),
            node: "n1".to_owned(),
            attempt: 1,
        }
        .into_stream_event();

        assert_eq!(event.mode, StreamMode::Tasks);
        assert_eq!(
            event.payload.get("event").and_then(|value| value.as_str()),
            Some("task_started")
        );
    }

    #[test]
    fn maps_interrupt_events_to_debug_mode() {
        let before = RuntimeEvent::InterruptBefore {
            step: 1,
            task_ids: vec!["t1".to_owned()],
            nodes: vec!["n1".to_owned()],
        }
        .into_stream_event();

        let after = RuntimeEvent::InterruptAfter {
            step: 1,
            task_ids: vec!["t1".to_owned()],
            nodes: vec!["n1".to_owned()],
        }
        .into_stream_event();

        assert_eq!(before.mode, StreamMode::Debug);
        assert_eq!(after.mode, StreamMode::Debug);
    }

    #[test]
    fn maps_task_cache_events_to_tasks_mode() {
        let hit = RuntimeEvent::TaskCacheHit {
            step: 1,
            task_id: "t1".to_owned(),
            node: "n1".to_owned(),
        }
        .into_stream_event();
        let miss = RuntimeEvent::TaskCacheMiss {
            step: 1,
            task_id: "t1".to_owned(),
            node: "n1".to_owned(),
        }
        .into_stream_event();
        let stored = RuntimeEvent::TaskCacheStored {
            step: 1,
            task_id: "t1".to_owned(),
            node: "n1".to_owned(),
            ttl_millis: Some(1000),
        }
        .into_stream_event();

        assert_eq!(hit.mode, StreamMode::Tasks);
        assert_eq!(miss.mode, StreamMode::Tasks);
        assert_eq!(stored.mode, StreamMode::Tasks);
    }
}
