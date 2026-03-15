## LangGraph Rust -> Detailed Architecture Explanation
The langgraph_rs module is a Rust rewrite of LangGraph's core runtime, implementing a Pregel-style superstep execution engine for stateful, graph-based AI agent workflows.

Here's how it works:

# Architecture Overview
The system is organized into 6 main modules:
1. Core - Framework-agnostic graph kernel (determinstic logic)
2. Runtime - Execution engine (Pregel-style loop)
3. Checkpoint - State persistence layer
4. Adapters - Integration with external frameworks (rig, langchain-rust)
5. Cache - Node result memoization 
6. Store - Long-term key-valye storage with vector search 

# 1. Core Module - The Graph Kernel
The code module defines the fundamental graph abstractions without any provider-specific code.

- Channels (core/channels)
Channels are the state containers in the graph. They manage how data flows between nodes:
"Channels are mailboxes with different rules on how they handle messages that nodes can read and write to for coordinating nodes in a graph."

**Basic Value Channels:**
* **LastValue**: Stores the most recent value, accepts one update per step
* **LastValueAfterFinish**: Same but only exposes value after `finish()` is called
* **AnyValue**: Similar to LastValue but clears on empty update (accepts last value from multiple updates)
* **EphemeralValue**: Clears on empty update (temporary state), supports guard mode for single-update enforcement

**Aggregation Channels:**
* **BinaryOperatorAggregate**: Reduces multiple values using a custom binary operator (e.g., sum, append)
  - Built-in: `add_numeric()` for numeric addition
  - Custom: Pass any `Fn(&Value, &Value) -> Result<Value>` for custom reduction logic
  - Supports `Overwrite` special value to replace accumulated state
* **Topic**: Append/accumulate semantics for multi-value publication with flatten behavior

**Synchronization Channels:**
* **NamedBarrierValue**: Fan-in gate - only available when all named inputs are received
* **NamedBarrierValueAfterFinish**: Same fan-in gate plus explicit `finish()` gating

**Special Channels:**
* **UntrackedValue**: Stores value but never persists to checkpoint (for temporary/sensitive data)
  - `guard=true`: Enforces single value per step
  - `guard=false`: Accepts multiple values, keeps last

Each channel implements the `Channel` trait with operations:
* `get()` - retrieve current value
* `update(values)` - apply new values, returns true if state changed 
* `consume()` - mark as consumed (for trigger channels)
* `finish()` - finalize state for the step
* `checkpoint() / from_checkpoint()` - serialization for persistence

**Special Features:**
* **Overwrite Mechanism**: Channels that support aggregation (like BinaryOperatorAggregate) can receive an `Overwrite` value that replaces the current state instead of aggregating:
  ```rust
  // Using Overwrite struct
  Overwrite::new(json!(42)).into()
  
  // Using dict form
  json!({"__overwrite__": 42})
  ```
  Only one Overwrite is allowed per superstep, and it ignores all subsequent values in that step. 

- Graph Builder `core/graph`
The `GraphBuilder` providers a fluent API to constrcut graphs:
```rs 
let mut builder = GraphBuilder::new();
builder 
    .add_channel("input", Box::new(LastValue::new("input")))
    .add_channel("output", Box::new(LastValue::new("output")))
    .add_node("process")
    .add_edge("process", "next_node", "output")
    .add_conditional_edge("process", "branch_a", "output", "condition_yes")
```
Key features:
* Validates graph integrity (no duplicate nodes/channels, no dangling references)
* Detects conditional branch conflicts (same branch name can't route to different nodes)
* Complies to `CompliedGraph` which bridges to the runtime 

- Scheduler `core/scheduler`
The scheduler implements determinstic task planning and state updates:

Planning `plan.rs`:
* `build_trigger_to_nodes() - maps channels to nodes that listen to them
* `is_node_triggered()` - checks if a node should execute based on channel version changes 
* `plan_next_tasks()` - determines which nodes to execute in the next superstep

Applying `apply.rs`
* `apply_writes()` - applies task outputs to channels in determinstic order
* Updates `versions_seen` each node (tracks what channel versions it has processed)
* Handles trigger channel consumption
* Bumps channel versions when state changes 
* Calls `finish()` on channels when no more triggers are pending 

- Types `core/types`
Runtime-native contracts shared across all modules:
* ChannelWrite: Basic write tuple `(channel_name, value)`
* Command: Control flow commands `(goto, send to parent graph, interrupts)`
* TaskDescriptor: Unique task identity with deterministic path
* NodeExecutionResult: What nodes return (writes, commands, return value)
* StreamEvent: Runtime event envelope for observability


# 2. Runtime Module - The Execution Engine 
The runtime implements a Pregel-style superstep loop that executes compiled graphs.

- Loop Engine (`engine.rs`)
The `LoopEngine` is the heart of the system. Here's the execution flow:

Intialization:
1. Load checkpoint from saver (or start fresh)
2. Restore channel state from checkpoint
3. Initialize scheduler checkpoint (channel versions, versions_seen)

Main Loop (each iteration is a "superstep"):
1. Apply input writes (if any) -> emit InputApplied event 
2. Save checkpoint -> emit CheckpointSaved event 
3. Loop until done/interrupted/out of steps:
   a. Plan next tasks based on channel versions 
   b. Check interrupt_before conditions -> break if matched 
   c. Emit StepStarted event
   d. Execute each planned task:
      - Build input from channels
      - Run node via TaskRunner (with retries)
      - Convert result to TaskWrites 
   e. Apply writes to channels -> emit WritesApplied event
   f. Create new checkpoint
   g. Save checkpoint and pending writes
   h. Check interrupt_after conditions -> break if matched
   i. Increment step counter 
4. Emit LooopFinished event
5. Return LoopRunSummary

Key Features:
* Deterministic: Same inputs always produce same execution order
* Resumable: Can interrupt and resume from any checkpoint
* Observable: EMits structued events fro every operation
* Concurrent-ready: Task execution can be parallelized (currentl syn)

- Task Runner `(runtime/runner)`
Wraps node execution with
* Retry logic (configurable retry limit)
* Error handling (fatal vs retryable)
* Cache integration (memoization of node results)
* Streaming event emission (task start/end/retry)

- Interrupts `(runtime/interrupts)`
Supports two interrupt modes:
* interrupt_before: Pause before executing specific nodes
* interrupt_after: Pause after executing specific nodes

Interrupts are checked using node name selectors and emit events for client handline 

- Streaming `(runtime/streaming)
Defines `RuntimeStream` trait and `StreamEvent` types:
* `InputApplied, `StepStarted`, `Writesapplied`
* `CheckpointSaved`, `InterruptBefore`, `InterruptAfter`
* `ResumeApplied`, `LoopFinished`
* Task-level events: `TaskStarted`, `TaskComplted`, `TaskRetry`
* Cache events: `TaskCacheMiss`, `TaskCacheHit`, `TaskCacheStored`


# 3. Checkpoint Module - State Persistence 
Checkpoints enable durable, resumable execution 

- Base Types `(types.rs)`
* Checkpoint: Snapshot of graph state at a specific step
  * id: Unique checkpoint identifier 
  * channel_values: Current state of all channels
  * channel_versions: Version number for each channel
  * version_seen: What versions each node has processed
  * pending_sends: Queued cross-graph messages
* CheckpointConfig: Identifies execution thread
  * thread_id: Main identifier 
  * checpoint_ns: Optional namespace for subgraphs
  * checkpoint_id: Optional specific checkpoint to resume from
* PendingWrite: Write that hasn't been applied yet
  * Used for tracking task outputs before next step

- Saver Trait `(saver.rs)`
Backend-agnostic interface:
* `get()` - retrieve checkpoint by config
* `put()` - save checkpoint with metadata
* `put_writes()` - save pending writes for a task
* `list()`- query checkpoint history 

Backends
* InMemorySaver: HashMap-based for testing
* SqliteSaver: SQLite-backed, single-process durable storage
* PostgresSaver: Postgres-backed, multi-process/distributed storage

All backends pass the same conformance test suite `(conformance.rs)`

# 4. Adapters Module - Framework Integration
Adapters bridge external AI frameworks to the LangGraph runtime without coupling prodivers types into core

- Adapter Node `(node.rs)`
The `AdapterNode` trait defines the integration contract:
```rs
pub trait AdapterNode {
    fn exeute(
        &self,
        input: Value,
        ctx: &AdapterContext,
    ) -> Result<NodeExecutionResult, NodeExecutionError>;
}
```
`FnAdapterNode` providers a closure-based helper for simple adapters

- Adapter Registry `(registry.rs)`
Maps node names to adapter implementations:
```rs
let mut registry = AdapterRegistry::new();
registry.register_node("my_llm", MyLLMAdapter::new())?;
```

- Adapter Runner `(runner.rs)`
Implements `LoopNodeRunner` by dispatching to registered adapters:
```rs
let runner = AdapterRunner::new(registry)
    .with_node("echo", FnAdapterNode::new(|input, _| {
        Ok(NodeExecutionResult::default().with_return_value(input))
    }))?;
```
Provider Modules
* langchain_rust: Integration patterns for langchain-rust crates
* rig: Integration patterns for rig AI framework

These provider constructor helpers and type conversions specific to each framework

# 5. Cache module - Result Memoization
Caches node execution results to aviod redundant computation.

- Cache Trait `(cache/base)`
* `get(key) - retrieve cached result
* `set(key, value, ttl)` - store result with optional TTL
* `delete(key)` - remove entry
* `clear()` - remove all entries 
* `prune_expired()` - cleanup expired entries 

Integration
The runtime's `TaskRunner` checks cache before execution:
1. Compute cache key from task descriptor + input
2. Check cache -> emit `TaskCacheHit` if found
3. Execute node if miss -> emit `TaskCacheMiss`
4. Store result -> emit `TaskCacheStored`

Backends
* InMemoryCache: HaspMap with TTL tracking
* SqliteCache: SQLite-backed with automatic expiry
* PostgresCache: Postgres-backed with automatic expiry

# 6. Store Module - Long-term Storage
Provides key-value storage with vector search capabilities for agent memory.

- Store Trait `(store/base)`
* `put(namespace, key, value)` - store item
* `get(namespace, key)` - retrieve item
* `delete(namespace, key)` - rmeove item
* `list(namespace, filters)` - query items
* `search(namespace, query)` - semantic search
* `put_embedding()/vector_search() - vector operations

Use Cases
* Long-term agent memory across sessions
* Document storage for RAG
* Sematic search over stored knowledge

Backends
* InMemoryStore: HashMap-based
* SqliteStore: SQLite with FTS and vector suppory
* PostgresStore: Postgres with pgvector integration 

- Execution Flow Example
Here's a complete execution trace:
```rs
// 1. Build graph
let mut builder = GraphBuilder::new();
builder 
    .add_channel("input", Box::new(LastValue::new("input")))
    .add_channel("output", Box::new(LastValue::new("output")))
    .add_node_with_triggers("process", vec!["input"])
    .add_edge("process", "finish", "output")

let graph = builder.compile()?;

// 2. Create adapter runner
let runner = AdapterRunner::default()
    .with_node("process", FnAdapterNode::new(|input, _ctx| {
        let value = input.get("input").unwrap();
        Ok(NodeExecutionResult::default()
             .with_write(ChannelWrite::new("output", value)))
    }))?;

// 3. Execute with chckpoint
let saver = SqliteSaver::new("graph.db")?;
let config = LoopConfig:new(CheckpointConfig::new("thread-1"))
     .with_recursion_limit(10);

let result = graph.run(
    &runner,
    Some(&saver),
    config,
    vec![ChannelWrite::new("input", json!("hello"))],
)?;

// Result contains:
// - status: Done/InterruptedBefore/InterruptedAfter/OutOfSteps
// - checkpoint: Final state snapshot
// - steps_executed: Number of supersteps
// - tasks_executed: Number of node executions
```
What happens internally:
1. Input write updates "input" channel -> version bumps to 1
2. Scheduler plans "process" node (triggered by "input" channel)
3. TaskRunner executes "process" with input from channels
4. Node returns write to "output" channel
5. Scheduler applies write -> "output" version bumps to 1
6. Checkpoint saved with all channel states
7. No more nodes triggered -> loop exits with Done status 


# Key Design Principles
1. Determinism: Same inputs always produce same execution order (critical for debugging/testing)
2. Resumability: Any execution can be interrupted and resumed from checkpoint 
3. Observability: Every operation emits structured events
4. Modularity: Core logic is framework-agnostic, adapters handle integration
5. Backend Flexibilty: Swap memory/sqlite/postgres for checkpoints/cache/store
6. Type Safety: Rust's type system prvents many runtime errors
7. Pregel model: Superstep-based execution enables distributed scaling(future)