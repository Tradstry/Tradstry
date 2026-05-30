use async_trait::async_trait;
use serde_json::{Map, Value};
use thiserror::Error;

use super::{ChannelWrite, Command, SendPacket, TaskDescriptor};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MessageEvent {
    pub message: Value,
    pub metadata: Map<String, Value>,
}

impl MessageEvent {
    pub fn new(message: Value) -> Self {
        Self {
            message,
            metadata: Map::new(),
        }
    }

    pub fn with_metadata(mut self, metadata: Map<String, Value>) -> Self {
        self.metadata = metadata;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeReadSelection {
    Single(String),
    Many(Vec<String>),
}

impl From<&str> for RuntimeReadSelection {
    fn from(value: &str) -> Self {
        Self::Single(value.to_owned())
    }
}

impl From<String> for RuntimeReadSelection {
    fn from(value: String) -> Self {
        Self::Single(value)
    }
}

impl From<Vec<String>> for RuntimeReadSelection {
    fn from(value: Vec<String>) -> Self {
        Self::Many(value)
    }
}

impl From<&[String]> for RuntimeReadSelection {
    fn from(value: &[String]) -> Self {
        Self::Many(value.to_vec())
    }
}

#[async_trait]
pub trait RuntimeCapabilities: Send + Sync {
    fn pregel_read(
        &self,
        select: RuntimeReadSelection,
        fresh: bool,
    ) -> Result<Value, NodeExecutionError>;

    fn pregel_send(&self, writes: Vec<ChannelWrite>) -> Result<(), NodeExecutionError>;

    async fn pregel_call(&self, node_name: &str, input: Value)
    -> Result<Value, NodeExecutionError>;
}

#[derive(Debug, Clone, Default)]
pub struct NodeExecutionResult {
    pub writes: Vec<ChannelWrite>,
    pub sends: Vec<SendPacket>,
    pub return_value: Option<Value>,
    pub custom_events: Vec<Value>,
    pub message_events: Vec<MessageEvent>,
    pub command: Option<Command>,
}

impl NodeExecutionResult {
    pub fn with_write(mut self, write: ChannelWrite) -> Self {
        self.writes.push(write);
        self
    }

    pub fn with_send(mut self, send: SendPacket) -> Self {
        self.sends.push(send);
        self
    }

    pub fn with_return_value(mut self, value: Value) -> Self {
        self.return_value = Some(value);
        self
    }

    pub fn with_custom_event(mut self, event: Value) -> Self {
        self.custom_events.push(event);
        self
    }

    pub fn with_message_event(mut self, message: Value) -> Self {
        self.message_events.push(MessageEvent::new(message));
        self
    }

    pub fn with_message_event_with_metadata(
        mut self,
        message: Value,
        metadata: Map<String, Value>,
    ) -> Self {
        self.message_events
            .push(MessageEvent::new(message).with_metadata(metadata));
        self
    }

    pub fn with_command(mut self, command: Command) -> Self {
        self.command = Some(command);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeExecutionErrorKind {
    Retryable,
    Fatal,
    Interrupted,
}

#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct NodeExecutionError {
    pub kind: NodeExecutionErrorKind,
    pub message: String,
    pub error_class: String,
    pub error_code: Option<String>,
}

impl NodeExecutionError {
    pub fn class_for_kind(kind: NodeExecutionErrorKind) -> &'static str {
        match kind {
            NodeExecutionErrorKind::Retryable => "retryable",
            NodeExecutionErrorKind::Fatal => "fatal",
            NodeExecutionErrorKind::Interrupted => "interrupted",
        }
    }

    pub fn new(kind: NodeExecutionErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            error_class: Self::class_for_kind(kind).to_owned(),
            error_code: None,
        }
    }

    pub fn retryable(message: impl Into<String>) -> Self {
        Self::new(NodeExecutionErrorKind::Retryable, message)
    }

    pub fn fatal(message: impl Into<String>) -> Self {
        Self::new(NodeExecutionErrorKind::Fatal, message)
    }

    pub fn interrupted(message: impl Into<String>) -> Self {
        Self::new(NodeExecutionErrorKind::Interrupted, message)
    }

    pub fn with_error_class(mut self, error_class: impl Into<String>) -> Self {
        self.error_class = error_class.into();
        self
    }

    pub fn with_error_code(mut self, error_code: impl Into<String>) -> Self {
        self.error_code = Some(error_code.into());
        self
    }
}

#[derive(Clone)]
pub struct ExecutionContext {
    pub task: std::sync::Arc<TaskDescriptor>,
    pub step: u64,
    pub recursion_limit: u64,
    pub attempt: u32,
    pub resuming: bool,
    pub runtime: Option<std::sync::Arc<dyn RuntimeCapabilities>>,
}

impl ExecutionContext {
    pub fn pregel_read(
        &self,
        select: impl Into<RuntimeReadSelection>,
        fresh: bool,
    ) -> Result<Value, NodeExecutionError> {
        self.runtime_or_err()?.pregel_read(select.into(), fresh)
    }

    pub fn pregel_send(&self, writes: Vec<ChannelWrite>) -> Result<(), NodeExecutionError> {
        self.runtime_or_err()?.pregel_send(writes)
    }

    pub async fn pregel_call(
        &self,
        node_name: impl AsRef<str>,
        input: Value,
    ) -> Result<Value, NodeExecutionError> {
        self.runtime_or_err()?
            .pregel_call(node_name.as_ref(), input)
            .await
    }

    fn runtime_or_err(&self) -> Result<&dyn RuntimeCapabilities, NodeExecutionError> {
        self.runtime
            .as_deref()
            .ok_or_else(|| NodeExecutionError::fatal("runtime capabilities are unavailable"))
    }
}

impl std::fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("task", &self.task)
            .field("step", &self.step)
            .field("recursion_limit", &self.recursion_limit)
            .field("attempt", &self.attempt)
            .field("resuming", &self.resuming)
            .field("runtime", &self.runtime.as_ref().map(|_| "<capabilities>"))
            .finish()
    }
}

#[async_trait]
pub trait NodeExecutor: Send + Sync {
    async fn execute(
        &self,
        input: Value,
        ctx: ExecutionContext,
    ) -> Result<NodeExecutionResult, NodeExecutionError>;
}
