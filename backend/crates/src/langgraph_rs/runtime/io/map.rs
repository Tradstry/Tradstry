use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use thiserror::Error;

use crate::langgraph_rs::{
    checkpoint::base::{CheckpointConfig, CheckpointMetadata},
    core::{
        channels::Channel,
        constants::{ERROR, INTERRUPT, NO_WRITES, NULL_TASK_ID, RESUME, RETURN, START, TASKS},
        types::{
            ChannelName, ChannelWrite, Command, CommandGraph, GotoTarget, TaskDescriptor, TaskId,
        },
    },
};

pub type PendingWriteTuple = (TaskId, ChannelName, Value);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputChannels {
    Single(ChannelName),
    Many(Vec<ChannelName>),
}

impl OutputChannels {
    pub fn contains(&self, channel: &str) -> bool {
        match self {
            Self::Single(single) => single == channel,
            Self::Many(channels) => channels.iter().any(|candidate| candidate == channel),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Single(single) => single.is_empty(),
            Self::Many(channels) => channels.is_empty(),
        }
    }
}

impl From<&str> for OutputChannels {
    fn from(value: &str) -> Self {
        Self::Single(value.to_owned())
    }
}

impl From<String> for OutputChannels {
    fn from(value: String) -> Self {
        Self::Single(value)
    }
}

impl From<Vec<String>> for OutputChannels {
    fn from(value: Vec<String>) -> Self {
        Self::Many(value)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OutputWriteGate<'a> {
    All,
    Writes(&'a [ChannelWrite]),
}

impl<'a> OutputWriteGate<'a> {
    fn touches_output_channels(&self, output_channels: &OutputChannels) -> bool {
        match self {
            Self::All => true,
            Self::Writes(writes) => writes
                .iter()
                .any(|write| output_channels.contains(&write.channel)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TaskOutputWrites<'a> {
    pub task: &'a TaskDescriptor,
    pub writes: &'a [ChannelWrite],
}

impl<'a> TaskOutputWrites<'a> {
    pub fn new(task: &'a TaskDescriptor, writes: &'a [ChannelWrite]) -> Self {
        Self { task, writes }
    }
}

#[derive(Debug, Error)]
pub enum IoMapError {
    #[error("there is no parent graph")]
    InvalidCommandGraph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugPayloadType {
    Task,
    TaskResult,
    Checkpoint,
}

impl DebugPayloadType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::TaskResult => "task_result",
            Self::Checkpoint => "checkpoint",
        }
    }
}

pub fn map_command(cmd: &Command) -> Result<Vec<PendingWriteTuple>, IoMapError> {
    if cmd.graph == CommandGraph::Parent {
        return Err(IoMapError::InvalidCommandGraph);
    }

    let mut mapped = Vec::<PendingWriteTuple>::new();

    for goto in &cmd.goto {
        match goto {
            GotoTarget::Send(packet) => {
                let packet_value = serde_json::to_value(packet).unwrap_or(Value::Null);
                mapped.push((NULL_TASK_ID.to_owned(), TASKS.to_owned(), packet_value));
            }
            GotoTarget::Node(node) => {
                mapped.push((
                    NULL_TASK_ID.to_owned(),
                    format!("branch:to:{node}"),
                    Value::String(START.to_owned()),
                ));
            }
        }
    }

    if let Some(resume) = &cmd.resume {
        mapped.push((NULL_TASK_ID.to_owned(), RESUME.to_owned(), resume.clone()));
    }

    if let Some(update) = &cmd.update {
        for write in update.clone().into_writes() {
            mapped.push((NULL_TASK_ID.to_owned(), write.channel, write.value));
        }
    }

    Ok(mapped)
}

pub fn map_command_to_writes(cmd: &Command) -> Result<Vec<ChannelWrite>, IoMapError> {
    if cmd.graph == CommandGraph::Parent {
        return Err(IoMapError::InvalidCommandGraph);
    }

    let mut writes = Vec::<ChannelWrite>::new();

    for goto in &cmd.goto {
        match goto {
            GotoTarget::Send(packet) => {
                let packet_value = serde_json::to_value(packet).unwrap_or(Value::Null);
                writes.push(ChannelWrite::new(TASKS, packet_value));
            }
            GotoTarget::Node(node) => {
                writes.push(ChannelWrite::new(
                    format!("branch:to:{node}"),
                    Value::String(START.to_owned()),
                ));
            }
        }
    }

    if let Some(resume) = &cmd.resume {
        writes.push(ChannelWrite::new(RESUME, resume.clone()));
    }

    if let Some(update) = &cmd.update {
        writes.extend(update.clone().into_writes());
    }

    Ok(writes)
}

pub fn map_input_writes(
    writes: Vec<ChannelWrite>,
    channels: &BTreeMap<ChannelName, Box<dyn Channel>>,
) -> Vec<ChannelWrite> {
    writes
        .into_iter()
        .filter(|write| {
            channels.contains_key(&write.channel)
                || write.channel == RESUME
                || write.channel == TASKS
                || write.channel.starts_with("branch:to:")
        })
        .collect()
}

pub fn map_output_values(
    output_channels: &OutputChannels,
    pending_writes: OutputWriteGate<'_>,
    channels: &BTreeMap<ChannelName, Box<dyn Channel>>,
) -> Option<Value> {
    if output_channels.is_empty() || !pending_writes.touches_output_channels(output_channels) {
        return None;
    }

    match output_channels {
        OutputChannels::Single(channel_name) => Some(
            channels
                .get(channel_name)
                .and_then(|channel| channel.get().ok())
                .unwrap_or(Value::Null),
        ),
        OutputChannels::Many(channel_names) => {
            let mut values = serde_json::Map::new();
            for channel_name in channel_names {
                if let Some(value) = channels
                    .get(channel_name)
                    .and_then(|channel| channel.get().ok())
                {
                    values.insert(channel_name.clone(), value);
                }
            }
            Some(Value::Object(values))
        }
    }
}

pub fn map_output_updates(
    output_channels: &OutputChannels,
    tasks: &[TaskOutputWrites<'_>],
    cached: bool,
) -> Option<Value> {
    let output_tasks = tasks
        .iter()
        .copied()
        .filter(|task| {
            task.writes
                .first()
                .is_some_and(|write| write.channel != ERROR && write.channel != INTERRUPT)
        })
        .collect::<Vec<_>>();

    if output_tasks.is_empty() {
        return None;
    }

    let mut order = Vec::<String>::new();
    let mut grouped = BTreeMap::<String, Vec<Value>>::new();

    for task in &output_tasks {
        if !grouped.contains_key(&task.task.name) {
            order.push(task.task.name.clone());
            grouped.insert(task.task.name.clone(), Vec::new());
        }
    }

    for task in output_tasks {
        let node_name = task.task.name.clone();
        let Some(values) = grouped.get_mut(&node_name) else {
            continue;
        };

        if let Some(return_value) = task
            .writes
            .iter()
            .find(|write| write.channel == RETURN)
            .map(|write| write.value.clone())
        {
            values.push(return_value);
            continue;
        }

        match output_channels {
            OutputChannels::Single(channel_name) => {
                values.extend(
                    task.writes
                        .iter()
                        .filter(|write| write.channel == *channel_name)
                        .map(|write| write.value.clone()),
                );
            }
            OutputChannels::Many(channel_names) => {
                let selected = task
                    .writes
                    .iter()
                    .filter(|write| {
                        channel_names
                            .iter()
                            .any(|channel| channel == &write.channel)
                    })
                    .collect::<Vec<_>>();

                if selected.is_empty() {
                    continue;
                }

                let mut counts = BTreeMap::<String, usize>::new();
                for write in &selected {
                    *counts.entry(write.channel.clone()).or_default() += 1;
                }

                if counts.values().any(|count| *count > 1) {
                    for write in selected {
                        let mut single = serde_json::Map::new();
                        single.insert(write.channel.clone(), write.value.clone());
                        values.push(Value::Object(single));
                    }
                } else {
                    let mut packed = serde_json::Map::new();
                    for write in selected {
                        packed.insert(write.channel.clone(), write.value.clone());
                    }
                    values.push(Value::Object(packed));
                }
            }
        }
    }

    let mut payload = serde_json::Map::new();
    for node_name in order {
        let values = grouped.remove(&node_name).unwrap_or_default();
        let value = match values.len() {
            0 => Value::Null,
            1 => values.into_iter().next().unwrap_or(Value::Null),
            _ => Value::Array(values),
        };
        payload.insert(node_name, value);
    }

    if cached {
        payload.insert(
            "__metadata__".to_owned(),
            serde_json::json!({"cached": true}),
        );
    }

    Some(Value::Object(payload))
}

pub fn map_task_payload(task: &TaskDescriptor, input: &Value, triggers: &[ChannelName]) -> Value {
    serde_json::json!({
        "id": task.id,
        "name": task.name,
        "input": input,
        "triggers": triggers,
    })
}

pub fn map_task_result_payload(
    output_channels: &OutputChannels,
    task: &TaskDescriptor,
    writes: &[ChannelWrite],
) -> Value {
    let error = writes
        .iter()
        .find(|write| write.channel == ERROR)
        .map(|write| write.value.clone())
        .unwrap_or(Value::Null);
    let interrupts = writes
        .iter()
        .filter(|write| write.channel == INTERRUPT)
        .flat_map(|write| match &write.value {
            Value::Array(values) => values.clone(),
            value => vec![value.clone()],
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "id": task.id,
        "name": task.name,
        "error": error,
        "interrupts": interrupts,
        "result": map_task_result_value(output_channels, writes),
    })
}

fn map_task_result_value(output_channels: &OutputChannels, writes: &[ChannelWrite]) -> Value {
    if let Some(return_value) = writes
        .iter()
        .find(|write| write.channel == RETURN)
        .map(|write| write.value.clone())
    {
        return return_value;
    }

    match output_channels {
        OutputChannels::Single(channel_name) => {
            if is_internal_result_channel(channel_name) {
                return Value::Null;
            }
            let values = writes
                .iter()
                .filter(|write| write.channel == *channel_name)
                .map(|write| write.value.clone())
                .collect::<Vec<_>>();
            aggregate_single_channel_values(values)
        }
        OutputChannels::Many(channel_names) => {
            let mut object = serde_json::Map::new();
            for channel_name in channel_names {
                if is_internal_result_channel(channel_name) {
                    continue;
                }
                let values = writes
                    .iter()
                    .filter(|write| write.channel == *channel_name)
                    .map(|write| write.value.clone())
                    .collect::<Vec<_>>();
                if values.is_empty() {
                    continue;
                }
                object.insert(channel_name.clone(), aggregate_channel_values(values));
            }
            Value::Object(object)
        }
    }
}

fn aggregate_single_channel_values(values: Vec<Value>) -> Value {
    match values.len() {
        0 => Value::Null,
        1 => values.into_iter().next().unwrap_or(Value::Null),
        _ => serde_json::json!({ "$writes": values }),
    }
}

fn aggregate_channel_values(values: Vec<Value>) -> Value {
    if values.len() == 1 {
        values.into_iter().next().unwrap_or(Value::Null)
    } else {
        serde_json::json!({ "$writes": values })
    }
}

fn is_internal_result_channel(channel: &str) -> bool {
    matches!(channel, ERROR | INTERRUPT | RETURN | RESUME | NO_WRITES)
        || channel.starts_with("__pregel_")
        || channel.starts_with("branch:to:")
}

pub fn map_debug_wrapper(step: u64, payload_type: DebugPayloadType, payload: Value) -> Value {
    serde_json::json!({
        "step": step,
        "timestamp": utc_timestamp_iso8601(),
        "type": payload_type.as_str(),
        "payload": payload,
    })
}

pub fn map_checkpoint_payload(
    config: &CheckpointConfig,
    parent_config: Option<&CheckpointConfig>,
    metadata: &CheckpointMetadata,
    values: Value,
    next_tasks: &[TaskDescriptor],
    tasks: &[TaskOutputWrites<'_>],
    output_channels: &OutputChannels,
) -> Value {
    let tasks_payload = tasks
        .iter()
        .map(|task| map_task_result_payload(output_channels, task.task, task.writes))
        .collect::<Vec<_>>();
    let next = next_tasks
        .iter()
        .map(|task| Value::String(task.name.clone()))
        .collect::<Vec<_>>();

    serde_json::json!({
        "config": checkpoint_config_payload(config),
        "parent_config": parent_config.map(checkpoint_config_payload).unwrap_or(Value::Null),
        "metadata": serde_json::to_value(metadata).unwrap_or(Value::Object(serde_json::Map::new())),
        "values": values,
        "next": next,
        "tasks": tasks_payload,
    })
}

fn checkpoint_config_payload(config: &CheckpointConfig) -> Value {
    let mut configurable = serde_json::Map::new();
    configurable.insert(
        "thread_id".to_owned(),
        Value::String(config.thread_id.clone()),
    );
    configurable.insert(
        "checkpoint_ns".to_owned(),
        Value::String(config.checkpoint_ns.clone()),
    );
    if let Some(checkpoint_id) = &config.checkpoint_id {
        configurable.insert(
            "checkpoint_id".to_owned(),
            Value::String(checkpoint_id.clone()),
        );
    }

    let mut object = serde_json::Map::new();
    object.insert("configurable".to_owned(), Value::Object(configurable));
    Value::Object(object)
}

fn utc_timestamp_iso8601() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = now.as_secs() as i64;
    let nanos = now.subsec_nanos();

    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400) as u32;
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    let (year, month, day) = civil_from_days(days);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{nanos:09}Z")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };

    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::langgraph_rs::{
        checkpoint::base::{CheckpointConfig, CheckpointMetadata},
        core::{
            channels::{Channel, LastValue},
            constants::{NULL_TASK_ID, RESUME, RETURN, START, TASKS},
            types::{Command, CommandGraph, CommandUpdate, GotoTarget, SendPacket, TaskDescriptor},
        },
    };

    use super::{
        DebugPayloadType, IoMapError, OutputChannels, OutputWriteGate, TaskOutputWrites,
        map_checkpoint_payload, map_command, map_command_to_writes, map_debug_wrapper,
        map_input_writes, map_output_updates, map_output_values, map_task_payload,
        map_task_result_payload,
    };

    #[test]
    fn maps_command_goto_resume_and_update() {
        let command = Command::new()
            .with_goto(GotoTarget::Send(SendPacket::new("worker", json!({"x": 1}))))
            .with_goto(GotoTarget::Node("next".to_owned()))
            .with_resume(json!("resume"))
            .with_update(CommandUpdate::Object(serde_json::Map::from_iter([(
                "input".to_owned(),
                json!("hi"),
            )])));

        let mapped = map_command(&command).unwrap();

        assert!(mapped.iter().any(|(task_id, channel, value)| {
            task_id == NULL_TASK_ID
                && channel == TASKS
                && value.get("node") == Some(&json!("worker"))
        }));
        assert!(mapped.iter().any(|(task_id, channel, value)| {
            task_id == NULL_TASK_ID && channel == "branch:to:next" && value == START
        }));
        assert!(
            mapped
                .iter()
                .any(|(task_id, channel, value)| task_id == NULL_TASK_ID
                    && channel == RESUME
                    && value == "resume")
        );
        assert!(
            mapped
                .iter()
                .any(|(task_id, channel, value)| task_id == NULL_TASK_ID
                    && channel == "input"
                    && value == "hi")
        );
    }

    #[test]
    fn rejects_parent_graph_command() {
        let mut command = Command::new();
        command.graph = CommandGraph::Parent;
        let err = map_command(&command).unwrap_err();
        assert!(matches!(err, IoMapError::InvalidCommandGraph));
    }

    #[test]
    fn filters_input_writes_to_known_and_reserved_channels() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));

        let mapped = map_input_writes(
            vec![
                crate::langgraph_rs::core::types::ChannelWrite::new("input", json!("ok")),
                crate::langgraph_rs::core::types::ChannelWrite::new("unknown", json!(1)),
                crate::langgraph_rs::core::types::ChannelWrite::new(TASKS, json!({"node": "x"})),
            ],
            &channels,
        );

        assert_eq!(mapped.len(), 2);
        assert!(mapped.iter().any(|write| write.channel == "input"));
        assert!(mapped.iter().any(|write| write.channel == TASKS));
    }

    #[test]
    fn maps_current_graph_command_to_writes() {
        let command = Command::new()
            .with_goto(GotoTarget::Node("next".to_owned()))
            .with_resume(json!("r"))
            .with_update(CommandUpdate::Tuples(vec![
                crate::langgraph_rs::core::types::ChannelWrite::new("input", json!("v")),
            ]));

        let writes = map_command_to_writes(&command).unwrap();
        assert!(writes.iter().any(|w| w.channel == "branch:to:next"));
        assert!(writes.iter().any(|w| w.channel == RESUME));
        assert!(
            writes
                .iter()
                .any(|w| w.channel == "input" && w.value == "v")
        );
    }

    #[test]
    fn map_output_values_single_channel_respects_gate() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        let mut output = LastValue::new("output");
        output.update(&[json!("hello")]).unwrap();
        channels.insert("output".to_owned(), Box::new(output));

        let values = map_output_values(
            &OutputChannels::Single("output".to_owned()),
            OutputWriteGate::Writes(&[crate::langgraph_rs::core::types::ChannelWrite::new(
                "other",
                json!("x"),
            )]),
            &channels,
        );
        assert_eq!(values, None);

        let values = map_output_values(
            &OutputChannels::Single("output".to_owned()),
            OutputWriteGate::Writes(&[crate::langgraph_rs::core::types::ChannelWrite::new(
                "output",
                json!("x"),
            )]),
            &channels,
        );
        assert_eq!(values, Some(json!("hello")));
    }

    #[test]
    fn map_output_values_many_channels_reads_matching_state() {
        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        let mut output = LastValue::new("output");
        output.update(&[json!("hello")]).unwrap();
        let mut status = LastValue::new("status");
        status.update(&[json!("ok")]).unwrap();
        channels.insert("output".to_owned(), Box::new(output));
        channels.insert("status".to_owned(), Box::new(status));

        let values = map_output_values(
            &OutputChannels::Many(vec!["output".to_owned(), "status".to_owned()]),
            OutputWriteGate::Writes(&[crate::langgraph_rs::core::types::ChannelWrite::new(
                "status",
                json!("next"),
            )]),
            &channels,
        );
        assert_eq!(values, Some(json!({"output": "hello", "status": "ok"})));
    }

    #[test]
    fn map_output_updates_respects_return_precedence() {
        let task = TaskDescriptor::new("t1", "node_a", Vec::new());
        let writes = vec![
            crate::langgraph_rs::core::types::ChannelWrite::new("output", json!("value")),
            crate::langgraph_rs::core::types::ChannelWrite::new(RETURN, json!("done")),
        ];
        let payload = map_output_updates(
            &OutputChannels::Single("output".to_owned()),
            &[TaskOutputWrites::new(&task, &writes)],
            false,
        )
        .unwrap();

        assert_eq!(payload, json!({"node_a": "done"}));
    }

    #[test]
    fn map_output_updates_splits_duplicate_channels_for_many_outputs() {
        let task = TaskDescriptor::new("t1", "node_a", Vec::new());
        let writes = vec![
            crate::langgraph_rs::core::types::ChannelWrite::new("a", json!(1)),
            crate::langgraph_rs::core::types::ChannelWrite::new("a", json!(2)),
            crate::langgraph_rs::core::types::ChannelWrite::new("b", json!(3)),
        ];
        let payload = map_output_updates(
            &OutputChannels::Many(vec!["a".to_owned(), "b".to_owned()]),
            &[TaskOutputWrites::new(&task, &writes)],
            true,
        )
        .unwrap();

        assert_eq!(
            payload,
            json!({
                "node_a": [{"a": 1}, {"a": 2}, {"b": 3}],
                "__metadata__": {"cached": true}
            })
        );
    }

    #[test]
    fn map_output_updates_ignores_error_and_interrupt_prefixed_writes() {
        let task_error = TaskDescriptor::new("t1", "node_a", Vec::new());
        let task_ok = TaskDescriptor::new("t2", "node_b", Vec::new());
        let error_writes = vec![crate::langgraph_rs::core::types::ChannelWrite::new(
            "__error__",
            json!("boom"),
        )];
        let ok_writes = vec![crate::langgraph_rs::core::types::ChannelWrite::new(
            "output",
            json!("ok"),
        )];

        let payload = map_output_updates(
            &OutputChannels::Single("output".to_owned()),
            &[
                TaskOutputWrites::new(&task_error, &error_writes),
                TaskOutputWrites::new(&task_ok, &ok_writes),
            ],
            false,
        )
        .unwrap();

        assert_eq!(payload, json!({"node_b": "ok"}));
    }

    #[test]
    fn maps_task_payload_shape() {
        let task = TaskDescriptor::new("t1", "worker", Vec::new());
        let payload = map_task_payload(&task, &json!({"x": 1}), &["input".to_owned()]);
        assert_eq!(
            payload,
            json!({
                "id": "t1",
                "name": "worker",
                "input": {"x": 1},
                "triggers": ["input"]
            })
        );
    }

    #[test]
    fn maps_task_result_payload_with_return_precedence_and_interrupts() {
        let task = TaskDescriptor::new("t1", "worker", Vec::new());
        let writes = vec![
            crate::langgraph_rs::core::types::ChannelWrite::new("output", json!("value")),
            crate::langgraph_rs::core::types::ChannelWrite::new(
                "__interrupt__",
                json!([{"id": "i1"}]),
            ),
            crate::langgraph_rs::core::types::ChannelWrite::new(RETURN, json!("done")),
        ];

        let payload =
            map_task_result_payload(&OutputChannels::Single("output".to_owned()), &task, &writes);
        assert_eq!(
            payload,
            json!({
                "id": "t1",
                "name": "worker",
                "error": null,
                "interrupts": [{"id": "i1"}],
                "result": "done"
            })
        );
    }

    #[test]
    fn maps_task_result_payload_aggregates_duplicate_writes() {
        let task = TaskDescriptor::new("t1", "worker", Vec::new());
        let writes = vec![
            crate::langgraph_rs::core::types::ChannelWrite::new("a", json!(1)),
            crate::langgraph_rs::core::types::ChannelWrite::new("a", json!(2)),
            crate::langgraph_rs::core::types::ChannelWrite::new("b", json!(3)),
        ];

        let payload = map_task_result_payload(
            &OutputChannels::Many(vec!["a".to_owned(), "b".to_owned()]),
            &task,
            &writes,
        );
        assert_eq!(
            payload,
            json!({
                "id": "t1",
                "name": "worker",
                "error": null,
                "interrupts": [],
                "result": {"a": {"$writes": [1, 2]}, "b": 3}
            })
        );
    }

    #[test]
    fn maps_debug_wrapper_shape() {
        let payload = map_debug_wrapper(4, DebugPayloadType::Task, json!({"id": "t1"}));
        assert_eq!(
            payload.get("step").and_then(|value| value.as_u64()),
            Some(4)
        );
        assert_eq!(
            payload.get("type").and_then(|value| value.as_str()),
            Some("task")
        );
        assert_eq!(payload.get("payload"), Some(&json!({"id": "t1"})));
        assert!(
            payload
                .get("timestamp")
                .and_then(|value| value.as_str())
                .is_some_and(|value| value.ends_with('Z'))
        );
    }

    #[test]
    fn maps_checkpoint_payload_shape() {
        let config = CheckpointConfig::new("thread-io");
        let metadata = CheckpointMetadata {
            step: Some(3),
            ..Default::default()
        };
        let task = TaskDescriptor::new("t1", "worker", Vec::new());
        let writes = vec![crate::langgraph_rs::core::types::ChannelWrite::new(
            "output",
            json!("ok"),
        )];
        let task_outputs = vec![TaskOutputWrites::new(&task, &writes)];

        let payload = map_checkpoint_payload(
            &config,
            None,
            &metadata,
            json!({"output": "ok"}),
            std::slice::from_ref(&task),
            &task_outputs,
            &OutputChannels::Single("output".to_owned()),
        );

        assert_eq!(
            payload
                .get("config")
                .and_then(|value| value.get("configurable"))
                .and_then(|value| value.get("thread_id"))
                .and_then(|value| value.as_str()),
            Some("thread-io")
        );
        assert_eq!(payload.get("values"), Some(&json!({"output": "ok"})));
        assert_eq!(payload.get("next"), Some(&json!(["worker"])));
        assert_eq!(
            payload
                .get("tasks")
                .and_then(|value| value.as_array())
                .map(|tasks| tasks.len()),
            Some(1)
        );
    }
}
