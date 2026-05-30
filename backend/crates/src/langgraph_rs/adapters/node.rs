use std::{fmt, sync::Arc};

use futures::future::BoxFuture;
use serde_json::Value;

use crate::langgraph_rs::core::types::{NodeExecutionError, NodeExecutionResult};

use super::AdapterContext;

#[async_trait::async_trait]
pub trait AdapterNode: Send + Sync {
    async fn execute(
        &self,
        input: Value,
        ctx: &AdapterContext,
    ) -> Result<NodeExecutionResult, NodeExecutionError>;
}

pub type AdapterHandler = dyn Fn(
        Value,
        &AdapterContext,
    ) -> BoxFuture<'static, Result<NodeExecutionResult, NodeExecutionError>>
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
    pub fn new<F, Fut>(handler: F) -> Self
    where
        F: Fn(Value, &AdapterContext) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<NodeExecutionResult, NodeExecutionError>>
            + Send
            + 'static,
    {
        Self {
            handler: Arc::new(move |input, ctx| Box::pin(handler(input, ctx))),
        }
    }
}

#[async_trait::async_trait]
impl AdapterNode for FnAdapterNode {
    async fn execute(
        &self,
        input: Value,
        ctx: &AdapterContext,
    ) -> Result<NodeExecutionResult, NodeExecutionError> {
        (self.handler)(input, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::{
        adapters::{AdapterContext, AdapterNode, FnAdapterNode},
        core::types::NodeExecutionResult,
    };

    #[tokio::test]
    async fn fn_node_executes_handler() {
        let node = FnAdapterNode::new(|input, _ctx| async move {
            Ok(NodeExecutionResult::default().with_return_value(input))
        });
        let ctx = AdapterContext {
            node_name: "n".to_owned(),
            task_id: "t".to_owned(),
            task_name: "n".to_owned(),
            step: 0,
            recursion_limit: 10,
        };
        let result = node.execute(json!({"v": 1}), &ctx).await.unwrap();
        assert_eq!(result.return_value, Some(json!({"v": 1})));
    }
}
