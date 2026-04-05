use std::collections::{BTreeMap, BTreeSet};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

use crate::langgraph_rs::{
    checkpoint::base::CheckpointSaver,
    core::{
        channels::EphemeralValue,
        constants::{END, START, TASKS},
        managed::ManagedValueRegistry,
        scheduler::NodeScheduleSpec,
        types::{
            ChannelWrite, CommandGraph, ExecutionContext, NodeExecutionError, NodeExecutionResult,
            RuntimeReadSelection,
        },
    },
    runtime::{
        io::map_command_to_writes,
        r#loop::{LoopConfig, LoopEngine, LoopInput, LoopNodeRunner, LoopRunSummary},
        streaming::RuntimeStream,
    },
};

use super::{
    BranchTarget, GraphError, StateGraph, StateSchema,
    state::{ConditionalEdgeDef, StateNodeAction},
};

#[derive(Clone)]
struct StateGraphNodeRunner {
    handlers: BTreeMap<String, StateNodeAction>,
    direct_targets: BTreeMap<String, Vec<String>>,
    conditional_edges: BTreeMap<String, Vec<ConditionalEdgeDef>>,
    known_nodes: BTreeSet<String>,
    state_channels: BTreeSet<String>,
    readable_state_names: Vec<String>,
    tasks_channel: String,
}

impl StateGraphNodeRunner {
    fn apply_control_flow(
        &self,
        node_name: &str,
        input: &Value,
        ctx: ExecutionContext<'_>,
        output: &mut NodeExecutionResult,
    ) -> Result<(), NodeExecutionError> {
        if output
            .command
            .as_ref()
            .is_some_and(|command| command.graph == CommandGraph::Parent)
        {
            return Ok(());
        }

        let mut control_writes = Vec::<ChannelWrite>::new();

        if let Some(command) = output.command.take() {
            let mapped = map_command_to_writes(&command)
                .map_err(|_| NodeExecutionError::fatal("failed to map node command writes"))?;
            control_writes.extend(mapped);
        }

        if let Some(direct_targets) = self.direct_targets.get(node_name) {
            for target in direct_targets {
                if target == END {
                    continue;
                }
                control_writes.push(ChannelWrite::new(
                    branch_channel(target),
                    Value::String(START.to_owned()),
                ));
            }
        }

        if let Some(edges) = self.conditional_edges.get(node_name) {
            let post_state = self.build_post_state(input, output, &control_writes, ctx);
            for edge in edges {
                let targets = (edge.path)(post_state.clone(), output).map_err(|err| {
                    NodeExecutionError::fatal(format!(
                        "conditional branch evaluation failed: {err}"
                    ))
                })?;
                for target in targets {
                    if let Some(write) =
                        self.resolve_branch_target(&edge.source, target, edge.path_map.as_ref())?
                    {
                        control_writes.push(write);
                    }
                }
            }
        }

        let control_writes = dedupe_writes(control_writes);
        output.writes.extend(control_writes);
        Ok(())
    }

    fn build_post_state(
        &self,
        input: &Value,
        output: &NodeExecutionResult,
        command_writes: &[ChannelWrite],
        ctx: ExecutionContext<'_>,
    ) -> Value {
        let mut state_map = match ctx.pregel_read(
            RuntimeReadSelection::Many(self.readable_state_names.clone()),
            true,
        ) {
            Ok(Value::Object(map)) => map,
            _ => match input {
                Value::Object(map) => map.clone(),
                _ => Map::new(),
            },
        };

        for write in output.writes.iter().chain(command_writes.iter()) {
            if self.state_channels.contains(&write.channel) {
                state_map.insert(write.channel.clone(), write.value.clone());
            }
        }
        Value::Object(state_map)
    }

    fn resolve_branch_target(
        &self,
        source: &str,
        target: BranchTarget,
        path_map: Option<&BTreeMap<String, String>>,
    ) -> Result<Option<ChannelWrite>, NodeExecutionError> {
        let target = match target {
            BranchTarget::Symbol(symbol) => {
                let Some(mapped) = path_map.and_then(|map| map.get(&symbol)).cloned() else {
                    return Err(NodeExecutionError::fatal(format!(
                        "{}",
                        GraphError::UnknownBranchTarget {
                            from_node: source.to_owned(),
                            target: symbol,
                        }
                    )));
                };
                if mapped == END {
                    BranchTarget::End
                } else {
                    BranchTarget::Node(mapped)
                }
            }
            target => target,
        };

        match target {
            BranchTarget::End => Ok(None),
            BranchTarget::Node(node_name) => {
                if node_name == END {
                    return Ok(None);
                }
                if !self.known_nodes.contains(&node_name) {
                    return Err(NodeExecutionError::fatal(format!(
                        "{}",
                        GraphError::UnknownBranchTarget {
                            from_node: source.to_owned(),
                            target: node_name,
                        }
                    )));
                }
                Ok(Some(ChannelWrite::new(
                    branch_channel(&node_name),
                    Value::String(START.to_owned()),
                )))
            }
            BranchTarget::Send(packet) => {
                if packet.node == END {
                    return Err(NodeExecutionError::fatal(format!(
                        "{}",
                        GraphError::InvalidConditionalPathResult {
                            from_node: source.to_owned(),
                            message: "cannot send a packet to END".to_owned(),
                        }
                    )));
                }
                if !self.known_nodes.contains(&packet.node) {
                    return Err(NodeExecutionError::fatal(format!(
                        "{}",
                        GraphError::UnknownBranchTarget {
                            from_node: source.to_owned(),
                            target: packet.node.clone(),
                        }
                    )));
                }
                let value = serde_json::to_value(packet).map_err(|err| {
                    NodeExecutionError::fatal(format!("failed to serialize SendPacket: {err}"))
                })?;
                Ok(Some(ChannelWrite::new(self.tasks_channel.clone(), value)))
            }
            BranchTarget::Symbol(_) => unreachable!("symbols are resolved above"),
        }
    }
}

impl LoopNodeRunner for StateGraphNodeRunner {
    fn execute(
        &self,
        node_name: &str,
        input: Value,
        ctx: ExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, NodeExecutionError> {
        let Some(handler) = self.handlers.get(node_name) else {
            return Err(NodeExecutionError::fatal(format!(
                "state graph node '{node_name}' is not registered"
            )));
        };
        let mut output = (handler)(input.clone(), ctx)?;
        self.apply_control_flow(node_name, &input, ctx, &mut output)?;
        Ok(output)
    }
}

#[derive(Clone)]
pub struct CompiledStateGraph<State = Value, Context = (), Input = State, Output = State> {
    state_schema: StateSchema,
    input_schema: StateSchema,
    output_schema: StateSchema,
    loop_engine: LoopEngine,
    node_runner: StateGraphNodeRunner,
    managed_values: ManagedValueRegistry,
    start_direct_targets: Vec<String>,
    start_conditional_edges: Vec<ConditionalEdgeDef>,
    _marker: std::marker::PhantomData<(State, Context, Input, Output)>,
}

impl<State, Context, Input, Output> std::fmt::Debug
    for CompiledStateGraph<State, Context, Input, Output>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledStateGraph")
            .field("state_schema", &self.state_schema)
            .field("input_schema", &self.input_schema)
            .field("output_schema", &self.output_schema)
            .field("start_direct_targets", &self.start_direct_targets)
            .field("start_conditional_edges", &self.start_conditional_edges)
            .finish_non_exhaustive()
    }
}

impl<State, Context, Input, Output> CompiledStateGraph<State, Context, Input, Output>
where
    Input: Serialize,
    Output: DeserializeOwned,
{
    pub(crate) fn from_state_graph(
        graph: &StateGraph<State, Context, Input, Output>,
    ) -> Result<Self, GraphError> {
        let mut channels = graph.state_schema.channels();
        for node_name in graph.nodes.keys() {
            let key = branch_channel(node_name);
            if channels.contains_key(&key) {
                return Err(GraphError::InvalidSchemaField {
                    field: key,
                    message: "branch channel conflicts with an existing state channel".to_owned(),
                });
            }
            channels.insert(key.clone(), Box::new(EphemeralValue::new(key, false)));
        }

        let mut node_specs = Vec::<NodeScheduleSpec>::new();
        let mut handlers = BTreeMap::<String, StateNodeAction>::new();
        let mut known_nodes = BTreeSet::<String>::new();

        for (node_name, node) in &graph.nodes {
            known_nodes.insert(node_name.clone());
            handlers.insert(node_name.clone(), node.action.clone());
            let read_channels = node
                .options
                .input_schema
                .as_ref()
                .map(StateSchema::readable_names)
                .unwrap_or_else(|| graph.state_schema.readable_names());
            let mut spec =
                NodeScheduleSpec::new(node_name.clone(), vec![branch_channel(node_name)])
                    .with_read_channels(read_channels);
            if let Some(retry_policy) = &node.options.retry_policy {
                spec = spec.with_retry_policy(retry_policy.clone());
            }
            if let Some(cache_enabled) = node.options.cache_enabled {
                spec = spec.with_cache_enabled(cache_enabled);
            }
            node_specs.push(spec);
        }

        let mut direct_targets = BTreeMap::<String, Vec<String>>::new();
        let mut start_direct_targets = Vec::<String>::new();
        for (from, to) in &graph.edges {
            if from == START {
                if to != END {
                    start_direct_targets.push(to.clone());
                }
                continue;
            }
            if to == END {
                continue;
            }
            direct_targets
                .entry(from.clone())
                .or_default()
                .push(to.clone());
        }

        let mut conditional_edges = BTreeMap::<String, Vec<ConditionalEdgeDef>>::new();
        let mut start_conditional_edges = Vec::<ConditionalEdgeDef>::new();
        for edge in &graph.conditional_edges {
            if edge.source == START {
                start_conditional_edges.push(edge.clone());
                continue;
            }
            conditional_edges
                .entry(edge.source.clone())
                .or_default()
                .push(edge.clone());
        }

        let loop_engine = LoopEngine::new(channels, node_specs);
        let node_runner = StateGraphNodeRunner {
            handlers,
            direct_targets,
            conditional_edges,
            known_nodes,
            state_channels: graph.state_schema.channel_names().into_iter().collect(),
            readable_state_names: graph.state_schema.readable_names(),
            tasks_channel: TASKS.to_owned(),
        };

        Ok(Self {
            state_schema: graph.state_schema.clone(),
            input_schema: graph.input_schema.clone(),
            output_schema: graph.output_schema.clone(),
            managed_values: graph.state_schema.managed_values(),
            loop_engine,
            node_runner,
            start_direct_targets,
            start_conditional_edges,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn run_raw(
        &self,
        saver: Option<&dyn CheckpointSaver>,
        config: LoopConfig,
        input: Input,
    ) -> Result<LoopRunSummary, GraphError> {
        self.run_raw_with_stream(saver, config, input, None)
    }

    pub fn run_raw_with_stream(
        &self,
        saver: Option<&dyn CheckpointSaver>,
        mut config: LoopConfig,
        input: Input,
        stream: Option<&dyn RuntimeStream>,
    ) -> Result<LoopRunSummary, GraphError> {
        let (mut writes, state_view) = self.map_input_writes(input)?;
        writes.extend(self.start_direct_targets.iter().map(|target| {
            ChannelWrite::new(branch_channel(target), Value::String(START.to_owned()))
        }));
        writes.extend(self.evaluate_start_conditionals(state_view)?);
        writes = dedupe_writes(writes);

        let mut managed = config.managed_values.clone();
        for (name, managed_value) in &self.managed_values {
            managed.insert(name.clone(), managed_value.clone());
        }
        config = config.with_managed_values(managed);

        self.loop_engine
            .run_with_stream_and_input(
                &self.node_runner,
                saver,
                config,
                LoopInput::Writes(writes),
                stream,
            )
            .map_err(|e| GraphError::from(Box::new(e)))
    }

    pub fn invoke(
        &self,
        saver: Option<&dyn CheckpointSaver>,
        config: LoopConfig,
        input: Input,
    ) -> Result<Output, GraphError> {
        self.invoke_with_stream(saver, config, input, None)
    }

    pub fn invoke_with_stream(
        &self,
        saver: Option<&dyn CheckpointSaver>,
        config: LoopConfig,
        input: Input,
        stream: Option<&dyn RuntimeStream>,
    ) -> Result<Output, GraphError> {
        let summary = self.run_raw_with_stream(saver, config, input, stream)?;
        self.decode_output(&summary)
    }

    fn map_input_writes(&self, input: Input) -> Result<(Vec<ChannelWrite>, Value), GraphError> {
        let raw = serde_json::to_value(input).map_err(|err| GraphError::InvalidSchemaField {
            field: "__input__".to_owned(),
            message: format!("failed to serialize input: {err}"),
        })?;

        let input_channels = self.input_schema.channel_names();
        if input_channels.is_empty() {
            return Ok((Vec::new(), Value::Object(Map::new())));
        }

        match raw {
            Value::Object(map) => {
                let mut writes = Vec::new();
                let mut state_view = Map::new();
                for channel in input_channels {
                    if let Some(value) = map.get(&channel).cloned() {
                        writes.push(ChannelWrite::new(channel.clone(), value.clone()));
                        state_view.insert(channel, value);
                    }
                }
                Ok((writes, Value::Object(state_view)))
            }
            other => {
                if input_channels.len() != 1 {
                    return Err(GraphError::InvalidSchemaField {
                        field: "__input__".to_owned(),
                        message: "non-object input requires exactly one input channel".to_owned(),
                    });
                }
                let channel = input_channels.first().cloned().unwrap_or_default();
                Ok((
                    vec![ChannelWrite::new(channel.clone(), other.clone())],
                    Value::Object(Map::from_iter([(channel, other)])),
                ))
            }
        }
    }

    fn evaluate_start_conditionals(
        &self,
        state_view: Value,
    ) -> Result<Vec<ChannelWrite>, GraphError> {
        let mut writes = Vec::<ChannelWrite>::new();
        for edge in &self.start_conditional_edges {
            let targets = (edge.path)(state_view.clone(), &NodeExecutionResult::default())?;
            for target in targets {
                if let Some(write) = resolve_branch_target_for_start(
                    target,
                    edge.path_map.as_ref(),
                    &self.node_runner.known_nodes,
                )? {
                    writes.push(write);
                }
            }
        }
        Ok(writes)
    }

    fn decode_output(&self, summary: &LoopRunSummary) -> Result<Output, GraphError> {
        let output_channels = self.output_schema.channel_names();
        if output_channels.is_empty() {
            return serde_json::from_value(Value::Null).map_err(|err| {
                GraphError::InvalidSchemaField {
                    field: "__output__".to_owned(),
                    message: format!("failed to decode output: {err}"),
                }
            });
        }

        let output_value = if output_channels.len() == 1 {
            summary
                .checkpoint
                .channel_values
                .get(&output_channels[0])
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            let mut map = Map::new();
            for channel in output_channels {
                if let Some(value) = summary.checkpoint.channel_values.get(&channel).cloned() {
                    map.insert(channel, value);
                }
            }
            Value::Object(map)
        };

        serde_json::from_value(output_value).map_err(|err| GraphError::InvalidSchemaField {
            field: "__output__".to_owned(),
            message: format!("failed to decode output: {err}"),
        })
    }
}

fn dedupe_writes(writes: Vec<ChannelWrite>) -> Vec<ChannelWrite> {
    let mut seen = BTreeSet::<(String, String)>::new();
    let mut deduped = Vec::<ChannelWrite>::new();
    for write in writes {
        let value_key = serde_json::to_string(&write.value).unwrap_or_else(|_| "null".to_owned());
        let key = (write.channel.clone(), value_key);
        if seen.insert(key) {
            deduped.push(write);
        }
    }
    deduped
}

fn resolve_branch_target_for_start(
    target: BranchTarget,
    path_map: Option<&BTreeMap<String, String>>,
    known_nodes: &BTreeSet<String>,
) -> Result<Option<ChannelWrite>, GraphError> {
    let target = match target {
        BranchTarget::Symbol(symbol) => {
            let Some(mapped) = path_map.and_then(|map| map.get(&symbol)).cloned() else {
                return Err(GraphError::UnknownBranchTarget {
                    from_node: START.to_owned(),
                    target: symbol,
                });
            };
            if mapped == END {
                BranchTarget::End
            } else {
                BranchTarget::Node(mapped)
            }
        }
        target => target,
    };

    match target {
        BranchTarget::End => Ok(None),
        BranchTarget::Node(node_name) => {
            if node_name == END {
                return Ok(None);
            }
            if !known_nodes.contains(&node_name) {
                return Err(GraphError::UnknownBranchTarget {
                    from_node: START.to_owned(),
                    target: node_name,
                });
            }
            Ok(Some(ChannelWrite::new(
                branch_channel(&node_name),
                Value::String(START.to_owned()),
            )))
        }
        BranchTarget::Send(packet) => {
            if packet.node == END {
                return Err(GraphError::InvalidConditionalPathResult {
                    from_node: START.to_owned(),
                    message: "cannot send a packet to END".to_owned(),
                });
            }
            if !known_nodes.contains(&packet.node) {
                return Err(GraphError::UnknownBranchTarget {
                    from_node: START.to_owned(),
                    target: packet.node,
                });
            }
            let value = serde_json::to_value(packet).map_err(|err| {
                GraphError::InvalidConditionalPathResult {
                    from_node: START.to_owned(),
                    message: format!("failed to serialize SendPacket: {err}"),
                }
            })?;
            Ok(Some(ChannelWrite::new(TASKS, value)))
        }
        BranchTarget::Symbol(_) => unreachable!("symbols are resolved above"),
    }
}

fn branch_channel(node_name: &str) -> String {
    format!("branch:to:{node_name}")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use crate::langgraph_rs::{
        checkpoint::base::CheckpointConfig,
        core::{
            graph::{BranchTarget, StateGraph, StateNodeOptions, StateSchema},
            types::{ChannelWrite, Command, GotoTarget, NodeExecutionResult},
        },
        runtime::r#loop::LoopConfig,
    };

    #[test]
    fn direct_edge_routes_to_next_node() {
        let mut graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("input")
                .unwrap()
                .with_last_value("mid")
                .unwrap()
                .with_last_value("output")
                .unwrap(),
        );
        graph
            .add_node("a", |input: Value, _ctx| {
                Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                    "mid",
                    input.get("input").cloned().unwrap_or(Value::Null),
                )))
            })
            .unwrap()
            .add_node("b", |input: Value, _ctx| {
                Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                    "output",
                    input.get("mid").cloned().unwrap_or(Value::Null),
                )))
            })
            .unwrap()
            .set_entry_point("a")
            .unwrap()
            .add_edge("a", "b")
            .unwrap()
            .set_finish_point("b")
            .unwrap();

        let compiled = graph.compile().unwrap();
        let summary = compiled
            .run_raw(
                None,
                LoopConfig::new(CheckpointConfig::new("state-graph-direct")),
                json!({"input": "hello"}),
            )
            .unwrap();

        assert_eq!(
            summary.checkpoint.channel_values.get("output"),
            Some(&json!("hello"))
        );
    }

    #[test]
    fn conditional_path_map_routes_symbol_to_target() {
        let mut graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("input")
                .unwrap()
                .with_last_value("output")
                .unwrap(),
        );
        graph
            .add_node("router", |input: Value, _ctx| {
                Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                    "output",
                    input.get("input").cloned().unwrap_or(Value::Null),
                )))
            })
            .unwrap()
            .add_node("target", |input: Value, _ctx| {
                Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                    "output",
                    input.get("output").cloned().unwrap_or(Value::Null),
                )))
            })
            .unwrap()
            .set_entry_point("router")
            .unwrap()
            .add_conditional_edges(
                "router",
                |_state, _result| Ok(vec![BranchTarget::Symbol("yes".to_owned())]),
                Some(BTreeMap::from([("yes".to_owned(), "target".to_owned())])),
            )
            .unwrap()
            .set_finish_point("target")
            .unwrap();

        let compiled = graph.compile().unwrap();
        let summary = compiled
            .run_raw(
                None,
                LoopConfig::new(CheckpointConfig::new("state-graph-branch")),
                json!({"input": "routed"}),
            )
            .unwrap();

        assert_eq!(
            summary.checkpoint.channel_values.get("output"),
            Some(&json!("routed"))
        );
    }

    #[test]
    fn command_and_conditional_routes_coexist_in_same_step() {
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
                LoopConfig::new(CheckpointConfig::new("state-graph-command-branch")),
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
    fn managed_values_are_injected_into_node_input() {
        let mut graph = StateGraph::<Value, (), Value, Value>::new(
            StateSchema::new()
                .with_last_value("input")
                .unwrap()
                .with_last_value("output")
                .unwrap()
                .with_managed("is_last_step", super::super::ManagedValueKind::IsLastStep)
                .unwrap(),
        );
        graph
            .add_node_with_options(
                "check",
                |input: Value, _ctx| {
                    Ok(NodeExecutionResult::default().with_write(ChannelWrite::new(
                        "output",
                        input.get("is_last_step").cloned().unwrap_or(Value::Null),
                    )))
                },
                StateNodeOptions::new().with_input_schema(
                    StateSchema::new()
                        .with_last_value("input")
                        .unwrap()
                        .with_managed("is_last_step", super::super::ManagedValueKind::IsLastStep)
                        .unwrap(),
                ),
            )
            .unwrap()
            .set_entry_point("check")
            .unwrap()
            .set_finish_point("check")
            .unwrap();

        let compiled = graph.compile().unwrap();
        let summary = compiled
            .run_raw(
                None,
                LoopConfig::new(CheckpointConfig::new("state-graph-managed"))
                    .with_recursion_limit(1),
                json!({"input": "x"}),
            )
            .unwrap();

        assert!(matches!(
            summary.checkpoint.channel_values.get("output"),
            Some(Value::Bool(_))
        ));
        assert!(
            !summary
                .checkpoint
                .channel_values
                .contains_key("is_last_step")
        );
    }
}
