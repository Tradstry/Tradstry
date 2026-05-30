//! # LangGraph Rust
//!
//! A Rust implementation of LangGraph for building stateful, graph-based AI agent workflows.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use langgraph::prelude::*;
//! use serde_json::json;
//!
//! // Build a simple graph
//! let mut builder = GraphBuilder::new();
//! builder
//!     .add_channel("input", Box::new(LastValue::new("input")))
//!     .unwrap()
//!     .add_channel("output", Box::new(LastValue::new("output")))
//!     .unwrap()
//!     .add_node_with_triggers("process", vec!["input".to_owned()])
//!     .unwrap();
//!
//! let graph = builder.compile().unwrap();
//!
//! // Create an adapter to define what nodes do
//! let runner: std::sync::Arc<dyn LoopNodeRunner> = std::sync::Arc::new(
//!     AdapterRunner::default()
//!         .with_node("process", FnAdapterNode::new(|input: serde_json::Value, _ctx| async move {
//!             let value = input.get("input").cloned().unwrap();
//!             Ok(NodeExecutionResult::default()
//!                 .with_write(ChannelWrite::new("output", value)))
//!         }))
//!         .unwrap(),
//! );
//!
//! // Run the graph
//! let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
//!     graph.run(
//!         runner,
//!         None,
//!         LoopConfig::new(CheckpointConfig::new("thread-1")),
//!         vec![ChannelWrite::new("input", json!("Hello!"))],
//!     ).await
//! }).unwrap();
//! ```
//!
//! ## Modules
//!
//! ### Core
//! - [`core`] - Graph building, channels, and scheduling
//!   - [`core::channels`] - Channel types for state management
//!   - [`core::graph`] - Graph builder and compiled graph
//!   - [`core::scheduler`] - Task planning and write application
//!   - [`core::types`] - Core type definitions
//!   - [`core::constants`] - Constant values
//!
//! ### Runtime
//! - [`runtime`] - Execution engine and loop
//!   - [`runtime::loop`] - Main execution loop engine
//!   - [`runtime::runner`] - Task runner with retry logic
//!   - [`runtime::streaming`] - Event streaming
//!   - [`runtime::interrupts`] - Interrupt handling
//!   - [`runtime::io`] - Input/output utilities
//!
//! ### Persistence
//! - [`checkpoint`] - State persistence
//!   - [`checkpoint::base`] - Base types and traits
//!   - [`checkpoint::memory`] - In-memory checkpoint storage
//!   - [`checkpoint::sqlite`] - SQLite checkpoint storage
//!   - [`checkpoint::postgres`] - PostgreSQL checkpoint storage
//!
//! ### Caching
//! - [`cache`] - Node result caching
//!   - [`cache::base`] - Base cache types and traits
//!   - [`cache::memory`] - In-memory cache
//!   - [`cache::sqlite`] - SQLite cache
//!   - [`cache::postgres`] - PostgreSQL cache
//!
//! ### Storage
//! - [`store`] - Long-term key-value storage
//!   - [`store::base`] - Base store types and traits
//!   - [`store::memory`] - In-memory store
//!   - [`store::sqlite`] - SQLite store
//!   - [`store::postgres`] - PostgreSQL store
//!
//! ### Adapters
//! - [`adapters`] - Integration with AI frameworks
//!   - [`adapters::langchain_rust`] - LangChain Rust integration
//!   - [`adapters::rig`] - Rig framework integration
//!
//! ## Prelude
//!
//! The [`prelude`] module re-exports commonly used types for convenience:
//!
//! ```rust
//! use langgraph::prelude::*;
//! ```

pub mod langgraph_rs;

// Re-export all main modules at the top level
pub use langgraph_rs::{adapters, cache, checkpoint, core, runtime, store};

/// Commonly used types and traits.
///
/// Import this module to get quick access to the most frequently used items:
///
/// ```rust
/// use langgraph::prelude::*;
/// ```
pub mod prelude {
    // Core - Graph Building
    pub use crate::core::graph::subgraph::{InputMappingFn, OutputMappingFn, SubgraphConfig};
    pub use crate::core::graph::{
        BranchTarget, CompiledGraph, CompiledStateGraph, GraphBuilder, GraphDefinition,
        GraphEdgeKind, GraphEdgeSpec, GraphError, GraphNodeSpec, ManagedValueKind, StateField,
        StateFieldKind, StateGraph, StateNodeOptions, StateSchema,
    };

    // Core - All Channel Types
    pub use crate::core::channels::{
        AnyValue, BinaryOperatorAggregate, BinaryOperatorFn, Channel, ChannelError, EphemeralValue,
        LastValue, LastValueAfterFinish, NamedBarrierValue, NamedBarrierValueAfterFinish, Topic,
        UntrackedValue,
    };

    // Core - All Types
    pub use crate::core::types::{
        ChannelName, ChannelWrite, Command, CommandGraph, CommandUpdate, ExecutionContext,
        GotoTarget, Interrupt, InterruptId, MessageEvent, NodeExecutionError,
        NodeExecutionErrorKind, NodeExecutionResult, NodeExecutor, NodeName, OVERWRITE_MARKER,
        Overwrite, RuntimeCapabilities, RuntimeReadSelection, SendPacket, StreamEvent, StreamMode,
        TaskDescriptor, TaskId, TaskPath, TaskPathPart, TaskPathStr, extract_overwrite_value,
        interrupt_id_from_namespace, is_overwrite_value,
    };

    // Core - Scheduler
    pub use crate::core::scheduler::{
        NodeScheduleSpec, PlannedTask, PlannedTaskKind, RESERVED_WRITE_CHANNELS,
        SchedulerApplySummary, SchedulerCheckpoint, SchedulerError, TaskWrites, TriggerToNodes,
        apply_writes, build_trigger_to_nodes, is_node_triggered, is_reserved_write_channel,
        plan_next_tasks, plan_next_tasks_detailed,
    };

    // Core - Managed Values
    pub use crate::core::managed::{
        BuiltInManagedValue, FnManagedValue, ManagedValue, ManagedValueContext,
    };

    // Runtime - Loop
    pub use crate::runtime::r#loop::{
        DurabilityMode, LoopConfig, LoopEngine, LoopError, LoopInput, LoopNodeRunner,
        LoopRunSummary, LoopStatus, LoopTaskReport, StreamParityMode,
    };

    // Runtime - Runner
    pub use crate::runtime::runner::{
        RetryOn, RetryPolicy, RunnerConfig, RunnerError, TaskExecutionRequest, TaskExecutionResult,
        TaskRunner, TaskRuntimeState,
    };

    // Runtime - Streaming
    pub use crate::runtime::streaming::{RuntimeEvent, RuntimeStream, StreamCollector};

    // Runtime - IO
    pub use crate::runtime::io::{
        DebugPayloadType, IoMapError, OutputChannels, OutputWriteGate, PendingWriteTuple,
        TaskOutputWrites, map_checkpoint_payload, map_command, map_command_to_writes,
        map_debug_wrapper, map_input_writes, map_output_updates, map_output_values,
        map_task_payload, map_task_result_payload,
    };

    // Runtime - Interrupts
    pub use crate::runtime::interrupts::{InterruptSelector, interrupted_nodes, should_interrupt};

    // Checkpoint - Base
    pub use crate::checkpoint::base::{
        CHECKPOINT_FORMAT_VERSION, ChannelVersions, Checkpoint, CheckpointCompatibilityPolicy,
        CheckpointConfig, CheckpointError, CheckpointId, CheckpointIdStrategy, CheckpointMetadata,
        CheckpointParents, CheckpointReadCompatibility, CheckpointSaver, CheckpointSource,
        CheckpointTuple, CheckpointWireFormat, ListCheckpointsQuery, MetadataMap, PendingWrite,
        PruneStrategy, VersionsSeen, copy_checkpoint, create_checkpoint,
        create_checkpoint_with_config, deserialize_checkpoint_json,
        effective_checkpoint_compatibility, empty_checkpoint, empty_checkpoint_with_config,
        get_checkpoint_id, get_checkpoint_metadata, get_serializable_checkpoint_metadata,
        is_excluded_metadata_key, next_checkpoint_id, next_checkpoint_id_with_strategy,
        next_uuid6_checkpoint_id, normalize_checkpoint_for_read,
        now_timestamp_string as checkpoint_now_timestamp_string, project_checkpoint_for_storage,
        write_idx_for_channel,
    };

    // Checkpoint - Backends
    pub use crate::checkpoint::memory::InMemorySaver;
    pub use crate::checkpoint::postgres::PostgresSaver;
    pub use crate::checkpoint::sqlite::SqliteSaver;

    // Cache - Base
    pub use crate::cache::base::{
        Cache, CacheError, CacheItem, CacheKey, CacheMetadata, CacheNamespace, CacheSetOptions,
        NodeResultCacheEnvelope, cache_value_to_node_result,
        namespace_matches_prefix as cache_namespace_matches_prefix, node_result_to_cache_value,
        now_unix_millis,
    };

    // Cache - Backends
    pub use crate::cache::memory::InMemoryCache;
    pub use crate::cache::postgres::PostgresCache;
    pub use crate::cache::sqlite::SqliteCache;

    // Store - Base
    pub use crate::store::base::{
        EmbeddingVector, NamespacePath, Store, StoreError, StoreItem, StoreListQuery,
        StoreMetadata, StoreScoredItem, StoreSearchQuery, StoreVectorQuery, VectorMetric,
        namespace_matches_prefix as store_namespace_matches_prefix,
        now_timestamp_string as store_now_timestamp_string, vector_score,
    };

    // Store - Backends
    pub use crate::store::memory::InMemoryStore;
    pub use crate::store::postgres::PostgresStore;
    pub use crate::store::sqlite::SqliteStore;

    // Adapters
    pub use crate::adapters::{
        AdapterContext, AdapterError, AdapterNode, AdapterRegistry, AdapterRunner, FnAdapterNode,
    };

    // Adapter Providers
    pub use crate::adapters::langchain_rust::{LangChainAdapterErrorMapper, LangChainNodeAdapter};
    pub use crate::adapters::rig::{RigAdapterErrorMapper, RigNodeAdapter};
}
