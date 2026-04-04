use std::sync::Arc;

use serde_json::Value;

use crate::langgraph_rs::core::types::ChannelWrite;

/// Maps parent state to child graph input.
pub type InputMappingFn = Arc<dyn Fn(Value) -> Value + Send + Sync>;

/// Maps child graph's final state to writes on parent channels.
pub type OutputMappingFn = Arc<dyn Fn(Value) -> Vec<ChannelWrite> + Send + Sync>;

/// Configuration for embedding a compiled child graph as a node.
pub struct SubgraphConfig {
    pub input_mapping: InputMappingFn,
    pub output_mapping: OutputMappingFn,
    pub checkpoint_ns: Option<String>,
    pub recursion_limit: Option<u64>,
}

impl std::fmt::Debug for SubgraphConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubgraphConfig")
            .field("checkpoint_ns", &self.checkpoint_ns)
            .field("recursion_limit", &self.recursion_limit)
            .finish_non_exhaustive()
    }
}
