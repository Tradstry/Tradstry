#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::{Value, json};

    use crate::langgraph_rs::{
        checkpoint::base::CheckpointConfig,
        checkpoint::memory::InMemorySaver,
        core::{
            graph::{StateGraph, StateSchema, subgraph::SubgraphConfig},
            types::{ChannelWrite, NodeExecutionResult},
        },
        runtime::r#loop::LoopConfig,
    };

    #[tokio::test]
    async fn basic_subgraph_execution() {
        // Build child graph: reads "query", writes "processed: {query}" to "result"
        let mut child_graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("query")
                .unwrap()
                .with_last_value("result")
                .unwrap(),
        );
        child_graph
            .add_node("process", |input: Value, _ctx| async move {
                let query = input
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let processed = format!("processed: {}", query);
                Ok(NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("result", json!(processed))))
            })
            .unwrap()
            .set_entry_point("process")
            .unwrap()
            .set_finish_point("process")
            .unwrap();

        let compiled_child = child_graph.compile().unwrap();

        // Build parent graph with subgraph node
        let mut parent_graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("input")
                .unwrap()
                .with_last_value("output")
                .unwrap(),
        );

        let config = SubgraphConfig {
            input_mapping: Arc::new(|parent_state: Value| {
                let input_val = parent_state.get("input").cloned().unwrap_or(Value::Null);
                json!({ "query": input_val })
            }),
            output_mapping: Arc::new(|child_state: Value| {
                let result = child_state.get("result").cloned().unwrap_or(Value::Null);
                vec![ChannelWrite::new("output", result)]
            }),
            checkpoint_ns: None,
            recursion_limit: None,
        };

        parent_graph
            .add_subgraph("child", compiled_child, config, None)
            .unwrap()
            .set_entry_point("child")
            .unwrap()
            .set_finish_point("child")
            .unwrap();

        let compiled_parent = parent_graph.compile().unwrap();
        let summary = compiled_parent
            .run_raw(
                None,
                LoopConfig::new(CheckpointConfig::new("subgraph-basic")),
                json!({"input": "hello"}),
            )
            .await
            .unwrap();

        assert_eq!(
            summary.checkpoint.channel_values.get("output"),
            Some(&json!("processed: hello"))
        );
    }

    #[tokio::test]
    async fn subgraph_with_multiple_steps() {
        // Child graph with 2 nodes: step1 appends "-step1", step2 appends "-step2"
        let mut child_graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("data")
                .unwrap()
                .with_last_value("result")
                .unwrap(),
        );
        child_graph
            .add_node("step1", |input: Value, _ctx| async move {
                let data = input
                    .get("data")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let new_data = format!("{}-step1", data);
                Ok(NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("data", json!(new_data))))
            })
            .unwrap()
            .add_node("step2", |input: Value, _ctx| async move {
                let data = input
                    .get("data")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let result = format!("{}-step2", data);
                Ok(NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("result", json!(result))))
            })
            .unwrap()
            .set_entry_point("step1")
            .unwrap()
            .add_edge("step1", "step2")
            .unwrap()
            .set_finish_point("step2")
            .unwrap();

        let compiled_child = child_graph.compile().unwrap();

        let mut parent_graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("input")
                .unwrap()
                .with_last_value("output")
                .unwrap(),
        );

        let config = SubgraphConfig {
            input_mapping: Arc::new(|parent_state: Value| {
                let input_val = parent_state.get("input").cloned().unwrap_or(Value::Null);
                json!({ "data": input_val })
            }),
            output_mapping: Arc::new(|child_state: Value| {
                let result = child_state.get("result").cloned().unwrap_or(Value::Null);
                vec![ChannelWrite::new("output", result)]
            }),
            checkpoint_ns: None,
            recursion_limit: None,
        };

        parent_graph
            .add_subgraph("child", compiled_child, config, None)
            .unwrap()
            .set_entry_point("child")
            .unwrap()
            .set_finish_point("child")
            .unwrap();

        let compiled_parent = parent_graph.compile().unwrap();
        let summary = compiled_parent
            .run_raw(
                None,
                LoopConfig::new(CheckpointConfig::new("subgraph-multi-step")),
                json!({"input": "start"}),
            )
            .await
            .unwrap();

        assert_eq!(
            summary.checkpoint.channel_values.get("output"),
            Some(&json!("start-step1-step2"))
        );
    }

    #[tokio::test]
    async fn subgraph_without_checkpoint_saver() {
        // Same as basic test but with explicit None savers for both child and parent
        let mut child_graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("query")
                .unwrap()
                .with_last_value("result")
                .unwrap(),
        );
        child_graph
            .add_node("process", |input: Value, _ctx| async move {
                let query = input
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let processed = format!("processed: {}", query);
                Ok(NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("result", json!(processed))))
            })
            .unwrap()
            .set_entry_point("process")
            .unwrap()
            .set_finish_point("process")
            .unwrap();

        let compiled_child = child_graph.compile().unwrap();

        let mut parent_graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("input")
                .unwrap()
                .with_last_value("output")
                .unwrap(),
        );

        let config = SubgraphConfig {
            input_mapping: Arc::new(|parent_state: Value| {
                let input_val = parent_state.get("input").cloned().unwrap_or(Value::Null);
                json!({ "query": input_val })
            }),
            output_mapping: Arc::new(|child_state: Value| {
                let result = child_state.get("result").cloned().unwrap_or(Value::Null);
                vec![ChannelWrite::new("output", result)]
            }),
            checkpoint_ns: None,
            recursion_limit: None,
        };

        // Pass None as saver to add_subgraph
        parent_graph
            .add_subgraph("child", compiled_child, config, None)
            .unwrap()
            .set_entry_point("child")
            .unwrap()
            .set_finish_point("child")
            .unwrap();

        let compiled_parent = parent_graph.compile().unwrap();
        // Run parent also with None saver
        let summary = compiled_parent
            .run_raw(
                None,
                LoopConfig::new(CheckpointConfig::new("subgraph-no-saver")),
                json!({"input": "hello"}),
            )
            .await
            .unwrap();

        assert_eq!(
            summary.checkpoint.channel_values.get("output"),
            Some(&json!("processed: hello"))
        );
    }

    #[tokio::test]
    async fn subgraph_custom_checkpoint_namespace() {
        // Same as basic test but with custom checkpoint_ns and recursion_limit
        let mut child_graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("query")
                .unwrap()
                .with_last_value("result")
                .unwrap(),
        );
        child_graph
            .add_node("process", |input: Value, _ctx| async move {
                let query = input
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let processed = format!("processed: {}", query);
                Ok(NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("result", json!(processed))))
            })
            .unwrap()
            .set_entry_point("process")
            .unwrap()
            .set_finish_point("process")
            .unwrap();

        let compiled_child = child_graph.compile().unwrap();

        let mut parent_graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("input")
                .unwrap()
                .with_last_value("output")
                .unwrap(),
        );

        let config = SubgraphConfig {
            input_mapping: Arc::new(|parent_state: Value| {
                let input_val = parent_state.get("input").cloned().unwrap_or(Value::Null);
                json!({ "query": input_val })
            }),
            output_mapping: Arc::new(|child_state: Value| {
                let result = child_state.get("result").cloned().unwrap_or(Value::Null);
                vec![ChannelWrite::new("output", result)]
            }),
            checkpoint_ns: Some("my-custom-ns".to_string()),
            recursion_limit: Some(10),
        };

        let saver: Arc<dyn crate::langgraph_rs::checkpoint::base::CheckpointSaver> =
            Arc::new(InMemorySaver::new());

        parent_graph
            .add_subgraph("child", compiled_child, config, Some(saver.clone()))
            .unwrap()
            .set_entry_point("child")
            .unwrap()
            .set_finish_point("child")
            .unwrap();

        let compiled_parent = parent_graph.compile().unwrap();
        let summary = compiled_parent
            .run_raw(
                Some(saver.as_ref()),
                LoopConfig::new(CheckpointConfig::new("subgraph-custom-ns")),
                json!({"input": "hello"}),
            )
            .await
            .unwrap();

        assert_eq!(
            summary.checkpoint.channel_values.get("output"),
            Some(&json!("processed: hello"))
        );
    }
}
