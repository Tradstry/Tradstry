#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use crate::langgraph_rs::{
        checkpoint::base::CheckpointConfig,
        core::{
            graph::{BranchTarget, StateGraph, StateSchema},
            types::{ChannelWrite, Command, GotoTarget, NodeExecutionResult},
        },
        runtime::r#loop::LoopConfig,
    };

    #[test]
    fn state_graph_parity_supports_command_and_conditional_routing_together() {
        let mut graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("input")
                .unwrap()
                .with_last_value("out_b")
                .unwrap()
                .with_last_value("out_c")
                .unwrap(),
        );
        graph
            .add_node("a", |_input: Value, _ctx| {
                Ok(NodeExecutionResult::default()
                    .with_command(Command::new().with_goto(GotoTarget::Node("b".to_owned()))))
            })
            .unwrap()
            .add_node("b", |_input: Value, _ctx| {
                Ok(NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("out_b", json!(true))))
            })
            .unwrap()
            .add_node("c", |_input: Value, _ctx| {
                Ok(NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("out_c", json!(true))))
            })
            .unwrap()
            .set_entry_point("a")
            .unwrap()
            .add_conditional_edges(
                "a",
                |_state, _result| Ok(vec![BranchTarget::Node("c".to_owned())]),
                None,
            )
            .unwrap();

        let compiled = graph.compile().unwrap();
        let summary = compiled
            .run_raw(
                None,
                LoopConfig::new(CheckpointConfig::new("parity-state-graph")),
                json!({"input": 1}),
            )
            .unwrap();

        assert_eq!(
            summary.checkpoint.channel_values.get("out_b"),
            Some(&json!(true))
        );
        assert_eq!(
            summary.checkpoint.channel_values.get("out_c"),
            Some(&json!(true))
        );
    }

    #[test]
    fn state_graph_conditional_entry_symbol_path_map_routes_correctly() {
        let mut graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("input")
                .unwrap()
                .with_last_value("output")
                .unwrap(),
        );
        graph
            .add_node("yes", |input: Value, _ctx| {
                Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                    "output",
                    input.get("input").cloned().unwrap_or(Value::Null),
                )))
            })
            .unwrap()
            .add_node("no", |_input: Value, _ctx| {
                Ok(NodeExecutionResult::default()
                    .with_write(ChannelWrite::new("output", json!("no"))))
            })
            .unwrap()
            .set_conditional_entry_point(
                |state, _result| {
                    if state.get("input") == Some(&json!("go")) {
                        Ok(vec![BranchTarget::Symbol("yes".to_owned())])
                    } else {
                        Ok(vec![BranchTarget::Symbol("no".to_owned())])
                    }
                },
                Some(BTreeMap::from([
                    ("yes".to_owned(), "yes".to_owned()),
                    ("no".to_owned(), "no".to_owned()),
                ])),
            )
            .unwrap()
            .set_finish_point("yes")
            .unwrap()
            .set_finish_point("no")
            .unwrap();

        let compiled = graph.compile().unwrap();
        let summary = compiled
            .run_raw(
                None,
                LoopConfig::new(CheckpointConfig::new("parity-state-entry")),
                json!({"input": "go"}),
            )
            .unwrap();

        assert_eq!(
            summary.checkpoint.channel_values.get("output"),
            Some(&json!("go"))
        );
    }
}
