# `core/graph` Purpose

This folder implements both the low-level graph API and the high-level StateGraph API.

## Responsibilities
- Low-level graph builder APIs for channels/nodes/edges.
- High-level state schema + state graph ergonomics.
- Compilation to runtime scheduler + loop execution.
- Validation rules for graph integrity before execution.

## File Map

### `mod.rs`
- Public export surface for both low-level and state-graph APIs.

### `types.rs`
- Defines graph-native metadata:
  - `GraphNodeSpec`
  - `GraphEdgeKind`
  - `GraphEdgeSpec`
  - `GraphDefinition`

### `error.rs`
- Defines `GraphError` for:
  - duplicate nodes/channels
  - unknown node/channel references
  - conditional branch conflicts
  - graph validation failures

### `builder.rs`
- Implements `GraphBuilder`:
  - channel/node registration
  - trigger registration
  - direct and conditional edges
  - graph validation
  - build/compile entry points

### `compiled.rs`
- Implements `CompiledGraph`:
  - conversion from graph definition to scheduler specs
  - runtime loop bridging (`run` / `run_with_stream`)
  - compiled metadata accessors
  - integration tests with runtime loop

### `managed.rs`
- State-graph managed value declarations:
  - `ManagedValueKind::{IsLastStep, RemainingSteps}`

### `state_schema.rs`
- Runtime schema DSL for state fields:
  - `StateSchema`
  - `StateField`
  - `StateFieldKind`
- Supports channel-bound fields (`LastValue`, `Topic`, `AnyValue`, `UntrackedValue`, `BinaryOperatorAggregate`) and managed fields.

### `state.rs`
- High-level `StateGraph` builder:
  - `add_node`, `add_edge`, `add_conditional_edges`
  - `set_entry_point`, `set_conditional_entry_point`, `set_finish_point`
  - compile-time validation for entrypoint/targets/reserved names.

### `state_compiled.rs`
- `CompiledStateGraph` runtime bridge:
  - compiles schema to channels + scheduler specs
  - injects managed values into node inputs
  - executes direct edge routing, conditional branch routing, and command-driven routing
  - exposes `invoke`, `invoke_with_stream`, and `run_raw` APIs.

## Current Coverage
- Implemented:
  - deterministic low-level graph builder/compiled execution
  - additive high-level `StateGraph` with runtime schema DSL
  - reducer and managed-value field support
  - conditional branch routing + `Command.goto`/`Send` coexistence
  - read-channel aware task input construction through scheduler spec integration
- Remaining:
  - parent-graph routing semantics for nested graph execution
  - derive-macro typed schema ergonomics (runtime DSL is current v1 path)
  - broader subgraph composition helpers
