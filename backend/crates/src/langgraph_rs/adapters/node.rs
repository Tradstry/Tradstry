use std::{fmt, sync::Arc};

use serde_json::Value;

use crate::langgraph_rs::core::types::{NodeExecutionError, NodeExecutionResult};

use super::AdapterContext;

pub trait AdapterNode: Send + Sync {
    fn execute(
        &self,
        input: Value,
        ctx: &AdapterContext,
    ) -> Result<NodeExecutionResult, NodeExecutionError>;
}

pub type AdapterHandler = dyn Fn(Value, &AdapterContext) -> Result<NodeExecutionResult, NodeExecutionError>
    + Send
    + Sync
    + 'static;

#[derive(Clone)]
pub struct FnAdapterNode {
    handler: Arc<AdapterHandler>,
}

impl fmt::Debug for FnAdapterNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FnAdapterNode").finish_non_exhaustive()
    }
}

impl FnAdapterNode {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(Value, &AdapterContext) -> Result<NodeExecutionResult, NodeExecutionError>
            + Send
            + Sync
            + 'static,
    {
        Self {
            handler: Arc::new(handler),
        }
    }
}

impl AdapterNode for FnAdapterNode {
    fn execute(
        &self,
        input: Value,
        ctx: &AdapterContext,
    ) -> Result<NodeExecutionResult, NodeExecutionError> {
        (self.handler)(input, ctx)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::{
        adapters::{AdapterContext, AdapterNode, FnAdapterNode},
        core::types::NodeExecutionResult,
    };

    #[test]
    fn fn_node_executes_handler() {
        let node = FnAdapterNode::new(|input, _ctx| {
            Ok(NodeExecutionResult::default().with_return_value(input))
        });
        let ctx = AdapterContext {
            node_name: "n".to_owned(),
            task_id: "t".to_owned(),
            task_name: "n".to_owned(),
            step: 0,
            recursion_limit: 10,
        };
        let result = node.execute(json!({"v": 1}), &ctx).unwrap();
        assert_eq!(result.return_value, Some(json!({"v": 1})));
    }
}
