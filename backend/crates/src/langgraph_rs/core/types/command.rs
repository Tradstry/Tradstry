use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use super::{ChannelWrite, NodeName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommandGraph {
    #[default]
    Current,
    #[serde(alias = "__parent__")]
    Parent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SendPacket {
    pub node: NodeName,
    pub arg: Value,
}

impl SendPacket {
    pub fn new(node: impl Into<NodeName>, arg: Value) -> Self {
        Self {
            node: node.into(),
            arg,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum GotoTarget {
    Node(NodeName),
    Send(SendPacket),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommandUpdate {
    Root(Value),
    Object(Map<String, Value>),
    Tuples(Vec<ChannelWrite>),
}

impl CommandUpdate {
    pub fn into_writes(self) -> Vec<ChannelWrite> {
        match self {
            Self::Root(value) => vec![ChannelWrite::new("__root__", value)],
            Self::Object(map) => map
                .into_iter()
                .map(|(channel, value)| ChannelWrite::new(channel, value))
                .collect(),
            Self::Tuples(writes) => writes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Command {
    #[serde(default)]
    pub graph: CommandGraph,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<CommandUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "deserialize_goto_targets"
    )]
    pub goto: Vec<GotoTarget>,
}

impl Command {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn to_parent(mut self) -> Self {
        self.graph = CommandGraph::Parent;
        self
    }

    pub fn with_update(mut self, update: CommandUpdate) -> Self {
        self.update = Some(update);
        self
    }

    pub fn with_resume(mut self, resume: Value) -> Self {
        self.resume = Some(resume);
        self
    }

    pub fn with_goto(mut self, goto: GotoTarget) -> Self {
        self.goto.push(goto);
        self
    }

    pub fn update_as_writes(&self) -> Vec<ChannelWrite> {
        self.update
            .clone()
            .map(CommandUpdate::into_writes)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum OneOrMany<T> {
    Many(Vec<T>),
    One(T),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum GotoSerde {
    Tagged(GotoTarget),
    Node(NodeName),
    Send(SendPacket),
}

impl From<GotoSerde> for GotoTarget {
    fn from(value: GotoSerde) -> Self {
        match value {
            GotoSerde::Tagged(target) => target,
            GotoSerde::Node(node_name) => GotoTarget::Node(node_name),
            GotoSerde::Send(packet) => GotoTarget::Send(packet),
        }
    }
}

fn deserialize_goto_targets<'de, D>(deserializer: D) -> Result<Vec<GotoTarget>, D::Error>
where
    D: Deserializer<'de>,
{
    let targets = Option::<OneOrMany<GotoSerde>>::deserialize(deserializer)?;
    Ok(match targets {
        None => Vec::new(),
        Some(OneOrMany::One(target)) => vec![target.into()],
        Some(OneOrMany::Many(targets)) => targets.into_iter().map(Into::into).collect(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::core::types::{Command, CommandGraph, GotoTarget};

    #[test]
    fn deserializes_python_parent_graph_alias() {
        let command: Command = serde_json::from_value(json!({
            "graph": "__parent__",
            "update": {"input": "hi"}
        }))
        .unwrap();

        assert_eq!(command.graph, CommandGraph::Parent);
    }

    #[test]
    fn deserializes_single_goto_shape() {
        let command: Command = serde_json::from_value(json!({
            "goto": "worker"
        }))
        .unwrap();

        assert_eq!(command.goto.len(), 1);
        assert!(matches!(&command.goto[0], GotoTarget::Node(name) if name == "worker"));
    }

    #[test]
    fn deserializes_list_goto_shape() {
        let command: Command = serde_json::from_value(json!({
            "goto": [
                "worker",
                {"node": "other", "arg": {"k": 1}}
            ]
        }))
        .unwrap();

        assert_eq!(command.goto.len(), 2);
        assert!(matches!(&command.goto[0], GotoTarget::Node(name) if name == "worker"));
        assert!(matches!(&command.goto[1], GotoTarget::Send(packet) if packet.node == "other"));
    }
}
