use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::langgraph_rs::core::{
    channels::Channel,
    constants::{ERROR, INTERRUPT, RESUME, TASKS},
    types::{ChannelName, NodeName, SendPacket, TaskId},
};

use super::{
    CheckpointError, CheckpointIdStrategy, next_checkpoint_id_with_strategy, now_timestamp_string,
};

pub const CHECKPOINT_FORMAT_VERSION: u32 = 1;
pub const PYTHON_CHECKPOINT_FORMAT_VERSION_V2: u32 = 2;
pub const PYTHON_CHECKPOINT_FORMAT_VERSION_V4: u32 = 4;

pub const ERROR_WRITE_CHANNEL: &str = ERROR;
pub const SCHEDULED_WRITE_CHANNEL: &str = "__scheduled__";
pub const INTERRUPT_WRITE_CHANNEL: &str = INTERRUPT;
pub const RESUME_WRITE_CHANNEL: &str = RESUME;
pub const DEFAULT_TASKS_CHANNEL: &str = TASKS;

pub const EXCLUDED_METADATA_KEYS: &[&str] = &[
    "thread_id",
    "checkpoint_id",
    "checkpoint_ns",
    "checkpoint_map",
    "langgraph_step",
    "langgraph_node",
    "langgraph_triggers",
    "langgraph_path",
    "langgraph_checkpoint_ns",
];

pub type CheckpointId = String;
pub type ChannelVersions = BTreeMap<ChannelName, u64>;
pub type VersionsSeen = BTreeMap<NodeName, ChannelVersions>;
pub type CheckpointParents = BTreeMap<String, String>;
pub type MetadataMap = BTreeMap<String, Value>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointWireFormat {
    #[default]
    RustV1,
    PythonV2,
    PythonV4,
}

impl CheckpointWireFormat {
    pub fn version(self) -> u32 {
        match self {
            Self::RustV1 => CHECKPOINT_FORMAT_VERSION,
            Self::PythonV2 => PYTHON_CHECKPOINT_FORMAT_VERSION_V2,
            Self::PythonV4 => PYTHON_CHECKPOINT_FORMAT_VERSION_V4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointReadCompatibility {
    pub rust_v1: bool,
    pub python_v2: bool,
    pub python_v4: bool,
}

impl Default for CheckpointReadCompatibility {
    fn default() -> Self {
        Self {
            rust_v1: true,
            python_v2: true,
            python_v4: true,
        }
    }
}

impl CheckpointReadCompatibility {
    pub fn allows(&self, version: u32) -> bool {
        match version {
            CHECKPOINT_FORMAT_VERSION => self.rust_v1,
            PYTHON_CHECKPOINT_FORMAT_VERSION_V2 => self.python_v2,
            PYTHON_CHECKPOINT_FORMAT_VERSION_V4 => self.python_v4,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CheckpointCompatibilityPolicy {
    #[serde(default)]
    pub read_compat: CheckpointReadCompatibility,
    #[serde(default)]
    pub write_format: CheckpointWireFormat,
    #[serde(default)]
    pub id_strategy: CheckpointIdStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointSource {
    Input,
    Loop,
    Update,
    Fork,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CheckpointMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<CheckpointSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parents: CheckpointParents,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: MetadataMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub v: u32,
    pub id: CheckpointId,
    pub ts: String,
    #[serde(default)]
    pub channel_values: BTreeMap<ChannelName, Value>,
    #[serde(default)]
    pub channel_versions: ChannelVersions,
    #[serde(default)]
    pub versions_seen: VersionsSeen,
    #[serde(default)]
    pub pending_sends: Vec<SendPacket>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_channels: Option<Vec<ChannelName>>,
}

impl Checkpoint {
    pub fn new(id: impl Into<CheckpointId>, ts: impl Into<String>) -> Self {
        Self {
            v: CHECKPOINT_FORMAT_VERSION,
            id: id.into(),
            ts: ts.into(),
            channel_values: BTreeMap::new(),
            channel_versions: BTreeMap::new(),
            versions_seen: BTreeMap::new(),
            pending_sends: Vec::new(),
            updated_channels: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointConfig {
    pub thread_id: String,
    #[serde(default)]
    pub checkpoint_ns: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_id: Option<CheckpointId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<MetadataMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configurable: Option<MetadataMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<CheckpointCompatibilityPolicy>,
}

impl CheckpointConfig {
    pub fn new(thread_id: impl Into<String>) -> Self {
        Self {
            thread_id: thread_id.into(),
            checkpoint_ns: String::new(),
            checkpoint_id: None,
            metadata: None,
            configurable: None,
            compatibility: None,
        }
    }

    pub fn with_namespace(mut self, checkpoint_ns: impl Into<String>) -> Self {
        self.checkpoint_ns = checkpoint_ns.into();
        self
    }

    pub fn with_checkpoint_id(mut self, checkpoint_id: impl Into<CheckpointId>) -> Self {
        self.checkpoint_id = Some(checkpoint_id.into());
        self
    }

    pub fn with_metadata(mut self, metadata: MetadataMap) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_configurable(mut self, configurable: MetadataMap) -> Self {
        self.configurable = Some(configurable);
        self
    }

    pub fn with_compatibility(mut self, compatibility: CheckpointCompatibilityPolicy) -> Self {
        self.compatibility = Some(compatibility);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingWrite {
    pub task_id: TaskId,
    pub channel: ChannelName,
    pub value: Value,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub task_path: String,
}

impl PendingWrite {
    pub fn new(task_id: impl Into<TaskId>, channel: impl Into<ChannelName>, value: Value) -> Self {
        Self {
            task_id: task_id.into(),
            channel: channel.into(),
            value,
            task_path: String::new(),
        }
    }

    pub fn with_task_path(mut self, task_path: impl Into<String>) -> Self {
        self.task_path = task_path.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckpointTuple {
    pub config: CheckpointConfig,
    pub checkpoint: Checkpoint,
    pub metadata: CheckpointMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_config: Option<CheckpointConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_writes: Option<Vec<PendingWrite>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ListCheckpointsQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<CheckpointConfig>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata_filter: MetadataMap,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<CheckpointConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PruneStrategy {
    #[default]
    KeepLatest,
    Delete,
}

pub fn empty_checkpoint() -> Checkpoint {
    empty_checkpoint_with_config(None)
}

pub fn empty_checkpoint_with_config(config: Option<&CheckpointConfig>) -> Checkpoint {
    let compatibility = effective_checkpoint_compatibility(config);
    Checkpoint {
        v: compatibility.write_format.version(),
        id: next_checkpoint_id_with_strategy(-2, compatibility.id_strategy),
        ts: now_timestamp_string(),
        channel_values: BTreeMap::new(),
        channel_versions: BTreeMap::new(),
        versions_seen: BTreeMap::new(),
        pending_sends: Vec::new(),
        updated_channels: None,
    }
}

pub fn copy_checkpoint(checkpoint: &Checkpoint) -> Checkpoint {
    checkpoint.clone()
}

pub fn create_checkpoint(
    checkpoint: &Checkpoint,
    channels: Option<&BTreeMap<ChannelName, Box<dyn Channel>>>,
    step: i64,
    id: Option<CheckpointId>,
) -> Result<Checkpoint, CheckpointError> {
    create_checkpoint_with_config(checkpoint, channels, step, id, None)
}

pub fn create_checkpoint_with_config(
    checkpoint: &Checkpoint,
    channels: Option<&BTreeMap<ChannelName, Box<dyn Channel>>>,
    step: i64,
    id: Option<CheckpointId>,
    config: Option<&CheckpointConfig>,
) -> Result<Checkpoint, CheckpointError> {
    let compatibility = effective_checkpoint_compatibility(config);
    let channel_values = match channels {
        None => checkpoint.channel_values.clone(),
        Some(channels) => {
            let mut values = BTreeMap::new();
            for (channel_name, channel) in channels {
                if !checkpoint.channel_versions.contains_key(channel_name) {
                    continue;
                }
                if let Some(value) = channel.checkpoint()? {
                    values.insert(channel_name.clone(), value);
                }
            }
            values
        }
    };

    Ok(Checkpoint {
        v: compatibility.write_format.version(),
        ts: now_timestamp_string(),
        id: id.unwrap_or_else(|| next_checkpoint_id_with_strategy(step, compatibility.id_strategy)),
        channel_values,
        channel_versions: checkpoint.channel_versions.clone(),
        versions_seen: checkpoint.versions_seen.clone(),
        pending_sends: checkpoint.pending_sends.clone(),
        updated_channels: None,
    })
}

pub fn get_checkpoint_id(config: &CheckpointConfig) -> Option<&str> {
    config.checkpoint_id.as_deref()
}

pub fn get_checkpoint_metadata(
    config: &CheckpointConfig,
    metadata: &CheckpointMetadata,
) -> CheckpointMetadata {
    let mut normalized = metadata.clone();
    sanitize_metadata_strings(&mut normalized);

    for source in [&config.metadata, &config.configurable] {
        let Some(values) = source else {
            continue;
        };
        for (key, value) in values {
            if is_excluded_metadata_key(key)
                || key.starts_with("__")
                || metadata_has_key(&normalized, key)
            {
                continue;
            }

            if let Some(value) = normalize_metadata_value(value) {
                normalized.extra.insert(key.clone(), value);
            }
        }
    }

    normalized
}

pub fn get_serializable_checkpoint_metadata(
    config: &CheckpointConfig,
    metadata: &CheckpointMetadata,
) -> CheckpointMetadata {
    let mut normalized = get_checkpoint_metadata(config, metadata);
    normalized.extra.remove("writes");
    normalized
}

pub fn effective_checkpoint_compatibility(
    config: Option<&CheckpointConfig>,
) -> CheckpointCompatibilityPolicy {
    config
        .and_then(|config| config.compatibility.clone())
        .unwrap_or_default()
}

pub fn normalize_checkpoint_for_read(
    checkpoint: Checkpoint,
    config: &CheckpointConfig,
) -> Result<Checkpoint, CheckpointError> {
    let compatibility = effective_checkpoint_compatibility(Some(config));
    normalize_checkpoint_with_compatibility(checkpoint, &compatibility.read_compat)
}

pub fn normalize_checkpoint_with_compatibility(
    mut checkpoint: Checkpoint,
    read_compat: &CheckpointReadCompatibility,
) -> Result<Checkpoint, CheckpointError> {
    if !read_compat.allows(checkpoint.v) {
        return Err(CheckpointError::serialization(format!(
            "unsupported checkpoint format version '{}'",
            checkpoint.v
        )));
    }
    checkpoint.v = CHECKPOINT_FORMAT_VERSION;
    Ok(checkpoint)
}

pub fn project_checkpoint_for_storage(
    mut checkpoint: Checkpoint,
    config: &CheckpointConfig,
) -> Checkpoint {
    let compatibility = effective_checkpoint_compatibility(Some(config));
    checkpoint.v = compatibility.write_format.version();
    checkpoint
}

pub fn deserialize_checkpoint_json(
    checkpoint_json: &str,
    config: &CheckpointConfig,
) -> Result<Checkpoint, CheckpointError> {
    let value: Value = serde_json::from_str(checkpoint_json).map_err(|err| {
        CheckpointError::serialization(format!("failed to deserialize checkpoint payload: {err}"))
    })?;
    let raw: RawCheckpoint = serde_json::from_value(value).map_err(|err| {
        CheckpointError::serialization(format!("failed to decode checkpoint payload: {err}"))
    })?;
    let checkpoint = raw.into_checkpoint()?;
    normalize_checkpoint_for_read(checkpoint, config)
}

fn sanitize_metadata_strings(metadata: &mut CheckpointMetadata) {
    if let Some(run_id) = &mut metadata.run_id {
        *run_id = run_id.replace('\0', "");
    }
    for value in metadata.extra.values_mut() {
        if let Value::String(string) = value {
            *string = string.replace('\0', "");
        }
    }
}

fn metadata_has_key(metadata: &CheckpointMetadata, key: &str) -> bool {
    matches!(key, "source" | "step" | "parents" | "run_id") || metadata.extra.contains_key(key)
}

fn normalize_metadata_value(value: &Value) -> Option<Value> {
    match value {
        Value::String(string) => Some(Value::String(string.replace('\0', ""))),
        Value::Bool(value) => Some(Value::Bool(*value)),
        Value::Number(number) => Some(Value::Number(number.clone())),
        _ => None,
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawCheckpoint {
    v: u32,
    id: CheckpointId,
    ts: String,
    #[serde(default)]
    channel_values: BTreeMap<ChannelName, Value>,
    #[serde(default)]
    channel_versions: BTreeMap<ChannelName, Value>,
    #[serde(default)]
    versions_seen: BTreeMap<NodeName, BTreeMap<ChannelName, Value>>,
    #[serde(default)]
    pending_sends: Vec<SendPacket>,
    #[serde(default)]
    updated_channels: Option<Vec<ChannelName>>,
}

impl RawCheckpoint {
    fn into_checkpoint(self) -> Result<Checkpoint, CheckpointError> {
        Ok(Checkpoint {
            v: self.v,
            id: self.id,
            ts: self.ts,
            channel_values: self.channel_values,
            channel_versions: normalize_channel_versions(self.channel_versions)?,
            versions_seen: normalize_versions_seen(self.versions_seen)?,
            pending_sends: self.pending_sends,
            updated_channels: self.updated_channels,
        })
    }
}

fn normalize_channel_versions(
    versions: BTreeMap<ChannelName, Value>,
) -> Result<ChannelVersions, CheckpointError> {
    versions
        .into_iter()
        .map(|(channel, value)| Ok((channel, parse_channel_version(&value)?)))
        .collect()
}

fn normalize_versions_seen(
    versions_seen: BTreeMap<NodeName, BTreeMap<ChannelName, Value>>,
) -> Result<VersionsSeen, CheckpointError> {
    versions_seen
        .into_iter()
        .map(|(node, versions)| Ok((node, normalize_channel_versions(versions)?)))
        .collect()
}

fn parse_channel_version(value: &Value) -> Result<u64, CheckpointError> {
    match value {
        Value::Number(number) => {
            if let Some(version) = number.as_u64() {
                return Ok(version);
            }
            if let Some(version) = number.as_i64()
                && version >= 0
            {
                return Ok(version as u64);
            }
            if let Some(version) = number.as_f64()
                && version.is_finite()
                && version >= 0.0
            {
                return Ok(version as u64);
            }
            Err(CheckpointError::serialization(format!(
                "unsupported numeric channel version '{number}'"
            )))
        }
        Value::String(value) => value.parse::<u64>().map_err(|err| {
            CheckpointError::serialization(format!(
                "unsupported string channel version '{value}': {err}"
            ))
        }),
        other => Err(CheckpointError::serialization(format!(
            "unsupported channel version value '{other}'"
        ))),
    }
}

pub fn is_excluded_metadata_key(key: &str) -> bool {
    EXCLUDED_METADATA_KEYS.contains(&key)
}

pub fn write_idx_for_channel(channel: &str, fallback_idx: usize) -> i32 {
    match channel {
        ERROR_WRITE_CHANNEL => -1,
        SCHEDULED_WRITE_CHANNEL => -2,
        INTERRUPT_WRITE_CHANNEL => -3,
        RESUME_WRITE_CHANNEL => -4,
        _ => i32::try_from(fallback_idx).unwrap_or(i32::MAX),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::langgraph_rs::core::channels::{Channel, LastValue};

    use super::{
        CheckpointCompatibilityPolicy, CheckpointConfig, CheckpointIdStrategy, CheckpointMetadata,
        CheckpointWireFormat, create_checkpoint, create_checkpoint_with_config,
        deserialize_checkpoint_json, empty_checkpoint, get_serializable_checkpoint_metadata,
        write_idx_for_channel,
    };

    #[test]
    fn write_index_reserves_special_channels() {
        assert_eq!(write_idx_for_channel("__error__", 2), -1);
        assert_eq!(write_idx_for_channel("__scheduled__", 2), -2);
        assert_eq!(write_idx_for_channel("__interrupt__", 2), -3);
        assert_eq!(write_idx_for_channel("__resume__", 2), -4);
        assert_eq!(write_idx_for_channel("messages", 2), 2);
    }

    #[test]
    fn empty_checkpoint_is_well_formed() {
        let checkpoint = empty_checkpoint();
        assert_eq!(checkpoint.v, 1);
        assert!(!checkpoint.id.is_empty());
        assert!(checkpoint.channel_values.is_empty());
        assert!(checkpoint.channel_versions.is_empty());
        assert!(checkpoint.versions_seen.is_empty());
    }

    #[test]
    fn create_checkpoint_only_snapshots_versioned_channels() {
        let mut previous = empty_checkpoint();
        previous.channel_versions.insert("messages".to_owned(), 1);

        let mut messages = LastValue::new("messages");
        messages.update(&[json!(["hello"])]).unwrap();

        let mut ephemeral = LastValue::new("ephemeral");
        ephemeral.update(&[json!("skip")]).unwrap();

        let mut channels: BTreeMap<String, Box<dyn Channel>> = BTreeMap::new();
        channels.insert("messages".to_owned(), Box::new(messages));
        channels.insert("ephemeral".to_owned(), Box::new(ephemeral));

        let next = create_checkpoint(&previous, Some(&channels), 0, None).unwrap();

        assert_eq!(next.channel_values.get("messages"), Some(&json!(["hello"])));
        assert!(!next.channel_values.contains_key("ephemeral"));
    }

    #[test]
    fn serializable_metadata_removes_writes() {
        let mut metadata = CheckpointMetadata::default();
        metadata.extra.insert("writes".to_owned(), json!({"x": 1}));
        metadata.extra.insert("run_type".to_owned(), json!("test"));

        let serializable =
            get_serializable_checkpoint_metadata(&CheckpointConfig::new("thread"), &metadata);

        assert!(!serializable.extra.contains_key("writes"));
        assert_eq!(serializable.extra.get("run_type"), Some(&json!("test")));
    }

    #[test]
    fn metadata_merge_uses_config_sources() {
        let mut metadata = CheckpointMetadata::default();
        metadata.extra.insert("keep".to_owned(), json!("meta"));

        let config = CheckpointConfig::new("thread")
            .with_metadata(BTreeMap::from([
                ("foo".to_owned(), json!("bar\u{0000}")),
                ("keep".to_owned(), json!("override")),
                ("__private".to_owned(), json!("x")),
            ]))
            .with_configurable(BTreeMap::from([
                ("count".to_owned(), json!(2)),
                ("checkpoint_id".to_owned(), json!("skip")),
            ]));

        let merged = super::get_checkpoint_metadata(&config, &metadata);
        assert_eq!(merged.extra.get("foo"), Some(&json!("bar")));
        assert_eq!(merged.extra.get("count"), Some(&json!(2)));
        assert_eq!(merged.extra.get("keep"), Some(&json!("meta")));
        assert!(!merged.extra.contains_key("__private"));
        assert!(!merged.extra.contains_key("checkpoint_id"));
    }

    #[test]
    fn deserialize_checkpoint_json_accepts_v2_and_v4() {
        let config = CheckpointConfig::new("thread");
        let v2 = json!({
            "v": 2,
            "id": "cp-2",
            "ts": "t",
            "channel_values": {},
            "channel_versions": {"messages": 1},
            "versions_seen": {}
        });
        let v4 = json!({
            "v": 4,
            "id": "cp-4",
            "ts": "t",
            "channel_values": {},
            "channel_versions": {"messages": "2"},
            "versions_seen": {}
        });

        let cp2 =
            deserialize_checkpoint_json(&serde_json::to_string(&v2).unwrap(), &config).unwrap();
        let cp4 =
            deserialize_checkpoint_json(&serde_json::to_string(&v4).unwrap(), &config).unwrap();
        assert_eq!(cp2.v, super::CHECKPOINT_FORMAT_VERSION);
        assert_eq!(cp4.v, super::CHECKPOINT_FORMAT_VERSION);
        assert_eq!(cp4.channel_versions.get("messages"), Some(&2));
    }

    #[test]
    fn deserialize_checkpoint_json_rejects_unknown_version() {
        let config = CheckpointConfig::new("thread");
        let unknown = json!({
            "v": 99,
            "id": "cp-99",
            "ts": "t",
            "channel_values": {},
            "channel_versions": {"messages": 1},
            "versions_seen": {}
        });

        let err = deserialize_checkpoint_json(&serde_json::to_string(&unknown).unwrap(), &config)
            .expect_err("unknown version should be rejected");
        assert!(
            err.to_string()
                .contains("unsupported checkpoint format version")
        );
    }

    #[test]
    fn create_checkpoint_uses_uuid6_when_configured() {
        let previous = empty_checkpoint();
        let config =
            CheckpointConfig::new("thread").with_compatibility(CheckpointCompatibilityPolicy {
                write_format: CheckpointWireFormat::PythonV4,
                id_strategy: CheckpointIdStrategy::Uuid6,
                ..Default::default()
            });
        let checkpoint =
            create_checkpoint_with_config(&previous, None, 0, None, Some(&config)).unwrap();
        assert_eq!(checkpoint.v, 4);
        assert_eq!(checkpoint.id.len(), 36);
    }
}
