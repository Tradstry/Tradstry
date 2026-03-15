use std::collections::BTreeSet;

use serde_json::Value;

use crate::langgraph_rs::{
    checkpoint::base::{Checkpoint, CheckpointConfig},
    core::{
        constants::TASKS,
        managed::{ManagedValueRef, ManagedValueRegistry, builtin_managed_values},
        types::{
            ChannelName, ChannelWrite, Command, ExecutionContext, NodeExecutionError,
            NodeExecutionResult, StreamMode, TaskDescriptor,
        },
    },
    runtime::{interrupts::InterruptSelector, io::OutputChannels, runner::RetryPolicy},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DurabilityMode {
    Sync,
    #[default]
    Async,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamParityMode {
    #[default]
    RuntimeEventsOnly,
    DualPythonCompat,
}

#[derive(Debug, Clone)]
pub enum LoopInput {
    Writes(Vec<ChannelWrite>),
    Command(Command),
    None,
}

#[derive(Clone)]
pub struct LoopConfig {
    pub checkpoint_config: CheckpointConfig,
    pub recursion_limit: u64,
    pub retry_limit: u32,
    pub retry_policies: Vec<RetryPolicy>,
    pub max_concurrency: usize,
    pub tasks_channel: String,
    pub durability: DurabilityMode,
    pub stream_parity_mode: StreamParityMode,
    pub parity_stream_modes: Option<Vec<StreamMode>>,
    pub output_channels: Option<OutputChannels>,
    pub interrupt_before: InterruptSelector,
    pub interrupt_after: InterruptSelector,
    pub managed_values: ManagedValueRegistry,
}

impl std::fmt::Debug for LoopConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let managed_keys = self.managed_values.keys().cloned().collect::<Vec<_>>();
        f.debug_struct("LoopConfig")
            .field("checkpoint_config", &self.checkpoint_config)
            .field("recursion_limit", &self.recursion_limit)
            .field("retry_limit", &self.retry_limit)
            .field("retry_policies", &self.retry_policies)
            .field("max_concurrency", &self.max_concurrency)
            .field("tasks_channel", &self.tasks_channel)
            .field("durability", &self.durability)
            .field("stream_parity_mode", &self.stream_parity_mode)
            .field("parity_stream_modes", &self.parity_stream_modes)
            .field("output_channels", &self.output_channels)
            .field("interrupt_before", &self.interrupt_before)
            .field("interrupt_after", &self.interrupt_after)
            .field("managed_values", &managed_keys)
            .finish()
    }
}

impl LoopConfig {
    pub fn new(checkpoint_config: CheckpointConfig) -> Self {
        Self {
            checkpoint_config,
            recursion_limit: 25,
            retry_limit: 0,
            retry_policies: Vec::new(),
            max_concurrency: std::thread::available_parallelism()
                .map(|value| value.get())
                .unwrap_or(1)
                .max(1),
            tasks_channel: TASKS.to_owned(),
            durability: DurabilityMode::default(),
            stream_parity_mode: StreamParityMode::default(),
            parity_stream_modes: None,
            output_channels: None,
            interrupt_before: InterruptSelector::none(),
            interrupt_after: InterruptSelector::none(),
            managed_values: builtin_managed_values(),
        }
    }

    pub fn with_recursion_limit(mut self, recursion_limit: u64) -> Self {
        self.recursion_limit = recursion_limit;
        self
    }

    pub fn with_tasks_channel(mut self, tasks_channel: impl Into<String>) -> Self {
        self.tasks_channel = tasks_channel.into();
        self
    }

    pub fn with_retry_limit(mut self, retry_limit: u32) -> Self {
        self.retry_limit = retry_limit;
        self
    }

    pub fn with_retry_policies(mut self, retry_policies: Vec<RetryPolicy>) -> Self {
        self.retry_policies = retry_policies;
        self
    }

    pub fn with_max_concurrency(mut self, max_concurrency: usize) -> Self {
        self.max_concurrency = max_concurrency.max(1);
        self
    }

    pub fn with_durability(mut self, durability: DurabilityMode) -> Self {
        self.durability = durability;
        self
    }

    pub fn with_stream_parity_mode(mut self, stream_parity_mode: StreamParityMode) -> Self {
        self.stream_parity_mode = stream_parity_mode;
        self
    }

    pub fn with_parity_stream_modes(mut self, parity_stream_modes: Vec<StreamMode>) -> Self {
        self.parity_stream_modes = Some(parity_stream_modes);
        self
    }

    pub fn with_output_channels(mut self, output_channels: impl Into<OutputChannels>) -> Self {
        self.output_channels = Some(output_channels.into());
        self
    }

    pub fn with_interrupt_before(mut self, interrupt_before: InterruptSelector) -> Self {
        self.interrupt_before = interrupt_before;
        self
    }

    pub fn with_interrupt_after(mut self, interrupt_after: InterruptSelector) -> Self {
        self.interrupt_after = interrupt_after;
        self
    }

    pub fn with_managed_values(mut self, managed_values: ManagedValueRegistry) -> Self {
        self.managed_values = managed_values;
        self
    }

    pub fn with_managed_value(
        mut self,
        name: impl Into<String>,
        managed_value: ManagedValueRef,
    ) -> Self {
        self.managed_values.insert(name.into(), managed_value);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopStatus {
    Done,
    OutOfSteps,
    InterruptedBefore,
    InterruptedAfter,
}

#[derive(Debug, Clone)]
pub struct LoopTaskReport {
    pub step: u64,
    pub task: TaskDescriptor,
    pub attempts: u32,
    pub writes: Vec<ChannelWrite>,
}

#[derive(Debug, Clone)]
pub struct LoopRunSummary {
    pub status: LoopStatus,
    pub steps_executed: u64,
    pub tasks_executed: usize,
    pub final_output: Option<Value>,
    pub checkpoint: Checkpoint,
    pub checkpoint_config: CheckpointConfig,
    pub updated_channels: BTreeSet<ChannelName>,
    pub task_reports: Vec<LoopTaskReport>,
}

pub trait LoopNodeRunner: Send + Sync {
    fn execute(
        &self,
        node_name: &str,
        input: Value,
        ctx: ExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, NodeExecutionError>;
}
