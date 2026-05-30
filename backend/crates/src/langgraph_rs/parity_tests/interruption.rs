#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use crate::langgraph_rs::{
        checkpoint::memory::InMemorySaver,
        core::{
            channels::{Channel, LastValue},
            scheduler::NodeScheduleSpec,
            types::{ChannelWrite, ExecutionContext, NodeExecutionError, NodeExecutionResult},
        },
        runtime::{
            interrupts::InterruptSelector,
            r#loop::{LoopConfig, LoopEngine, LoopInput, LoopNodeRunner, LoopStatus},
        },
    };

    struct EchoRunner;

    #[async_trait::async_trait]
    impl LoopNodeRunner for EchoRunner {
        async fn execute(
            &self,
            _node_name: &str,
            input: Value,
            _ctx: ExecutionContext,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                "output",
                input.get("input").cloned().unwrap_or(Value::Null),
            )))
        }
    }

    #[tokio::test]
    async fn interruption_gate_requires_updates_before_next_interrupt() {
        let mut channels = BTreeMap::<String, Box<dyn Channel>>::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let saver = InMemorySaver::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("parity-interruption"),
        )
        .with_interrupt_before(InterruptSelector::all())
        .with_recursion_limit(4);

        let runner = std::sync::Arc::new(EchoRunner) as std::sync::Arc<dyn LoopNodeRunner>;
        let first = engine
            .run_with_input(
                std::sync::Arc::clone(&runner),
                Some(&saver),
                config.clone(),
                LoopInput::Writes(vec![ChannelWrite::new("input", json!("x"))]),
            )
            .await
            .unwrap();
        assert_eq!(first.status, LoopStatus::InterruptedBefore);

        let second = engine
            .run_with_input(
                std::sync::Arc::clone(&runner),
                Some(&saver),
                config,
                LoopInput::None,
            )
            .await
            .unwrap();
        assert_eq!(second.status, LoopStatus::Done);
        assert_eq!(
            second.checkpoint.channel_values.get("output"),
            Some(&json!("x"))
        );
    }
}
