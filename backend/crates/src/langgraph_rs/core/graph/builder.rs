use std::collections::BTreeMap;

use crate::langgraph_rs::core::{
    channels::Channel,
    types::{ChannelName, NodeName},
};

use super::{
    CompiledGraph, GraphDefinition, GraphEdgeKind, GraphEdgeSpec, GraphError, GraphNodeSpec,
};

#[derive(Debug, Clone, Default)]
pub struct GraphBuilder {
    channels: BTreeMap<ChannelName, Box<dyn Channel>>,
    nodes: BTreeMap<NodeName, GraphNodeSpec>,
    edges: Vec<GraphEdgeSpec>,
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_channel(
        &mut self,
        channel_name: impl Into<ChannelName>,
        mut channel: Box<dyn Channel>,
    ) -> Result<&mut Self, GraphError> {
        let channel_name = channel_name.into();
        if self.channels.contains_key(&channel_name) {
            return Err(GraphError::DuplicateChannel {
                channel: channel_name,
            });
        }
        channel.set_key(channel_name.clone());
        self.channels.insert(channel_name, channel);
        Ok(self)
    }

    pub fn add_node(&mut self, node_name: impl Into<NodeName>) -> Result<&mut Self, GraphError> {
        let node_name = node_name.into();
        if self.nodes.contains_key(&node_name) {
            return Err(GraphError::DuplicateNode { node: node_name });
        }
        self.nodes
            .insert(node_name.clone(), GraphNodeSpec::new(node_name));
        Ok(self)
    }

    pub fn add_node_with_triggers(
        &mut self,
        node_name: impl Into<NodeName>,
        triggers: impl IntoIterator<Item = ChannelName>,
    ) -> Result<&mut Self, GraphError> {
        let node_name = node_name.into();
        self.add_node(node_name.clone())?;

        for trigger in triggers {
            self.add_trigger(&node_name, trigger)?;
        }
        Ok(self)
    }

    pub fn add_trigger(
        &mut self,
        node_name: impl AsRef<str>,
        channel_name: impl Into<ChannelName>,
    ) -> Result<&mut Self, GraphError> {
        let node_name = node_name.as_ref();
        let channel_name = channel_name.into();
        self.ensure_node_exists(node_name)?;
        self.ensure_channel_exists(&channel_name)?;

        let node = self
            .nodes
            .get_mut(node_name)
            .ok_or_else(|| GraphError::unknown_node(node_name))?;
        node.add_trigger(channel_name);
        Ok(self)
    }

    pub fn set_node_input_channels(
        &mut self,
        node_name: impl AsRef<str>,
        input_channels: impl IntoIterator<Item = ChannelName>,
    ) -> Result<&mut Self, GraphError> {
        let node_name = node_name.as_ref();
        self.ensure_node_exists(node_name)?;
        let node = self
            .nodes
            .get_mut(node_name)
            .ok_or_else(|| GraphError::unknown_node(node_name))?;
        node.input_channels = Some(input_channels.into_iter().collect());
        Ok(self)
    }

    pub fn add_edge(
        &mut self,
        from_node: impl AsRef<str>,
        to_node: impl AsRef<str>,
        channel_name: impl Into<ChannelName>,
    ) -> Result<&mut Self, GraphError> {
        let from_node = from_node.as_ref().to_owned();
        let to_node = to_node.as_ref().to_owned();
        let channel_name = channel_name.into();

        self.ensure_node_exists(&from_node)?;
        self.ensure_node_exists(&to_node)?;
        self.ensure_channel_exists(&channel_name)?;
        self.add_trigger(&to_node, channel_name.clone())?;

        self.edges
            .push(GraphEdgeSpec::direct(from_node, to_node, channel_name));
        Ok(self)
    }

    pub fn add_conditional_edge(
        &mut self,
        from_node: impl AsRef<str>,
        to_node: impl AsRef<str>,
        channel_name: impl Into<ChannelName>,
        branch: impl Into<String>,
    ) -> Result<&mut Self, GraphError> {
        let from_node = from_node.as_ref().to_owned();
        let to_node = to_node.as_ref().to_owned();
        let channel_name = channel_name.into();
        let branch = branch.into();

        if branch.trim().is_empty() {
            return Err(GraphError::InvalidBranch {
                branch,
                message: "branch cannot be empty or whitespace".to_owned(),
            });
        }

        self.ensure_node_exists(&from_node)?;
        self.ensure_node_exists(&to_node)?;
        self.ensure_channel_exists(&channel_name)?;

        let branch_conflict = self.edges.iter().any(|edge| {
            edge.from == from_node
                && matches!(
                    &edge.kind,
                    GraphEdgeKind::Conditional {
                        branch: existing_branch
                    } if existing_branch == &branch && edge.to != to_node
                )
        });
        if branch_conflict {
            return Err(GraphError::ConflictingConditionalRoute {
                from: from_node,
                branch,
            });
        }

        self.add_trigger(&to_node, channel_name.clone())?;
        self.edges.push(GraphEdgeSpec::conditional(
            from_node,
            to_node,
            channel_name,
            branch,
        ));
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), GraphError> {
        if self.channels.is_empty() {
            return Err(GraphError::validation(
                "graph must contain at least one channel",
            ));
        }
        if self.nodes.is_empty() {
            return Err(GraphError::validation(
                "graph must contain at least one node",
            ));
        }

        for node in self.nodes.values() {
            for trigger in &node.triggers {
                if !self.channels.contains_key(trigger) {
                    return Err(GraphError::unknown_channel(trigger));
                }
            }
        }

        let mut branch_destinations = BTreeMap::<(String, String), String>::new();
        for edge in &self.edges {
            if !self.nodes.contains_key(&edge.from) {
                return Err(GraphError::unknown_node(edge.from.clone()));
            }
            if !self.nodes.contains_key(&edge.to) {
                return Err(GraphError::unknown_node(edge.to.clone()));
            }
            if !self.channels.contains_key(&edge.channel) {
                return Err(GraphError::unknown_channel(edge.channel.clone()));
            }
            if let GraphEdgeKind::Conditional { branch } = &edge.kind {
                let key = (edge.from.clone(), branch.clone());
                if let Some(existing_to) = branch_destinations.insert(key.clone(), edge.to.clone())
                    && existing_to != edge.to
                {
                    return Err(GraphError::ConflictingConditionalRoute {
                        from: edge.from.clone(),
                        branch: branch.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn build(&self) -> Result<GraphDefinition, GraphError> {
        self.validate()?;
        Ok(GraphDefinition {
            channels: self.channels.clone(),
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
        })
    }

    pub fn compile(&self) -> Result<CompiledGraph, GraphError> {
        let definition = self.build()?;
        CompiledGraph::from_definition(definition)
    }

    fn ensure_node_exists(&self, node_name: &str) -> Result<(), GraphError> {
        if self.nodes.contains_key(node_name) {
            Ok(())
        } else {
            Err(GraphError::unknown_node(node_name))
        }
    }

    fn ensure_channel_exists(&self, channel_name: &str) -> Result<(), GraphError> {
        if self.channels.contains_key(channel_name) {
            Ok(())
        } else {
            Err(GraphError::unknown_channel(channel_name))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::langgraph_rs::core::channels::LastValue;

    use super::GraphBuilder;

    #[test]
    fn edge_addition_automatically_registers_target_trigger() {
        let mut builder = GraphBuilder::new();
        builder
            .add_channel("a", Box::new(LastValue::new("a")))
            .unwrap()
            .add_node("n1")
            .unwrap()
            .add_node("n2")
            .unwrap()
            .add_edge("n1", "n2", "a")
            .unwrap();

        let definition = builder.build().unwrap();
        let n2 = definition.nodes.get("n2").unwrap();
        assert!(n2.triggers.contains("a"));
    }

    #[test]
    fn conditional_branch_conflict_is_rejected() {
        let mut builder = GraphBuilder::new();
        builder
            .add_channel("a", Box::new(LastValue::new("a")))
            .unwrap()
            .add_node("root")
            .unwrap()
            .add_node("left")
            .unwrap()
            .add_node("right")
            .unwrap();

        builder
            .add_conditional_edge("root", "left", "a", "yes")
            .unwrap();
        let err = builder
            .add_conditional_edge("root", "right", "a", "yes")
            .unwrap_err();
        assert!(format!("{err}").contains("conditional branch conflict"));
    }

    #[test]
    fn unknown_channel_trigger_fails_validation() {
        let mut builder = GraphBuilder::new();
        builder
            .add_channel("known", Box::new(LastValue::new("known")))
            .unwrap()
            .add_node("n1")
            .unwrap();

        let err = builder.add_trigger("n1", "missing").unwrap_err();
        assert!(format!("{err}").contains("unknown channel"));
    }
}
