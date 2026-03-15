use std::collections::{BTreeMap, BTreeSet};

use crate::langgraph_rs::core::{
    channels::Channel,
    scheduler::NodeScheduleSpec,
    types::{ChannelName, NodeName},
};
use crate::langgraph_rs::runtime::runner::RetryPolicy;

#[derive(Debug, Clone)]
pub struct GraphNodeSpec {
    pub name: NodeName,
    pub triggers: BTreeSet<ChannelName>,
    pub input_channels: Option<Vec<ChannelName>>,
    pub retry_policy: Option<Vec<RetryPolicy>>,
    pub cache_enabled: Option<bool>,
}

impl GraphNodeSpec {
    pub fn new(name: impl Into<NodeName>) -> Self {
        Self {
            name: name.into(),
            triggers: BTreeSet::new(),
            input_channels: None,
            retry_policy: None,
            cache_enabled: None,
        }
    }

    pub fn with_triggers(mut self, triggers: impl IntoIterator<Item = ChannelName>) -> Self {
        self.triggers.extend(triggers);
        self
    }

    pub fn add_trigger(&mut self, trigger: impl Into<ChannelName>) {
        self.triggers.insert(trigger.into());
    }

    pub fn with_input_channels(
        mut self,
        input_channels: impl IntoIterator<Item = ChannelName>,
    ) -> Self {
        self.input_channels = Some(input_channels.into_iter().collect());
        self
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphEdgeKind {
    Direct,
    Conditional { branch: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdgeSpec {
    pub from: NodeName,
    pub to: NodeName,
    pub channel: ChannelName,
    pub kind: GraphEdgeKind,
}

impl GraphEdgeSpec {
    pub fn direct(
        from: impl Into<NodeName>,
        to: impl Into<NodeName>,
        channel: impl Into<ChannelName>,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            channel: channel.into(),
            kind: GraphEdgeKind::Direct,
        }
    }

    pub fn conditional(
        from: impl Into<NodeName>,
        to: impl Into<NodeName>,
        channel: impl Into<ChannelName>,
        branch: impl Into<String>,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            channel: channel.into(),
            kind: GraphEdgeKind::Conditional {
                branch: branch.into(),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct GraphDefinition {
    pub channels: BTreeMap<ChannelName, Box<dyn Channel>>,
    pub nodes: BTreeMap<NodeName, GraphNodeSpec>,
    pub edges: Vec<GraphEdgeSpec>,
}

impl GraphDefinition {
    pub fn to_scheduler_specs(&self) -> Vec<NodeScheduleSpec> {
        self.nodes
            .values()
            .map(|node| {
                let mut spec =
                    NodeScheduleSpec::new(node.name.clone(), node.triggers.iter().cloned());
                if let Some(input_channels) = &node.input_channels {
                    spec = spec.with_input_channels(input_channels.clone());
                }
                if let Some(retry_policy) = &node.retry_policy {
                    spec = spec.with_retry_policy(retry_policy.clone());
                }
                if let Some(cache_enabled) = node.cache_enabled {
                    spec = spec.with_cache_enabled(cache_enabled);
                }
                spec
            })
            .collect()
    }
}
