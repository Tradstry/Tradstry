#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::atomic::{AtomicU32, Ordering}};

    use serde_json::{Value, json};

    use crate::langgraph_rs::{
        core::{
            channels::{Channel, LastValue},
            scheduler::NodeScheduleSpec,
            types::{ChannelWrite, ExecutionContext, NodeExecutionError, NodeExecutionResult},
        },
        runtime::r#loop::{LoopConfig, LoopEngine, LoopNodeRunner},
    };

    struct FlakyRunner {
        calls: AtomicU32,
    }

    impl FlakyRunner {
        fn new() -> Self {
            Self {
                calls: AtomicU32::new(0),
            }
        }
    }

    impl LoopNodeRunner for FlakyRunner {
        fn execute(
            &self,
            _node_name: &str,
            input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            let attempt = self.calls.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                return Err(NodeExecutionError::retryable("transient"));
            }
            Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                "output",
                input.get("input").cloned().unwrap_or(Value::Null),
            )))
        }
    }

    #[test]
    fn retry_parity_retries_retryable_failures_then_succeeds() {
        let mut channels = BTreeMap::<String, Box<dyn Channel>>::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let engine = LoopEngine::new(
            channels,
            vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])],
        );
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("parity-retry"),
        )
        .with_retry_limit(1)
        .with_recursion_limit(4);
        let runner = FlakyRunner::new();

        let result = engine
            .run(
                &runner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("hello"))],
            )
            .unwrap();

        assert_eq!(result.task_reports.len(), 1);
        assert_eq!(result.task_reports[0].attempts, 2);
        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("hello"))
        );
    }
}
