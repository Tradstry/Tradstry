# `core/types` Purpose

This folder contains runtime-native contracts shared across graph compilation, scheduling,
execution, checkpointing, and adapters.

## Design Rule
- `core/types` must stay external-crate-agnostic.
- Adapters map `rig` or `langchain-rust` inputs/outputs into these types.
- Scheduler/runtime/checkpoint modules should depend on these contracts, not on adapter types.

## File Map

### `mod.rs`
- Public re-export surface for the whole `types` module.
- Defines which types are considered stable imports for other modules.

### `write.rs`
- Basic aliases and write tuple model:
  - `ChannelName`
  - `NodeName`
  - `TaskId`
  - `ChannelWrite`
- This is the lowest-level state mutation primitive used by scheduler/runtime.

### `command.rs`
- Graph command model:
  - `CommandGraph` (`current` vs `parent`)
  - `SendPacket`
  - `GotoTarget`
  - `CommandUpdate`
  - `Command`
- Contains helper conversion from command updates into `Vec<ChannelWrite>`.
- Accepts Python-compatible deserialize shapes for:
  - `graph="__parent__"` alias
  - single `goto` object or list `goto`
- This is the primary control/write envelope emitted by nodes.

### `interrupt.rs`
- Interrupt payload contracts:
  - `InterruptId`
  - `Interrupt`
- Stable namespace-hash helper:
  - `interrupt_id_from_namespace`
- Holds resumable pause data surfaced to clients and persisted in checkpoints.

### `overwrite.rs`
- Overwrite marker contracts:
  - `OVERWRITE_MARKER`
  - `Overwrite`
  - overwrite detection helpers
- Used by aggregate channels to bypass reducer logic with explicit replacement values.

### `task.rs`
- Task identity and deterministic task path representation:
  - `TaskPathPart`
  - `TaskPath`
  - `TaskPathStr`
  - `TaskDescriptor`
- Used for stable ordering, debugging, and checkpoint write association.

### `stream.rs`
- Streaming event contracts:
  - `StreamMode`
  - `StreamEvent`
- Defines runtime event envelope independent of execution backend.

### `execution.rs`
- Node execution boundary:
  - `NodeExecutor` trait
  - `ExecutionContext`
  - `NodeExecutionResult`
  - `NodeExecutionErrorKind`
  - `NodeExecutionError`
- This is the adapter integration point: every adapter should produce
  `NodeExecutionResult` and `NodeExecutionError`.
- `NodeExecutionResult` also carries control/stream payloads used by state graph parity:
  - `command` (for `goto` / `send` routing)
  - `custom_events`
  - `message_events`

## How Other Modules Should Use This Folder
- `core/scheduler`: consumes `ChannelWrite`, `TaskDescriptor`, `Command`.
- `runtime/loop` and `runtime/runner`: consume execution contracts and stream contracts.
- `checkpoint/*`: persists task IDs/paths, interrupts, and writes.
- `adapters/*`: implement `NodeExecutor` and map external tool/model results into these types.
