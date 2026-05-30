use serde_json::Value;

use crate::langgraph_rs::{
    adapters::{AdapterContext, AdapterNode, FnAdapterNode},
    core::types::{ChannelWrite, NodeExecutionError, NodeExecutionResult},
};

pub type LangChainAdapterErrorMapper = fn(String) -> NodeExecutionError;

#[derive(Clone, Debug)]
pub struct LangChainNodeAdapter {
    inner: FnAdapterNode,
}

impl LangChainNodeAdapter {
    pub fn from_handler<F>(handler: F) -> Self
    where
        F: Fn(Value, &AdapterContext) -> Result<NodeExecutionResult, NodeExecutionError>
            + Send
            + Sync
            + 'static,
    {
        Self {
            inner: FnAdapterNode::new(move |input, ctx| {
                let result = handler(input, ctx);
                async move { result }
            }),
        }
    }

    pub fn from_value_handler<F>(output_channel: impl Into<String>, handler: F) -> Self
    where
        F: Fn(Value, &AdapterContext) -> Result<Value, NodeExecutionError> + Send + Sync + 'static,
    {
        let output_channel = output_channel.into();
        Self::from_handler(move |input, ctx| {
            let value = handler(input, ctx)?;
            Ok(NodeExecutionResult::default()
                .with_write(ChannelWrite::new(output_channel.clone(), value)))
        })
    }

    pub fn from_message_handler<F>(output_channel: impl Into<String>, handler: F) -> Self
    where
        F: Fn(Value, &AdapterContext) -> Result<Vec<String>, NodeExecutionError>
            + Send
            + Sync
            + 'static,
    {
        let output_channel = output_channel.into();
        Self::from_handler(move |input, ctx| {
            let messages = handler(input, ctx)?;
            let payload = messages.into_iter().map(Value::String).collect::<Vec<_>>();
            Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                output_channel.clone(),
                Value::Array(payload),
            )))
        })
    }

    pub fn map_provider_error(error: impl ToString) -> NodeExecutionError {
        NodeExecutionError::fatal(format!(
            "langchain-rust adapter error: {}",
            error.to_string()
        ))
    }
}

#[async_trait::async_trait]
impl AdapterNode for LangChainNodeAdapter {
    async fn execute(
        &self,
        input: Value,
        ctx: &AdapterContext,
    ) -> Result<NodeExecutionResult, NodeExecutionError> {
        self.inner.execute(input, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::{
        adapters::{AdapterContext, AdapterNode},
        core::types::NodeExecutionErrorKind,
    };

    use super::LangChainNodeAdapter;

    #[tokio::test]
    async fn message_handler_maps_to_array_write() {
        let adapter = LangChainNodeAdapter::from_message_handler("messages", |_input, _ctx| {
            Ok(vec!["a".to_owned(), "b".to_owned()])
        });
        let ctx = AdapterContext {
            node_name: "chain".to_owned(),
            task_id: "t".to_owned(),
            task_name: "chain".to_owned(),
            step: 1,
            recursion_limit: 10,
        };
        let result = adapter.execute(json!({}), &ctx).await.unwrap();

        assert_eq!(result.writes.len(), 1);
        assert_eq!(result.writes[0].channel, "messages");
        assert_eq!(result.writes[0].value, json!(["a", "b"]));
    }

    #[test]
    fn map_provider_error_creates_fatal_node_error() {
        let error = LangChainNodeAdapter::map_provider_error("decode");
        assert_eq!(error.kind, NodeExecutionErrorKind::Fatal);
        assert!(error.message.contains("langchain-rust adapter error"));
    }
}
