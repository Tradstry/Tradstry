use crate::langgraph_rs::{
    checkpoint::base::CheckpointSaver,
    core::{
        scheduler::NodeScheduleSpec,
        types::{ChannelWrite, TaskDescriptor},
    },
    runtime::{
        r#loop::{LoopConfig, LoopEngine, LoopError, LoopNodeRunner, LoopRunSummary},
        streaming::RuntimeStream,
    },
};

use super::{GraphDefinition, GraphError};

#[derive(Debug, Clone)]
pub struct CompiledGraph {
    definition: GraphDefinition,
    node_specs: Vec<NodeScheduleSpec>,
    loop_engine: LoopEngine,
}

impl CompiledGraph {
    pub fn from_definition(definition: GraphDefinition) -> Result<Self, GraphError> {
        if definition.channels.is_empty() {
            return Err(GraphError::validation(
                "compiled graph must contain at least one channel",
            ));
        }
        if definition.nodes.is_empty() {
            return Err(GraphError::validation(
                "compiled graph must contain at least one node",
            ));
        }

        let node_specs = definition.to_scheduler_specs();
        let loop_engine = LoopEngine::new(definition.channels.clone(), node_specs.clone());

        Ok(Self {
            definition,
            node_specs,
            loop_engine,
        })
    }

    pub fn definition(&self) -> &GraphDefinition {
        &self.definition
    }

    pub fn node_specs(&self) -> &[NodeScheduleSpec] {
        &self.node_specs
    }

    pub fn planned_edges(&self) -> &[super::GraphEdgeSpec] {
        &self.definition.edges
    }

    pub async fn run(
        &self,
        runner: std::sync::Arc<dyn LoopNodeRunner>,
        saver: Option<&dyn CheckpointSaver>,
        config: LoopConfig,
        input_writes: Vec<ChannelWrite>,
    ) -> Result<LoopRunSummary, LoopError> {
        self.loop_engine
            .run(runner, saver, config, input_writes)
            .await
    }

    pub async fn run_with_stream(
        &self,
        runner: std::sync::Arc<dyn LoopNodeRunner>,
        saver: Option<&dyn CheckpointSaver>,
        config: LoopConfig,
        input_writes: Vec<ChannelWrite>,
        stream: Option<std::sync::Arc<dyn RuntimeStream>>,
    ) -> Result<LoopRunSummary, LoopError> {
        self.loop_engine
            .run_with_stream(runner, saver, config, input_writes, stream)
            .await
    }

    pub fn task_for_node(&self, node_name: &str, step: u64) -> Option<TaskDescriptor> {
        self.node_specs
            .iter()
            .find(|spec| spec.name == node_name)
            .map(|spec| {
                TaskDescriptor::new(
                    format!("pull:{step}:{}", spec.name),
                    spec.name.clone(),
                    vec![
                        crate::langgraph_rs::core::types::TaskPathPart::Name("pull".to_owned()),
                        crate::langgraph_rs::core::types::TaskPathPart::Name(spec.name.clone()),
                        crate::langgraph_rs::core::types::TaskPathPart::Index(step),
                    ],
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use crate::langgraph_rs::{
        checkpoint::base::CheckpointConfig,
        core::{
            channels::{Channel, LastValue},
            graph::{GraphBuilder, GraphError},
            types::{ChannelWrite, ExecutionContext, NodeExecutionError, NodeExecutionResult},
        },
        runtime::r#loop::{LoopConfig, LoopNodeRunner, LoopStatus},
    };

    struct EchoRunner;

    #[async_trait::async_trait]
    impl LoopNodeRunner for EchoRunner {
        async fn execute(
            &self,
            _node_name: &str,
            input: Value,
            _ctx: ExecutionContext,
        ) -> Result<NodeExecutionResult, NodeExecutionError> {
            let value = input
                .get("input")
                .cloned()
                .ok_or_else(|| NodeExecutionError::fatal("missing input"))?;
            Ok(NodeExecutionResult::default().with_write(ChannelWrite::new("output", value)))
        }
    }

    #[tokio::test]
    async fn compiles_and_executes_graph_with_runtime_loop() {
        let mut builder = GraphBuilder::new();
        builder
            .add_channel("input", Box::new(LastValue::new("input")))
            .unwrap()
            .add_channel("output", Box::new(LastValue::new("output")))
            .unwrap()
            .add_node_with_triggers("echo", vec!["input".to_owned()])
            .unwrap();

        let graph = builder.compile().unwrap();
        let result = graph
            .run(
                std::sync::Arc::new(EchoRunner) as std::sync::Arc<dyn LoopNodeRunner>,
                None,
                LoopConfig::new(CheckpointConfig::new("graph-thread")).with_recursion_limit(3),
                vec![ChannelWrite::new("input", json!("hello"))],
            )
            .await
            .unwrap();

        assert_eq!(result.status, LoopStatus::Done);
        assert_eq!(
            result.checkpoint.channel_values.get("output"),
            Some(&json!("hello"))
        );
    }

    #[test]
    fn rejects_empty_definition_during_compile() {
        let definition = super::GraphDefinition {
            channels: BTreeMap::<String, Box<dyn Channel>>::new(),
            nodes: BTreeMap::new(),
            edges: Vec::new(),
        };

        let err = super::CompiledGraph::from_definition(definition).unwrap_err();
        assert!(matches!(err, GraphError::Validation { .. }));
    }
}
