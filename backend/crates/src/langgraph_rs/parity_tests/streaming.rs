#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Map, Value, json};

    use crate::langgraph_rs::{
        core::{
            channels::{Channel, LastValue},
            scheduler::NodeScheduleSpec,
            types::{
                ChannelWrite, ExecutionContext, NodeExecutionError, NodeExecutionResult, StreamMode,
            },
        },
        runtime::{
            io::OutputChannels,
            r#loop::{LoopConfig, LoopEngine, LoopNodeRunner, StreamParityMode},
            streaming::StreamCollector,
        },
    };

    struct StreamingPayloadRunner;

    impl LoopNodeRunner for StreamingPayloadRunner {
        fn execute(
            &self,
            _node_name: &str,
            _input: Value,
            _ctx: ExecutionContext<'_>,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            Ok(NodeExecutionResult::default()
                .with_write(ChannelWrite::new("output", json!("hello")))
                .with_custom_event(json!({"kind": "custom", "value": 1}))
                .with_message_event_with_metadata(
                    json!("message"),
                    Map::from_iter([("source".to_owned(), json!("node"))]),
                ))
        }
    }

    #[test]
    fn streaming_parity_emits_custom_and_messages_payloads() {
        let mut channels = BTreeMap::<String, Box<dyn Channel>>::new();
        channels.insert("input".to_owned(), Box::new(LastValue::new("input")));
        channels.insert("output".to_owned(), Box::new(LastValue::new("output")));

        let node_specs = vec![NodeScheduleSpec::new("streamer", vec!["input".to_owned()])];
        let engine = LoopEngine::new(channels, node_specs);
        let collector = StreamCollector::new();
        let config = LoopConfig::new(
            crate::langgraph_rs::checkpoint::base::CheckpointConfig::new("parity-stream-msg"),
        )
        .with_stream_parity_mode(StreamParityMode::DualPythonCompat)
        .with_parity_stream_modes(vec![StreamMode::Custom, StreamMode::Messages])
        .with_output_channels(OutputChannels::Single("output".to_owned()))
        .with_recursion_limit(3);

        let _result = engine
            .run_with_stream(
                &StreamingPayloadRunner,
                None,
                config,
                vec![ChannelWrite::new("input", json!("go"))],
                Some(&collector),
            )
            .unwrap();

        let events = collector.events();
        assert!(events.iter().any(|event| {
            event.mode == StreamMode::Custom
                && event.payload == json!({"kind": "custom", "value": 1})
        }));

        let message = events
            .iter()
            .find(|event| event.mode == StreamMode::Messages)
            .and_then(|event| event.payload.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(message.len(), 2);
        assert_eq!(message.first(), Some(&json!("message")));

        let metadata = message
            .get(1)
            .and_then(|value| value.as_object())
            .cloned()
            .unwrap_or_default();
        assert_eq!(metadata.get("source"), Some(&json!("node")));
        assert_eq!(metadata.get("node"), Some(&json!("streamer")));
        assert!(metadata.contains_key("task_id"));
        assert!(metadata.contains_key("step"));
        assert!(metadata.contains_key("attempts"));
    }
}
