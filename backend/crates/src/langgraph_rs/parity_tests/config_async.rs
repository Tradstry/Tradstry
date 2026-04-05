#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Mutex, thread, time::Duration};

    use serde_json::{Value, json};

    use crate::langgraph_rs::{
        checkpoint::{
            base::{
                ChannelVersions, Checkpoint, CheckpointConfig, CheckpointError, CheckpointMetadata,
                CheckpointSaver, CheckpointTuple,
            },
            memory::InMemorySaver,
        },
        core::{
            channels::{Channel, LastValue},
            scheduler::NodeScheduleSpec,
            types::{ChannelWrite, ExecutionContext, NodeExecutionError, NodeExecutionResult},
        },
        runtime::r#loop::{DurabilityMode, LoopConfig, LoopEngine, LoopInput, LoopNodeRunner},
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

    #[derive(Default)]
    struct RecordingSaver {
        inner: InMemorySaver,
        operations: Mutex<Vec<String>>,
    }

    impl RecordingSaver {
        fn operations(&self) -> Vec<String> {
            self.operations
                .lock()
                .map(|ops| ops.clone())
                .unwrap_or_default()
        }
    }

    impl CheckpointSaver for RecordingSaver {
        fn get_tuple(
            &self,
            config: &CheckpointConfig,
        ) -> Result<Option<CheckpointTuple>, CheckpointError> {
            self.inner.get_tuple(config)
        }

        fn put(
            &self,
            config: &CheckpointConfig,
            checkpoint: Checkpoint,
            metadata: CheckpointMetadata,
            new_versions: ChannelVersions,
        ) -> Result<CheckpointConfig, CheckpointError> {
            thread::sleep(Duration::from_millis(10));
            if let Ok(mut ops) = self.operations.lock() {
                ops.push(format!("put:{}", checkpoint.id));
            }
            self.inner.put(config, checkpoint, metadata, new_versions)
        }

        fn put_writes(
            &self,
            config: &CheckpointConfig,
            writes: &[(String, Value)],
            task_id: &String,
            task_path: &str,
        ) -> Result<(), CheckpointError> {
            if let Ok(mut ops) = self.operations.lock()
                && let Some(checkpoint_id) = &config.checkpoint_id
            {
                ops.push(format!("writes:{checkpoint_id}"));
            }
            self.inner.put_writes(config, writes, task_id, task_path)
        }
    }

    #[test]
    fn async_durability_flushes_in_checkpoint_then_writes_order() {
        let mut channels = BTreeMap::<String, Box<dyn Channel>>::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let engine = LoopEngine::new(
            channels,
            vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])],
        );
        let saver = RecordingSaver::default();
        let config = LoopConfig::new(CheckpointConfig::new("parity-config-async"))
            .with_durability(DurabilityMode::Async)
            .with_recursion_limit(4);

        let result = engine
            .run_with_input(
                &EchoRunner,
                Some(&saver),
                config,
                LoopInput::Writes(vec![ChannelWrite::new("input", json!("hello"))]),
            )
            .unwrap();

        let saved = saver.get_tuple(&result.checkpoint_config).unwrap().unwrap();
        assert_eq!(
            saved.checkpoint.channel_values.get("output"),
            Some(&json!("hello"))
        );

        let operations = saver.operations();
        for (idx, operation) in operations.iter().enumerate() {
            if let Some(checkpoint_id) = operation.strip_prefix("writes:") {
                let put_seen = operations
                    .iter()
                    .take(idx)
                    .any(|entry| entry == &format!("put:{checkpoint_id}"));
                assert!(
                    put_seen,
                    "writes for checkpoint '{checkpoint_id}' were persisted before checkpoint put"
                );
            }
        }
    }
}
