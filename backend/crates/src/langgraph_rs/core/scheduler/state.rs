use std::collections::{BTreeMap, BTreeSet};

use serde_json::to_value;

use crate::langgraph_rs::core::{
    constants::{PULL, PUSH, TASKS},
    types::{
        ChannelName, ChannelWrite, NodeExecutionResult, NodeName, SendPacket, TaskDescriptor,
        TaskPathStr,
    },
};
use crate::langgraph_rs::runtime::runner::RetryPolicy;

use super::SchedulerError;

pub const DEFAULT_TASKS_CHANNEL: &str = TASKS;
pub const PUSH_WRITE_CHANNEL: &str = PUSH;
pub const PULL_TASK_PREFIX: &str = PULL;
pub const PUSH_TASK_PREFIX: &str = PUSH;

pub type ChannelVersions = BTreeMap<ChannelName, u64>;
pub type VersionsSeen = BTreeMap<NodeName, ChannelVersions>;
pub type TriggerToNodes = BTreeMap<ChannelName, BTreeSet<NodeName>>;

#[derive(Debug, Clone, Default)]
pub struct SchedulerCheckpoint {
    pub channel_versions: ChannelVersions,
    pub versions_seen: VersionsSeen,
    pub updated_channels: BTreeSet<ChannelName>,
}

impl SchedulerCheckpoint {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_version(&self) -> u64 {
        self.channel_versions
            .values()
            .copied()
            .max()
            .map_or(1, |current| current.saturating_add(1))
    }
}

#[derive(Debug, Clone)]
pub struct NodeScheduleSpec {
    pub name: NodeName,
    pub triggers: Vec<ChannelName>,
    pub read_channels: Option<Vec<ChannelName>>,
    #[deprecated(note = "use read_channels/with_read_channels instead")]
    pub input_channels: Option<Vec<ChannelName>>,
    pub retry_policy: Option<Vec<RetryPolicy>>,
    pub cache_enabled: Option<bool>,
}

#[allow(deprecated)]
impl NodeScheduleSpec {
    pub fn new(name: impl Into<NodeName>, triggers: impl IntoIterator<Item = ChannelName>) -> Self {
        Self {
            name: name.into(),
            triggers: triggers.into_iter().collect(),
            read_channels: None,
            input_channels: None,
            retry_policy: None,
            cache_enabled: None,
        }
    }

    pub fn with_read_channels(
        mut self,
        read_channels: impl IntoIterator<Item = ChannelName>,
    ) -> Self {
        self.read_channels = Some(read_channels.into_iter().collect());
        self
    }

    pub fn with_input_channels(
        mut self,
        input_channels: impl IntoIterator<Item = ChannelName>,
    ) -> Self {
        let channels = input_channels.into_iter().collect::<Vec<_>>();
        self.read_channels = Some(channels.clone());
        self.input_channels = Some(channels);
        self
    }

    pub fn effective_input_channels(&self) -> Vec<ChannelName> {
        self.read_channels
            .clone()
            .or_else(|| self.input_channels.clone())
            .unwrap_or_else(|| self.triggers.clone())
    }

    pub fn with_retry_policy(mut self, retry_policy: Vec<RetryPolicy>) -> Self {
        self.retry_policy = Some(retry_policy);
        self
    }

    pub fn with_cache_enabled(mut self, cache_enabled: bool) -> Self {
        self.cache_enabled = Some(cache_enabled);
        self
    }
}

#[derive(Debug, Clone)]
pub enum PlannedTaskKind {
    Pull,
    PushSend { send_idx: usize, packet: SendPacket },
}

#[derive(Debug, Clone)]
pub struct PlannedTask {
    pub descriptor: TaskDescriptor,
    pub triggers: Vec<ChannelName>,
    pub kind: PlannedTaskKind,
}

impl PlannedTask {
    pub fn new_pull(
        descriptor: TaskDescriptor,
        triggers: impl IntoIterator<Item = ChannelName>,
    ) -> Self {
        Self {
            descriptor,
            triggers: triggers.into_iter().collect(),
            kind: PlannedTaskKind::Pull,
        }
    }

    pub fn new_push_send(descriptor: TaskDescriptor, send_idx: usize, packet: SendPacket) -> Self {
        Self {
            descriptor,
            triggers: vec![PUSH_WRITE_CHANNEL.to_owned()],
            kind: PlannedTaskKind::PushSend { send_idx, packet },
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskWrites {
    pub task: TaskDescriptor,
    pub triggers: Vec<ChannelName>,
    pub writes: Vec<ChannelWrite>,
}

impl TaskWrites {
    pub fn new(
        task: TaskDescriptor,
        triggers: impl IntoIterator<Item = ChannelName>,
        writes: impl IntoIterator<Item = ChannelWrite>,
    ) -> Self {
        Self {
            task,
            triggers: triggers.into_iter().collect(),
            writes: writes.into_iter().collect(),
        }
    }

    pub fn from_execution_result(
        task: TaskDescriptor,
        triggers: impl IntoIterator<Item = ChannelName>,
        mut result: NodeExecutionResult,
        tasks_channel: Option<&str>,
    ) -> Result<Self, SchedulerError> {
        let tasks_channel = tasks_channel.unwrap_or(DEFAULT_TASKS_CHANNEL);

        for send in result.sends.drain(..) {
            let send_value =
                serialize_send(&send).map_err(|message| SchedulerError::SendSerialization {
                    task_id: task.id.clone(),
                    message,
                })?;
            result
                .writes
                .push(ChannelWrite::new(tasks_channel, send_value));
        }

        if let Some(return_value) = result.return_value {
            result
                .writes
                .push(ChannelWrite::new("__return__", return_value));
        }

        Ok(Self {
            task,
            triggers: triggers.into_iter().collect(),
            writes: result.writes,
        })
    }

    pub fn deterministic_sort_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.task.path.to_path_string(),
            self.task.name,
            self.task.id
        )
    }
}

fn serialize_send(send: &SendPacket) -> Result<serde_json::Value, String> {
    to_value(send).map_err(|err| err.to_string())
}
