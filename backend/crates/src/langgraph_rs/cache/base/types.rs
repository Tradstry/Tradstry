use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::langgraph_rs::core::{
    constants::{RESUME, START, TASKS},
    types::{ChannelWrite, CommandGraph, GotoTarget, NodeExecutionResult, SendPacket},
};

use super::CacheError;

pub type CacheNamespace = Vec<String>;
pub type CacheMetadata = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CacheKey {
    pub namespace: CacheNamespace,
    pub key: String,
}

impl CacheKey {
    pub fn new(namespace: CacheNamespace, key: impl Into<String>) -> Self {
        Self {
            namespace,
            key: key.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CacheSetOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: CacheMetadata,
}

impl CacheSetOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ttl_millis(mut self, ttl_millis: u64) -> Self {
        self.ttl_millis = Some(ttl_millis);
        self
    }

    pub fn with_metadata(mut self, metadata: CacheMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheItem {
    pub cache_key: CacheKey,
    pub value: Value,
    pub created_at_millis: u64,
    pub updated_at_millis: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: CacheMetadata,
}

impl CacheItem {
    pub fn new(cache_key: CacheKey, value: Value) -> Self {
        let now = now_unix_millis();
        Self {
            cache_key,
            value,
            created_at_millis: now,
            updated_at_millis: now,
            expires_at_millis: None,
            metadata: BTreeMap::new(),
        }
    }

    pub fn is_expired_at(&self, now_millis: u64) -> bool {
        self.expires_at_millis
            .is_some_and(|expires_at| now_millis >= expires_at)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskWritesCacheEnvelope {
    pub writes: Vec<ChannelWrite>,
}

pub type NodeResultCacheEnvelope = TaskWritesCacheEnvelope;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegacyNodeResultCacheEnvelope {
    pub writes: Vec<ChannelWrite>,
    pub sends: Vec<SendPacket>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_value: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_events: Vec<Value>,
}

impl From<NodeExecutionResult> for LegacyNodeResultCacheEnvelope {
    fn from(result: NodeExecutionResult) -> Self {
        Self {
            writes: result.writes,
            sends: result.sends,
            return_value: result.return_value,
            custom_events: result.custom_events,
        }
    }
}

impl From<LegacyNodeResultCacheEnvelope> for NodeExecutionResult {
    fn from(envelope: LegacyNodeResultCacheEnvelope) -> Self {
        NodeExecutionResult {
            writes: envelope.writes,
            sends: envelope.sends,
            return_value: envelope.return_value,
            custom_events: envelope.custom_events,
            message_events: Vec::new(),
            command: None,
        }
    }
}

pub fn node_result_to_cache_value(result: &NodeExecutionResult) -> Value {
    let envelope = TaskWritesCacheEnvelope {
        writes: result_to_writes(result),
    };
    serde_json::to_value(envelope).unwrap_or_else(|_| serde_json::json!({}))
}

pub fn cache_value_to_node_result(value: &Value) -> Result<NodeExecutionResult, CacheError> {
    if let Ok(envelope) = serde_json::from_value::<TaskWritesCacheEnvelope>(value.clone()) {
        return Ok(NodeExecutionResult {
            writes: envelope.writes,
            sends: Vec::new(),
            return_value: None,
            custom_events: Vec::new(),
            message_events: Vec::new(),
            command: None,
        });
    }

    let envelope: LegacyNodeResultCacheEnvelope =
        serde_json::from_value(value.clone()).map_err(|err| {
            CacheError::serialization(format!("failed to decode task-write cache envelope: {err}"))
        })?;
    let legacy = NodeExecutionResult::from(envelope);
    Ok(NodeExecutionResult {
        writes: result_to_writes(&legacy),
        sends: Vec::new(),
        return_value: None,
        custom_events: Vec::new(),
        message_events: Vec::new(),
        command: None,
    })
}

fn result_to_writes(result: &NodeExecutionResult) -> Vec<ChannelWrite> {
    let mut writes = result.writes.clone();
    for send in &result.sends {
        let value = serde_json::to_value(send).unwrap_or(Value::Null);
        writes.push(ChannelWrite::new(TASKS, value));
    }
    if let Some(return_value) = &result.return_value {
        writes.push(ChannelWrite::new("__return__", return_value.clone()));
    }
    if let Some(command) = &result.command {
        if command.graph != CommandGraph::Parent {
            for goto in &command.goto {
                match goto {
                    GotoTarget::Send(packet) => {
                        let value = serde_json::to_value(packet).unwrap_or(Value::Null);
                        writes.push(ChannelWrite::new(TASKS, value));
                    }
                    GotoTarget::Node(node) => {
                        writes.push(ChannelWrite::new(
                            format!("branch:to:{node}"),
                            Value::String(START.to_owned()),
                        ));
                    }
                }
            }
            if let Some(resume) = &command.resume {
                writes.push(ChannelWrite::new(RESUME, resume.clone()));
            }
            if let Some(update) = &command.update {
                writes.extend(update.clone().into_writes());
            }
        }
    }
    writes
}

pub fn namespace_matches_prefix(namespace: &[String], prefix: &[String]) -> bool {
    namespace.len() >= prefix.len()
        && namespace
            .iter()
            .zip(prefix.iter())
            .all(|(value, expected)| value == expected)
}

pub fn now_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::core::types::ChannelWrite;

    use super::{
        CacheItem, CacheKey, cache_value_to_node_result, namespace_matches_prefix,
        node_result_to_cache_value,
    };

    #[test]
    fn cache_item_expiration_checks_are_consistent() {
        let item = CacheItem {
            cache_key: CacheKey::new(vec!["a".to_owned()], "k"),
            value: json!(1),
            created_at_millis: 100,
            updated_at_millis: 100,
            expires_at_millis: Some(150),
            metadata: Default::default(),
        };

        assert!(!item.is_expired_at(149));
        assert!(item.is_expired_at(150));
        assert!(item.is_expired_at(151));
    }

    #[test]
    fn node_result_roundtrips_through_cache_value() {
        let result = crate::langgraph_rs::core::types::NodeExecutionResult::default()
            .with_write(ChannelWrite::new("messages", json!("hello")))
            .with_return_value(json!({"ok": true}));

        let value = node_result_to_cache_value(&result);
        let restored = cache_value_to_node_result(&value).unwrap();

        assert_eq!(restored.writes.len(), 2);
        assert!(
            restored
                .writes
                .iter()
                .any(|write| write.channel == "messages" && write.value == json!("hello"))
        );
        assert!(
            restored
                .writes
                .iter()
                .any(|write| write.channel == "__return__" && write.value == json!({"ok": true}))
        );
        assert_eq!(restored.return_value, None);
    }

    #[test]
    fn namespace_prefix_check_is_path_based() {
        assert!(namespace_matches_prefix(
            &["thread".to_owned(), "agent".to_owned()],
            &["thread".to_owned()]
        ));
        assert!(!namespace_matches_prefix(
            &["thread".to_owned()],
            &["thread".to_owned(), "agent".to_owned()]
        ));
    }
}
