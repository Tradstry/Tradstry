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

    impl LoopNodeRunner for EchoRunner {
        fn execute(
            &self,
            _node_name: &str,
            input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                "output",
                input.get("input").cloned().unwrap_or(Value::Null),
            )))
        }
    }

    #[test]
    fn interruption_gate_requires_updates_before_next_interrupt() {
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

        let first = engine
            .run_with_input(
                &EchoRunner,
                Some(&saver),
                config.clone(),
                LoopInput::Writes(vec![ChannelWrite::new("input", json!("x"))]),
            )
            .unwrap();
        assert_eq!(first.status, LoopStatus::InterruptedBefore);

        let second = engine
            .run_with_input(&EchoRunner, Some(&saver), config, LoopInput::None)
            .unwrap();
        assert_eq!(second.status, LoopStatus::Done);
        assert_eq!(
            second.checkpoint.channel_values.get("output"),
            Some(&json!("x"))
        );
    }
}
