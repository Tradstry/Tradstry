use std::{collections::BTreeMap, sync::Arc};

use super::{AdapterError, AdapterNode};

#[derive(Clone, Default)]
pub struct AdapterRegistry {
    nodes: BTreeMap<String, Arc<dyn AdapterNode>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_node<N>(
        mut self,
        node_name: impl Into<String>,
        node: N,
    ) -> Result<Self, AdapterError>
    where
        N: AdapterNode + 'static,
    {
        self.register_node(node_name, node)?;
        Ok(self)
    }

    pub fn register_node<N>(
        &mut self,
        node_name: impl Into<String>,
        node: N,
    ) -> Result<(), AdapterError>
    where
        N: AdapterNode + 'static,
    {
        self.register_arc(node_name, Arc::new(node))
    }

    pub fn register_arc(
        &mut self,
        node_name: impl Into<String>,
        node: Arc<dyn AdapterNode>,
    ) -> Result<(), AdapterError> {
        let node_name = node_name.into();
        if node_name.trim().is_empty() {
            return Err(AdapterError::InvalidNodeName);
        }
        if self.nodes.contains_key(&node_name) {
            return Err(AdapterError::duplicate_node(node_name));
        }
        self.nodes.insert(node_name, node);
        Ok(())
    }

    pub fn node(&self, node_name: &str) -> Option<Arc<dyn AdapterNode>> {
        self.nodes.get(node_name).cloned()
    }

    pub fn contains_node(&self, node_name: &str) -> bool {
        self.nodes.contains_key(node_name)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn node_names(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use crate::langgraph_rs::{
        adapters::{AdapterContext, AdapterRegistry, FnAdapterNode},
        core::types::{NodeExecutionError, NodeExecutionResult},
    };

    fn noop_node() -> FnAdapterNode {
        FnAdapterNode::new(|_input: Value, _ctx: &AdapterContext| async move {
            Ok::<NodeExecutionResult, NodeExecutionError>(NodeExecutionResult::default())
        })
    }

    #[test]
    fn rejects_duplicate_node_names() {
        let mut registry = AdapterRegistry::new();
        registry.register_node("n1", noop_node()).unwrap();
        let error = registry.register_node("n1", noop_node()).unwrap_err();
        assert!(format!("{error}").contains("already registered"));
    }

    #[test]
    fn rejects_empty_node_name() {
        let mut registry = AdapterRegistry::new();
        let error = registry.register_node("   ", noop_node()).unwrap_err();
        assert!(format!("{error}").contains("cannot be empty"));
    }
}
