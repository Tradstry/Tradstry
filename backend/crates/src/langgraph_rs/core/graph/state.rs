use std::{collections::BTreeMap, marker::PhantomData, sync::Arc};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::langgraph_rs::{
    checkpoint::base::{CheckpointConfig, CheckpointSaver},
    core::{
        constants::{END, START},
        types::{ExecutionContext, NodeExecutionError, NodeExecutionResult, SendPacket},
    },
    runtime::{r#loop::LoopConfig, runner::RetryPolicy},
};

use super::{CompiledStateGraph, GraphError, StateSchema, subgraph::SubgraphConfig};

pub type StateNodeAction = Arc<
    dyn for<'a> Fn(Value, ExecutionContext<'a>) -> Result<NodeExecutionResult, NodeExecutionError>
        + Send
        + Sync,
>;

pub type ConditionalPathFn =
    Arc<dyn Fn(Value, &NodeExecutionResult) -> Result<Vec<BranchTarget>, GraphError> + Send + Sync>;

#[derive(Debug, Clone, PartialEq)]
pub enum BranchTarget {
    Node(String),
    Send(SendPacket),
    End,
    Symbol(String),
}

#[derive(Debug, Clone, Default)]
pub struct StateNodeOptions {
    pub input_schema: Option<StateSchema>,
    pub retry_policy: Option<Vec<RetryPolicy>>,
    pub cache_enabled: Option<bool>,
    pub destinations: Option<BTreeMap<String, String>>,
}

impl StateNodeOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_input_schema(mut self, input_schema: StateSchema) -> Self {
        self.input_schema = Some(input_schema);
        self
    }

    pub fn with_retry_policy(mut self, retry_policy: Vec<RetryPolicy>) -> Self {
        self.retry_policy = Some(retry_policy);
        self
    }

    pub fn with_cache_enabled(mut self, cache_enabled: bool) -> Self {
        self.cache_enabled = Some(cache_enabled);
        self
    }

    pub fn with_destinations(
        mut self,
        destinations: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        self.destinations = Some(destinations.into_iter().collect());
        self
    }
}

#[derive(Clone)]
pub(crate) struct StateNodeDef {
    pub(crate) action: StateNodeAction,
    pub(crate) options: StateNodeOptions,
}

impl std::fmt::Debug for StateNodeDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateNodeDef")
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub(crate) struct ConditionalEdgeDef {
    pub(crate) source: String,
    pub(crate) path: ConditionalPathFn,
    pub(crate) path_map: Option<BTreeMap<String, String>>,
}

impl std::fmt::Debug for ConditionalEdgeDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConditionalEdgeDef")
            .field("source", &self.source)
            .field("path_map", &self.path_map)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct StateGraph<State = Value, Context = (), Input = State, Output = State> {
    pub(crate) state_schema: StateSchema,
    pub(crate) input_schema: StateSchema,
    pub(crate) output_schema: StateSchema,
    pub(crate) nodes: BTreeMap<String, StateNodeDef>,
    pub(crate) edges: Vec<(String, String)>,
    pub(crate) conditional_edges: Vec<ConditionalEdgeDef>,
    pub(crate) _marker: PhantomData<(State, Context, Input, Output)>,
}

impl<State, Context, Input, Output> std::fmt::Debug for StateGraph<State, Context, Input, Output> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateGraph")
            .field("state_schema", &self.state_schema)
            .field("input_schema", &self.input_schema)
            .field("output_schema", &self.output_schema)
            .field("nodes", &self.nodes.keys().collect::<Vec<_>>())
            .field("edges", &self.edges)
            .field("conditional_edges", &self.conditional_edges)
            .finish()
    }
}

impl<State, Context, Input, Output> StateGraph<State, Context, Input, Output> {
    pub fn new(state_schema: StateSchema) -> Self {
        let io_schema = state_schema.without_managed();
        Self {
            input_schema: io_schema.clone(),
            output_schema: io_schema,
            state_schema,
            nodes: BTreeMap::new(),
            edges: Vec::new(),
            conditional_edges: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn with_input_schema(mut self, input_schema: StateSchema) -> Self {
        self.input_schema = input_schema;
        self
    }

    pub fn with_output_schema(mut self, output_schema: StateSchema) -> Self {
        self.output_schema = output_schema;
        self
    }

    pub fn add_node<F>(
        &mut self,
        node_name: impl Into<String>,
        action: F,
    ) -> Result<&mut Self, GraphError>
    where
        F: for<'a> Fn(
                Value,
                ExecutionContext<'a>,
            ) -> Result<NodeExecutionResult, NodeExecutionError>
            + Send
            + Sync
            + 'static,
    {
        self.add_node_with_options(node_name, action, StateNodeOptions::new())
    }

    pub fn add_node_with_options<F>(
        &mut self,
        node_name: impl Into<String>,
        action: F,
        options: StateNodeOptions,
    ) -> Result<&mut Self, GraphError>
    where
        F: for<'a> Fn(
                Value,
                ExecutionContext<'a>,
            ) -> Result<NodeExecutionResult, NodeExecutionError>
            + Send
            + Sync
            + 'static,
    {
        let node_name = node_name.into();
        validate_node_name(&node_name)?;
        if self.nodes.contains_key(&node_name) {
            return Err(GraphError::DuplicateNode { node: node_name });
        }
        self.nodes.insert(
            node_name,
            StateNodeDef {
                action: Arc::new(action),
                options,
            },
        );
        Ok(self)
    }

    pub fn add_subgraph(
        &mut self,
        node_name: impl Into<String>,
        child: CompiledStateGraph,
        config: SubgraphConfig,
        saver: Option<Arc<dyn CheckpointSaver>>,
    ) -> Result<&mut Self, GraphError> {
        let node_name_str = node_name.into();
        validate_node_name(&node_name_str)?;
        if self.nodes.contains_key(&node_name_str) {
            return Err(GraphError::DuplicateNode {
                node: node_name_str,
            });
        }

        let child = Arc::new(child);
        let input_mapping = config.input_mapping;
        let output_mapping = config.output_mapping;
        let checkpoint_ns = config
            .checkpoint_ns
            .unwrap_or_else(|| format!("subgraph:{}", node_name_str));
        let recursion_limit = config.recursion_limit.unwrap_or(25);

        let action: StateNodeAction =
            Arc::new(move |parent_state: Value, ctx: ExecutionContext<'_>| {
                let child_input = (input_mapping)(parent_state);

                let parent_thread_id = ctx.task.id.split(':').next().unwrap_or(&ctx.task.id);
                let child_thread_id = format!("{}:{}", parent_thread_id, checkpoint_ns);
                let child_config = LoopConfig::new(CheckpointConfig::new(&child_thread_id))
                    .with_recursion_limit(recursion_limit);

                let summary = child
                    .run_raw(
                        saver.as_ref().map(|s| s.as_ref()),
                        child_config,
                        child_input,
                    )
                    .map_err(|e| NodeExecutionError::fatal(format!("Subgraph error: {e:?}")))?;

                let final_state =
                    serde_json::to_value(&summary.checkpoint.channel_values).unwrap_or(Value::Null);

                let writes = (output_mapping)(final_state);

                let mut result = NodeExecutionResult::default();
                for write in writes {
                    result = result.with_write(write);
                }
                Ok(result)
            });

        self.nodes.insert(
            node_name_str,
            StateNodeDef {
                action,
                options: StateNodeOptions::new(),
            },
        );
        Ok(self)
    }

    pub fn add_edge(
        &mut self,
        from_node: impl Into<String>,
        to_node: impl Into<String>,
    ) -> Result<&mut Self, GraphError> {
        let from_node = from_node.into();
        let to_node = to_node.into();
        if from_node == END {
            return Err(GraphError::validation("END cannot be a start node"));
        }
        if to_node == START {
            return Err(GraphError::validation("START cannot be an end node"));
        }
        if from_node != START && !self.nodes.contains_key(&from_node) {
            return Err(GraphError::unknown_node(from_node));
        }
        if to_node != END && !self.nodes.contains_key(&to_node) {
            return Err(GraphError::unknown_node(to_node));
        }
        self.edges.push((from_node, to_node));
        Ok(self)
    }

    pub fn add_conditional_edges<F>(
        &mut self,
        source: impl Into<String>,
        path: F,
        path_map: Option<BTreeMap<String, String>>,
    ) -> Result<&mut Self, GraphError>
    where
        F: Fn(Value, &NodeExecutionResult) -> Result<Vec<BranchTarget>, GraphError>
            + Send
            + Sync
            + 'static,
    {
        let source = source.into();
        if source != START && !self.nodes.contains_key(&source) {
            return Err(GraphError::unknown_node(source));
        }
        if let Some(map) = &path_map {
            for target in map.values() {
                if target != END && !self.nodes.contains_key(target) {
                    return Err(GraphError::UnknownBranchTarget {
                        from_node: source.clone(),
                        target: target.clone(),
                    });
                }
            }
        }
        self.conditional_edges.push(ConditionalEdgeDef {
            source,
            path: Arc::new(path),
            path_map,
        });
        Ok(self)
    }

    pub fn set_entry_point(
        &mut self,
        node_name: impl Into<String>,
    ) -> Result<&mut Self, GraphError> {
        self.add_edge(START, node_name.into())
    }

    pub fn set_conditional_entry_point<F>(
        &mut self,
        path: F,
        path_map: Option<BTreeMap<String, String>>,
    ) -> Result<&mut Self, GraphError>
    where
        F: Fn(Value, &NodeExecutionResult) -> Result<Vec<BranchTarget>, GraphError>
            + Send
            + Sync
            + 'static,
    {
        self.add_conditional_edges(START, path, path_map)
    }

    pub fn set_finish_point(
        &mut self,
        node_name: impl Into<String>,
    ) -> Result<&mut Self, GraphError> {
        self.add_edge(node_name.into(), END)
    }

    pub fn compile(&self) -> Result<CompiledStateGraph<State, Context, Input, Output>, GraphError>
    where
        Input: Serialize,
        Output: DeserializeOwned,
    {
        self.input_schema.validate_no_managed("input")?;
        self.output_schema.validate_no_managed("output")?;
        self.validate()?;
        CompiledStateGraph::from_state_graph(self)
    }

    fn validate(&self) -> Result<(), GraphError> {
        if self.nodes.is_empty() {
            return Err(GraphError::validation(
                "state graph must contain at least one node",
            ));
        }

        let has_direct_entry = self.edges.iter().any(|(from, _)| from == START);
        let has_conditional_entry = self
            .conditional_edges
            .iter()
            .any(|edge| edge.source == START);
        if !has_direct_entry && !has_conditional_entry {
            return Err(GraphError::MissingEntryPoint);
        }

        for (from, to) in &self.edges {
            if from != START && !self.nodes.contains_key(from) {
                return Err(GraphError::unknown_node(from.clone()));
            }
            if to != END && !self.nodes.contains_key(to) {
                return Err(GraphError::unknown_node(to.clone()));
            }
        }

        for edge in &self.conditional_edges {
            if edge.source != START && !self.nodes.contains_key(&edge.source) {
                return Err(GraphError::unknown_node(edge.source.clone()));
            }
            if let Some(path_map) = &edge.path_map {
                for mapped in path_map.values() {
                    if mapped != END && !self.nodes.contains_key(mapped) {
                        return Err(GraphError::UnknownBranchTarget {
                            from_node: edge.source.clone(),
                            target: mapped.clone(),
                        });
                    }
                }
            }
        }

        Ok(())
    }
}

fn validate_node_name(name: &str) -> Result<(), GraphError> {
    if name == START || name == END {
        return Err(GraphError::validation(format!(
            "node name '{name}' is reserved"
        )));
    }
    if name.trim().is_empty() {
        return Err(GraphError::validation("node name cannot be empty"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::langgraph_rs::core::{
        graph::{BranchTarget, GraphError, ManagedValueKind, StateGraph, StateSchema},
        types::{ChannelWrite, NodeExecutionResult},
    };

    #[test]
    fn requires_entry_point() {
        let mut graph =
            StateGraph::<serde_json::Value>::new(StateSchema::new().with_last_value("x").unwrap());
        graph
            .add_node("a", |_input, _ctx| Ok(NodeExecutionResult::default()))
            .unwrap();

        let err = graph.compile().unwrap_err();
        assert!(matches!(err, GraphError::MissingEntryPoint));
    }

    #[test]
    fn rejects_unknown_conditional_path_map_target() {
        let mut graph =
            StateGraph::<serde_json::Value>::new(StateSchema::new().with_last_value("x").unwrap());
        graph
            .add_node("a", |_input, _ctx| Ok(NodeExecutionResult::default()))
            .unwrap();

        let err = graph
            .add_conditional_edges(
                "a",
                |_state, _result| Ok(vec![BranchTarget::Symbol("yes".to_owned())]),
                Some(BTreeMap::from([("yes".to_owned(), "missing".to_owned())])),
            )
            .unwrap_err();
        assert!(format!("{err}").contains("unknown branch target"));
    }

    #[test]
    fn disallows_managed_values_in_input_output_schema() {
        let schema = StateSchema::new()
            .with_last_value("x")
            .unwrap()
            .with_managed("is_last_step", ManagedValueKind::IsLastStep)
            .unwrap();
        let mut graph =
            StateGraph::<serde_json::Value>::new(StateSchema::new().with_last_value("x").unwrap())
                .with_input_schema(schema);
        graph
            .add_node("a", |_input, _ctx| Ok(NodeExecutionResult::default()))
            .unwrap();
        graph.set_entry_point("a").unwrap();
        let err = graph.compile().unwrap_err();
        assert!(format!("{err}").contains("cannot contain managed fields"));
    }

    #[test]
    fn compiles_branch_target_send_values() {
        let mut graph = StateGraph::<serde_json::Value>::new(
            StateSchema::new()
                .with_last_value("x")
                .unwrap()
                .with_last_value("out")
                .unwrap(),
        );
        graph
            .add_node("a", |_input, _ctx| {
                Ok(
                    NodeExecutionResult::default()
                        .with_write(ChannelWrite::new("out", json!("ok"))),
                )
            })
            .unwrap()
            .add_node("worker", |_input, _ctx| Ok(NodeExecutionResult::default()))
            .unwrap();
        graph
            .set_entry_point("a")
            .unwrap()
            .add_conditional_edges(
                "a",
                |_state, _result| {
                    Ok(vec![BranchTarget::Send(
                        crate::langgraph_rs::core::types::SendPacket::new(
                            "worker",
                            json!({"x": 1}),
                        ),
                    )])
                },
                None,
            )
            .unwrap();

        let compiled = graph.compile();
        assert!(compiled.is_ok());
    }
}
