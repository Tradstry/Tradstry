use serde_json::Value;

use crate::langgraph_rs::{
    core::types::{ExecutionContext, NodeExecutionError, NodeExecutionResult},
    runtime::r#loop::LoopNodeRunner,
};

use super::{AdapterContext, AdapterError, AdapterNode, AdapterRegistry};

#[derive(Clone, Default)]
pub struct AdapterRunner {
    registry: AdapterRegistry,
}

impl AdapterRunner {
    pub fn new(registry: AdapterRegistry) -> Self {
        Self { registry }
    }

    pub fn with_node<N>(
        mut self,
        node_name: impl Into<String>,
        node: N,
    ) -> Result<Self, AdapterError>
    where
        N: AdapterNode + 'static,
    {
        self.registry.register_node(node_name, node)?;
        Ok(self)
    }

    pub fn registry(&self) -> &AdapterRegistry {
        &self.registry
    }
}

#[async_trait::async_trait]
impl LoopNodeRunner for AdapterRunner {
    async fn execute(
        &self,
        node_name: &str,
        input: Value,
        ctx: ExecutionContext,
    ) -> Result<NodeExecutionResult, NodeExecutionError> {
        let Some(node) = self.registry.node(node_name) else {
            return Err(NodeExecutionError::fatal(format!(
                "adapter node '{node_name}' is not registered"
            )));
        };
        let adapter_ctx = AdapterContext::from_execution_context(node_name, &ctx);
        node.execute(input, &adapter_ctx).await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::{
        adapters::{AdapterRunner, FnAdapterNode},
        core::types::{ExecutionContext, TaskDescriptor, TaskPathPart},
        runtime::r#loop::LoopNodeRunner,
    };

    #[tokio::test]
    async fn dispatches_to_registered_node() {
        let runner = AdapterRunner::default()
            .with_node(
                "echo",
                FnAdapterNode::new(|input, _ctx| async move {
                    Ok(
                        crate::langgraph_rs::core::types::NodeExecutionResult::default()
                            .with_return_value(input),
                    )
                }),
            )
            .unwrap();
        let task = TaskDescriptor::new("t1", "echo", vec![TaskPathPart::Name("root".to_owned())]);
        let result = runner
            .execute(
                "echo",
                json!({"x": 1}),
                ExecutionContext {
                    task: std::sync::Arc::new(task),
                    step: 0,
                    recursion_limit: 10,
                    attempt: 1,
                    resuming: false,
                    runtime: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(result.return_value, Some(json!({"x": 1})));
    }

    #[tokio::test]
    async fn returns_fatal_error_for_unknown_node() {
        let runner = AdapterRunner::default();
        let task =
            TaskDescriptor::new("t1", "missing", vec![TaskPathPart::Name("root".to_owned())]);
        let error = runner
            .execute(
                "missing",
                json!({}),
                ExecutionContext {
                    task: std::sync::Arc::new(task),
                    step: 0,
                    recursion_limit: 10,
                    attempt: 1,
                    resuming: false,
                    runtime: None,
                },
            )
            .await
            .unwrap_err();

        assert!(format!("{error}").contains("not registered"));
    }
}
