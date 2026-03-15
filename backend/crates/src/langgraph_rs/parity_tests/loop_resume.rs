#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use crate::langgraph_rs::{
        checkpoint::{
            base::{CheckpointMetadata, CheckpointSaver, empty_checkpoint},
            memory::InMemorySaver,
        },
        core::{
            channels::{Channel, LastValue},
            constants::PULL,
            scheduler::NodeScheduleSpec,
            types::{ExecutionContext, NodeExecutionError, NodeExecutionResult},
        },
        runtime::r#loop::{LoopConfig, LoopEngine, LoopInput, LoopNodeRunner, LoopStatus},
    };

    struct PanicRunner;

    impl LoopNodeRunner for PanicRunner {
        fn execute(
            &self,
            _node_name: &str,
            _input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            panic!("node runner should not execute when task writes are replayed from checkpoint");
        }
    }

    #[test]
    fn loop_parity_replays_task_pending_writes_without_reexecuting_node() {
        let mut channels = BTreeMap::<String, Box<dyn Channel>>::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("echo", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let saver = InMemorySaver::new();

        let mut checkpoint = empty_checkpoint();
        checkpoint
            .channel_values
            .insert("input".to_owned(), json!("from_checkpoint"));
        checkpoint.channel_versions.insert("input".to_owned(), 1);

        let base_config =
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("parity-loop-replay");
        let mut metadata = CheckpointMetadata::default();
        metadata.step = Some(0);
        let checkpoint_config = saver
            .put(&base_config, checkpoint, metadata, BTreeMap::new())
            .unwrap();

        let replay_task_id = format!("{PULL}:1:echo:input");
        saver
            .put_writes(
                &checkpoint_config,
                &[("output".to_owned(), json!("from_replay"))],
                &replay_task_id,
                "pull::echo::0000000001",
            )
            .unwrap();

        let result = engine
            .run_with_input(
                &PanicRunner,
                Some(&saver),
                LoopConfig::new(base_config).with_recursion_limit(3),
                LoopInput::None,
            )
            .unwrap();

        assert_eq!(result.status, LoopStatus::Done);
        assert_eq!(result.tasks_executed, 0);
        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("from_replay"))
        );
        assert!(
            result
                .task_reports
                .iter()
                .any(|report| report.task.id == replay_task_id && report.attempts == 0)
        );
    }
}
