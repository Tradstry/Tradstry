#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use crate::langgraph_rs::{
        core::{
            channels::{Channel, LastValue, Topic},
            scheduler::NodeScheduleSpec,
            types::{ChannelWrite, ExecutionContext, NodeExecutionError, NodeExecutionResult, SendPacket, TaskPathStr},
        },
        runtime::r#loop::{LoopConfig, LoopEngine, LoopNodeRunner, LoopStatus},
    };

    struct LargeCaseRunner;

    impl LoopNodeRunner for LargeCaseRunner {
        fn execute(
            &self,
            node_name: &str,
            input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            match node_name {
                "producer" => Ok(NodeExecutionResult::default()
                    .with_send(SendPacket::new("worker", json!({"v": 1})))
                    .with_send(SendPacket::new("worker", json!({"v": 2})))),
                "worker" => Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                    "items",
                    input
                        .get("v")
                        .cloned()
                        .ok_or_else(|| NodeExecutionError::fatal("missing v"))?,
                ))),
                "finalize" => {
                    let values = input
                        .get("items")
                        .and_then(|value| value.as_array())
                        .cloned()
                        .unwrap_or_default();
                    Ok(NodeExecutionResult::default()
                        .with_write(ChannelWrite::new("output", json!(values.len()))))
                }
                other => Err(NodeExecutionError::fatal(format!(
                    "unexpected node '{other}'"
                ))),
            }
        }
    }

    fn run_case() -> crate::langgraph_rs::runtime::r#loop::LoopRunSummary {
        let mut channels = BTreeMap::<String, Box<dyn Channel>>::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("items".to_owned(), Box::new(Topic::new("items", true)));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![
            NodeScheduleSpec::new("producer", vec!["input".to_owned()]),
            NodeScheduleSpec::new("worker", Vec::<String>::new()),
            NodeScheduleSpec::new("finalize", vec!["items".to_owned()]),
        ];
        let engine = LoopEngine::new(channels, node_specs);
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("parity-large-case"),
        )
        .with_recursion_limit(8);

        engine
            .run(
                &LargeCaseRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("go"))],
            )
            .unwrap()
    }

    #[test]
    fn large_case_push_pull_flow_is_stable_and_deterministic() {
        let first = run_case();
        assert_eq!(first.status, LoopStatus::Done);
        assert_eq!(first.checkpoint.channel_values.get("output"), Some(&json!(2)));
        assert_eq!(first.tasks_executed, 4);

        let second = run_case();
        let mut first_tasks = first
            .task_reports
            .iter()
            .map(|report| (report.task.id.clone(), report.task.path.to_path_string()))
            .collect::<Vec<_>>();
        let mut second_tasks = second
            .task_reports
            .iter()
            .map(|report| (report.task.id.clone(), report.task.path.to_path_string()))
            .collect::<Vec<_>>();
        first_tasks.sort();
        second_tasks.sort();
        assert_eq!(first_tasks, second_tasks);
    }
}
