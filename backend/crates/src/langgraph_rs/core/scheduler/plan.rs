use std::collections::{BTreeMap, BTreeSet};

use crate::langgraph_rs::core::{
    channels::Channel,
    types::{ChannelName, NodeName, SendPacket, TaskDescriptor, TaskPathPart},
};

use super::{
    ChannelVersions, DEFAULT_TASKS_CHANNEL, NodeScheduleSpec, PULL_TASK_PREFIX, PUSH_TASK_PREFIX,
    PlannedTask, SchedulerCheckpoint, SchedulerError, TriggerToNodes, VersionsSeen,
};

pub fn build_trigger_to_nodes(specs: &[NodeScheduleSpec]) -> TriggerToNodes {
    let mut mapping: TriggerToNodes = BTreeMap::new();

    for spec in specs {
        for trigger in &spec.triggers {
            mapping
                .entry(trigger.clone())
                .or_default()
                .insert(spec.name.clone());
        }
    }

    mapping
}

pub fn is_node_triggered(
    node: &NodeScheduleSpec,
    channels: &BTreeMap<ChannelName, Box<dyn Channel>>,
    channel_versions: &ChannelVersions,
    versions_seen: Option<&VersionsSeen>,
) -> bool {
    match versions_seen.and_then(|seen| seen.get(&node.name)) {
        None => node.triggers.iter().any(|trigger| {
            channels
                .get(trigger)
                .is_some_and(|channel| channel.is_available())
        }),
        Some(seen_versions) => node.triggers.iter().any(|trigger| {
            let Some(channel) = channels.get(trigger) else {
                return false;
            };
            if !channel.is_available() {
                return false;
            }

            let current = channel_versions.get(trigger).copied().unwrap_or_default();
            let seen = seen_versions.get(trigger).copied().unwrap_or_default();
            current > seen
        }),
    }
}

pub fn plan_next_tasks_detailed(
    checkpoint: &SchedulerCheckpoint,
    channels: &BTreeMap<ChannelName, Box<dyn Channel>>,
    specs: &[NodeScheduleSpec],
    trigger_to_nodes: Option<&TriggerToNodes>,
    updated_channels: Option<&BTreeSet<ChannelName>>,
    step: u64,
    tasks_channel: Option<&str>,
) -> Result<Vec<PlannedTask>, SchedulerError> {
    let tasks_channel = tasks_channel.unwrap_or(DEFAULT_TASKS_CHANNEL);
    let spec_by_name: BTreeMap<NodeName, &NodeScheduleSpec> =
        specs.iter().map(|spec| (spec.name.clone(), spec)).collect();

    let mut planned = plan_push_send_tasks(channels, &spec_by_name, tasks_channel, step)?;

    let candidate_nodes: BTreeSet<NodeName> =
        if let (Some(mapping), Some(updated)) = (trigger_to_nodes, updated_channels) {
            let mut nodes = BTreeSet::new();
            for channel in updated {
                if let Some(triggered) = mapping.get(channel) {
                    nodes.extend(triggered.iter().cloned());
                }
            }
            nodes
        } else if checkpoint.channel_versions.is_empty() {
            BTreeSet::new()
        } else {
            spec_by_name.keys().cloned().collect()
        };

    for name in candidate_nodes {
        let Some(spec) = spec_by_name.get(&name) else {
            continue;
        };

        if !is_node_triggered(
            spec,
            channels,
            &checkpoint.channel_versions,
            Some(&checkpoint.versions_seen),
        ) {
            continue;
        }

        let triggers = sorted_triggers(spec);
        let task_id = format!(
            "{PULL_TASK_PREFIX}:{step}:{}:{}",
            spec.name,
            triggers.join("|")
        );
        let path = vec![
            TaskPathPart::Name("pull".to_owned()),
            TaskPathPart::Name(spec.name.clone()),
            TaskPathPart::Index(step),
        ];
        let descriptor = TaskDescriptor::new(task_id, spec.name.clone(), path);

        planned.push(PlannedTask::new_pull(descriptor, triggers));
    }

    Ok(planned)
}

pub fn plan_next_tasks(
    checkpoint: &SchedulerCheckpoint,
    channels: &BTreeMap<ChannelName, Box<dyn Channel>>,
    specs: &[NodeScheduleSpec],
    trigger_to_nodes: Option<&TriggerToNodes>,
    updated_channels: Option<&BTreeSet<ChannelName>>,
    step: u64,
) -> Vec<TaskDescriptor> {
    plan_next_tasks_detailed(
        checkpoint,
        channels,
        specs,
        trigger_to_nodes,
        updated_channels,
        step,
        None,
    )
    .map(|tasks| tasks.into_iter().map(|task| task.descriptor).collect())
    .unwrap_or_default()
}

fn sorted_triggers(spec: &NodeScheduleSpec) -> Vec<String> {
    let mut triggers = spec.triggers.clone();
    triggers.sort();
    triggers
}

fn plan_push_send_tasks(
    channels: &BTreeMap<ChannelName, Box<dyn Channel>>,
    spec_by_name: &BTreeMap<NodeName, &NodeScheduleSpec>,
    tasks_channel: &str,
    step: u64,
) -> Result<Vec<PlannedTask>, SchedulerError> {
    let Some(channel) = channels.get(tasks_channel) else {
        return Ok(Vec::new());
    };
    if !channel.is_available() {
        return Ok(Vec::new());
    }

    let tasks_value = channel.get()?;
    let entries =
        tasks_value
            .as_array()
            .ok_or_else(|| SchedulerError::InvalidTasksChannelPayload {
                channel: tasks_channel.to_owned(),
                message: format!("expected array, got {}", value_kind(&tasks_value)),
            })?;

    let mut planned = Vec::new();
    for (send_idx, entry) in entries.iter().enumerate() {
        let Ok(packet) = serde_json::from_value::<SendPacket>(entry.clone()) else {
            continue;
        };
        if !spec_by_name.contains_key(&packet.node) {
            continue;
        }

        let task_id = format!("{PUSH_TASK_PREFIX}:{step}:{}:{send_idx}", packet.node);
        let path = vec![
            TaskPathPart::Name("push".to_owned()),
            TaskPathPart::Index(send_idx as u64),
            TaskPathPart::Name(packet.node.clone()),
            TaskPathPart::Index(step),
        ];
        let descriptor = TaskDescriptor::new(task_id, packet.node.clone(), path);
        planned.push(PlannedTask::new_push_send(descriptor, send_idx, packet));
    }

    Ok(planned)
}

fn value_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use serde_json::json;

    use crate::langgraph_rs::core::{
        channels::{Channel, LastValue, Topic},
        scheduler::{
            DEFAULT_TASKS_CHANNEL, PULL_TASK_PREFIX, PlannedTaskKind, build_trigger_to_nodes,
            plan_next_tasks, plan_next_tasks_detailed,
        },
        types::{ChannelName, SendPacket},
    };

    use super::{NodeScheduleSpec, SchedulerCheckpoint};

    #[test]
    fn build_trigger_map_is_deterministic() {
        let specs = vec![
            NodeScheduleSpec::new("n1", vec!["a".to_owned(), "b".to_owned()]),
            NodeScheduleSpec::new("n2", vec!["b".to_owned()]),
        ];

        let trigger_to_nodes = build_trigger_to_nodes(&specs);

        let b_nodes = trigger_to_nodes.get("b").unwrap();
        let collected = b_nodes.iter().cloned().collect::<Vec<_>>();
        assert_eq!(collected, vec!["n1".to_owned(), "n2".to_owned()]);
    }

    #[test]
    fn plans_task_when_trigger_channel_version_advanced() {
        let mut checkpoint = SchedulerCheckpoint::new();
        checkpoint.channel_versions.insert("a".to_owned(), 2);
        checkpoint
            .versions_seen
            .insert("n1".to_owned(), BTreeMap::from([("a".to_owned(), 1)]));

        let mut channel = LastValue::new("a");
        channel.update(&[json!("x")]).unwrap();

        let mut channels: BTreeMap<ChannelName, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("a".to_owned(), Box::new(channel));

        let specs = vec![NodeScheduleSpec::new("n1", vec!["a".to_owned()])];
        let trigger_to_nodes = build_trigger_to_nodes(&specs);

        let tasks = plan_next_tasks(
            &checkpoint,
            &channels,
            &specs,
            Some(&trigger_to_nodes),
            Some(&BTreeSet::from(["a".to_owned()])),
            3,
        );

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "n1");
        assert!(
            tasks[0]
                .id
                .starts_with(&format!("{PULL_TASK_PREFIX}:3:n1:"))
        );
    }

    #[test]
    fn plans_push_tasks_from_tasks_channel_in_index_order() {
        let mut tasks_topic = Topic::new(DEFAULT_TASKS_CHANNEL, false);
        tasks_topic
            .update(&[
                serde_json::to_value(SendPacket::new("worker", json!({"i": 0}))).unwrap(),
                serde_json::to_value(SendPacket::new("worker", json!({"i": 1}))).unwrap(),
            ])
            .unwrap();

        let mut channels: BTreeMap<ChannelName, Box<dyn Channel>> = BTreeMap::new();
        channels.insert(DEFAULT_TASKS_CHANNEL.to_owned(), Box::new(tasks_topic));

        let specs = vec![NodeScheduleSpec::new("worker", Vec::<String>::new())];
        let planned = plan_next_tasks_detailed(
            &SchedulerCheckpoint::new(),
            &channels,
            &specs,
            None,
            None,
            2,
            Some(DEFAULT_TASKS_CHANNEL),
        )
        .unwrap();

        assert_eq!(planned.len(), 2);
        for (idx, task) in planned.into_iter().enumerate() {
            match task.kind {
                PlannedTaskKind::PushSend { send_idx, packet } => {
                    assert_eq!(send_idx, idx);
                    assert_eq!(packet.arg.get("i"), Some(&json!(idx)));
                }
                PlannedTaskKind::Pull => panic!("expected push task"),
            }
        }
    }

    #[test]
    fn mixes_push_and_pull_tasks_deterministically() {
        let mut checkpoint = SchedulerCheckpoint::new();
        checkpoint.channel_versions.insert("a".to_owned(), 2);
        checkpoint
            .versions_seen
            .insert("n1".to_owned(), BTreeMap::from([("a".to_owned(), 1)]));

        let mut trigger_channel = LastValue::new("a");
        trigger_channel.update(&[json!("x")]).unwrap();

        let mut tasks_topic = Topic::new(DEFAULT_TASKS_CHANNEL, false);
        tasks_topic
            .update(&[serde_json::to_value(SendPacket::new("n2", json!({"ok": true}))).unwrap()])
            .unwrap();

        let mut channels: BTreeMap<ChannelName, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("a".to_owned(), Box::new(trigger_channel));
        channels.insert(DEFAULT_TASKS_CHANNEL.to_owned(), Box::new(tasks_topic));

        let specs = vec![
            NodeScheduleSpec::new("n1", vec!["a".to_owned()]),
            NodeScheduleSpec::new("n2", Vec::<String>::new()),
        ];
        let trigger_to_nodes = build_trigger_to_nodes(&specs);

        let planned = plan_next_tasks_detailed(
            &checkpoint,
            &channels,
            &specs,
            Some(&trigger_to_nodes),
            Some(&BTreeSet::from(["a".to_owned()])),
            4,
            Some(DEFAULT_TASKS_CHANNEL),
        )
        .unwrap();

        assert_eq!(planned.len(), 2);
        assert!(matches!(planned[0].kind, PlannedTaskKind::PushSend { .. }));
        assert!(matches!(planned[1].kind, PlannedTaskKind::Pull));
    }

    #[test]
    fn ignores_invalid_packets_and_unknown_send_targets() {
        let mut tasks_topic = Topic::new(DEFAULT_TASKS_CHANNEL, false);
        tasks_topic
            .update(&[
                json!({"node":"unknown","arg":{"x":1}}),
                json!({"bad":"packet"}),
                serde_json::to_value(SendPacket::new("worker", json!({"x": 2}))).unwrap(),
            ])
            .unwrap();

        let mut channels: BTreeMap<ChannelName, Box<dyn Channel>> = BTreeMap::new();
        channels.insert(DEFAULT_TASKS_CHANNEL.to_owned(), Box::new(tasks_topic));

        let specs = vec![NodeScheduleSpec::new("worker", Vec::<String>::new())];
        let planned = plan_next_tasks_detailed(
            &SchedulerCheckpoint::new(),
            &channels,
            &specs,
            None,
            None,
            1,
            Some(DEFAULT_TASKS_CHANNEL),
        )
        .unwrap();

        assert_eq!(planned.len(), 1);
        assert!(matches!(planned[0].kind, PlannedTaskKind::PushSend { .. }));
    }

    #[test]
    fn errors_when_tasks_channel_payload_is_not_array() {
        let mut invalid_tasks = LastValue::new(DEFAULT_TASKS_CHANNEL);
        invalid_tasks.update(&[json!({"not":"array"})]).unwrap();

        let mut channels: BTreeMap<ChannelName, Box<dyn Channel>> = BTreeMap::new();
        channels.insert(DEFAULT_TASKS_CHANNEL.to_owned(), Box::new(invalid_tasks));

        let specs = vec![NodeScheduleSpec::new("worker", Vec::<String>::new())];
        let err = plan_next_tasks_detailed(
            &SchedulerCheckpoint::new(),
            &channels,
            &specs,
            None,
            None,
            1,
            Some(DEFAULT_TASKS_CHANNEL),
        )
        .unwrap_err();

        assert!(format!("{err}").contains("invalid tasks channel payload"));
    }
}
