use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::langgraph_rs::core::channels::Channel;
use crate::langgraph_rs::core::types::ChannelName;

use super::{PUSH_WRITE_CHANNEL, SchedulerCheckpoint, SchedulerError, TaskWrites, TriggerToNodes};

pub const RESERVED_WRITE_CHANNELS: &[&str] = &[
    "__no_writes__",
    PUSH_WRITE_CHANNEL,
    "__resume__",
    "__interrupt__",
    "__return__",
    "__error__",
];

pub fn is_reserved_write_channel(channel: &str) -> bool {
    RESERVED_WRITE_CHANNELS.contains(&channel)
}

#[derive(Debug, Clone, Default)]
pub struct SchedulerApplySummary {
    pub updated_channels: BTreeSet<ChannelName>,
}

pub fn apply_writes(
    checkpoint: &mut SchedulerCheckpoint,
    channels: &mut BTreeMap<ChannelName, Box<dyn Channel>>,
    tasks: &[TaskWrites],
    trigger_to_nodes: &TriggerToNodes,
) -> Result<SchedulerApplySummary, SchedulerError> {
    let mut sorted_tasks = tasks.to_vec();
    sorted_tasks.sort_by_key(TaskWrites::deterministic_sort_key);

    let bump_step = sorted_tasks.iter().any(|task| !task.triggers.is_empty());

    for task in &sorted_tasks {
        let seen = checkpoint
            .versions_seen
            .entry(task.task.name.clone())
            .or_default();

        for trigger in &task.triggers {
            if let Some(version) = checkpoint.channel_versions.get(trigger) {
                seen.insert(trigger.clone(), *version);
            }
        }
    }

    let next_version = checkpoint.next_version();

    let consumed_channels: BTreeSet<ChannelName> = sorted_tasks
        .iter()
        .flat_map(|task| task.triggers.iter().cloned())
        .collect();

    for channel_name in consumed_channels {
        if let Some(channel) = channels.get_mut(&channel_name) {
            if channel.consume()? {
                checkpoint
                    .channel_versions
                    .insert(channel_name.clone(), next_version);
            }
        }
    }

    let mut grouped_writes: BTreeMap<ChannelName, Vec<Value>> = BTreeMap::new();
    for task in &sorted_tasks {
        for write in &task.writes {
            if is_reserved_write_channel(&write.channel) {
                continue;
            }
            if channels.contains_key(&write.channel) {
                grouped_writes
                    .entry(write.channel.clone())
                    .or_default()
                    .push(write.value.clone());
            }
        }
    }

    let mut updated_channels: BTreeSet<ChannelName> = BTreeSet::new();

    for (channel_name, values) in grouped_writes {
        if let Some(channel) = channels.get_mut(&channel_name) {
            if channel.update(&values)? {
                checkpoint
                    .channel_versions
                    .insert(channel_name.clone(), next_version);
                if channel.is_available() {
                    updated_channels.insert(channel_name);
                }
            }
        }
    }

    if bump_step {
        for (channel_name, channel) in channels.iter_mut() {
            if updated_channels.contains(channel_name) {
                continue;
            }
            if !channel.is_available() {
                continue;
            }
            if channel.update(&[])? {
                checkpoint
                    .channel_versions
                    .insert(channel_name.clone(), next_version);
                if channel.is_available() {
                    updated_channels.insert(channel_name.clone());
                }
            }
        }
    }

    let triggers_intersect = updated_channels
        .iter()
        .any(|channel_name| trigger_to_nodes.contains_key(channel_name));

    if bump_step && !triggers_intersect {
        for (channel_name, channel) in channels.iter_mut() {
            if channel.finish()? {
                checkpoint
                    .channel_versions
                    .insert(channel_name.clone(), next_version);
                if channel.is_available() {
                    updated_channels.insert(channel_name.clone());
                }
            }
        }
    }

    checkpoint.updated_channels = updated_channels.clone();

    Ok(SchedulerApplySummary { updated_channels })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::langgraph_rs::core::{
        channels::{Channel, EphemeralValue, LastValue},
        types::{ChannelWrite, TaskDescriptor},
    };

    use super::{
        SchedulerCheckpoint, TaskWrites, TriggerToNodes, apply_writes, is_reserved_write_channel,
    };

    #[test]
    fn ignores_reserved_write_channels() {
        assert!(is_reserved_write_channel("__return__"));
        assert!(is_reserved_write_channel("__pregel_push"));
        assert!(!is_reserved_write_channel("messages"));
    }

    #[test]
    fn applies_channel_writes_and_bumps_versions() {
        let mut checkpoint = SchedulerCheckpoint::new();

        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("a".to_owned(), Box::new(LastValue::new("a")));
        channels.insert("b".to_owned(), Box::new(EphemeralValue::new("b", true)));

        let task = TaskDescriptor::new("t1", "node1", vec![]);
        let writes = vec![
            ChannelWrite::new("a", json!("x")),
            ChannelWrite::new("b", json!("y")),
        ];

        let task_writes = TaskWrites::new(task, vec![], writes);

        let summary = apply_writes(
            &mut checkpoint,
            &mut channels,
            &[task_writes],
            &TriggerToNodes::new(),
        )
        .unwrap();

        assert!(summary.updated_channels.contains("a"));
        assert!(summary.updated_channels.contains("b"));
        assert_eq!(checkpoint.channel_versions.get("a").copied(), Some(1));
        assert_eq!(checkpoint.channel_versions.get("b").copied(), Some(1));
    }
}
