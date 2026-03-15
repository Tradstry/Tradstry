use serde_json::Value;

use crate::langgraph_rs::{
    adapters::{AdapterContext, AdapterNode, FnAdapterNode},
    core::types::{ChannelWrite, NodeExecutionError, NodeExecutionResult},
};

pub type RigAdapterErrorMapper = fn(String) -> NodeExecutionError;

#[derive(Clone, Debug)]
pub struct RigNodeAdapter {
    inner: FnAdapterNode,
}

impl RigNodeAdapter {
    pub fn from_handler<F>(handler: F) -> Self
    where
        F: Fn(Value, &AdapterContext) -> Result<NodeExecutionResult, NodeExecutionError>
            + Send
            + Sync
            + 'static,
    {
        Self {
            inner: FnAdapterNode::new(handler),
        }
    }

    pub fn from_text_handler<F>(output_channel: impl Into<String>, handler: F) -> Self
    where
        F: Fn(Value, &AdapterContext) -> Result<String, NodeExecutionError> + Send + Sync + 'static,
    {
        let output_channel = output_channel.into();
        Self::from_handler(move |input, ctx| {
            let text = handler(input, ctx)?;
            Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                output_channel.clone(),
                Value::String(text),
            )))
        })
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

    pub fn map_provider_error(error: impl ToString) -> NodeExecutionError {
        NodeExecutionError::fatal(format!("rig adapter error: {}", error.to_string()))
    }
}

impl AdapterNode for RigNodeAdapter {
    fn execute(
        &self,
        input: Value,
        ctx: &AdapterContext,
    ) -> Result<NodeExecutionResult, NodeExecutionError> {
        self.inner.execute(input, ctx)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::{
        adapters::{AdapterContext, AdapterNode},
        core::types::NodeExecutionErrorKind,
    };

    use super::RigNodeAdapter;

    #[test]
    fn text_handler_maps_to_channel_write() {
        let adapter =
            RigNodeAdapter::from_text_handler("messages", |_input, _ctx| Ok("hello".to_owned()));
        let ctx = AdapterContext {
            node_name: "rig".to_owned(),
            task_id: "t".to_owned(),
            task_name: "rig".to_owned(),
            step: 1,
            recursion_limit: 10,
        };
        let result = adapter.execute(json!({}), &ctx).unwrap();

        assert_eq!(result.writes.len(), 1);
        assert_eq!(result.writes[0].channel, "messages");
        assert_eq!(result.writes[0].value, json!("hello"));
    }

    #[test]
    fn map_provider_error_creates_fatal_node_error() {
        let error = RigNodeAdapter::map_provider_error("network");
        assert_eq!(error.kind, NodeExecutionErrorKind::Fatal);
        assert!(error.message.contains("rig adapter error"));
    }
}
