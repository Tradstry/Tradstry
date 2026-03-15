# `adapters` Purpose

This folder integrates external Rust AI crates into the LangGraph runtime without coupling provider
types into `core` or `runtime`.

## Responsibilities
- Provider-specific execution wrappers.
- Data conversion between external crate types and runtime-native types.
- Optional integrations that do not pollute core runtime modules.

## File Map

### `mod.rs`
- Public export surface for adapter contracts and provider modules.

### `types.rs`
- Defines `AdapterContext` (owned execution context passed to adapter nodes).

### `node.rs`
- Defines `AdapterNode` trait and `FnAdapterNode` helper for closure-based adapters.

### `registry.rs`
- Defines `AdapterRegistry` for adapter-node registration and lookup by graph node name.

### `runner.rs`
- Defines `AdapterRunner`, a `LoopNodeRunner` dispatcher that resolves node names through `AdapterRegistry`.

### `error.rs`
- Defines adapter-layer validation errors (invalid name, duplicate registration, missing node).

## Current Coverage
- Implemented:
  - provider-agnostic adapter-node contract
  - registry-backed dispatch into runtime loop
  - closure-based adapter helper
  - provider modules for `rig` and `langchain_rust` constructor patterns
- Planned next:
  - concrete type-level bindings to selected `rig`/`langchain-rust` APIs behind cargo features
