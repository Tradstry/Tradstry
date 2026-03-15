# `runtime/streaming` Purpose

This folder exposes structured runtime stream events for loop and runner execution.

## Design Rule
- Streaming is transport-agnostic: runtime emits `StreamEvent` envelopes through a trait.
- Event payloads are structured and deterministic for identical execution paths.
- Loop and runner emit events in execution order.

## File Map

### `mod.rs`
- Public export surface for runtime streaming contracts and event model.

### `event.rs`
- Defines `RuntimeEvent` (structured runtime lifecycle payloads).
- Maps events to `StreamMode` and converts them into `StreamEvent`.

### `emitter.rs`
- Defines `RuntimeStream` trait (`emit(StreamEvent)`).
- Provides `StreamCollector` test/dev implementation for capturing ordered events.

## Current Coverage
- Implemented:
  - structured events for input/resume/interrupt/step/task/write/checkpoint/final-status lifecycle
  - task cache lifecycle events (`task_cache_miss`, `task_cache_hit`, `task_cache_stored`)
  - trait-based sync event delivery
  - in-memory event collector for tests and local debugging
  - optional dual-mode parity emission via runtime IO mappers when
    `LoopConfig.stream_parity_mode == DualPythonCompat`
  - parity mode selection through `LoopConfig.parity_stream_modes`
  - parity chunks for:
    - `values`
    - `updates`
    - `tasks` (task + task_result payloads)
    - `checkpoints`
    - `debug` wrappers for task/task_result/checkpoint payloads
    - `custom` (raw node custom events)
    - `messages` (node message events as `[message, metadata]`)
- Planned next:
  - async sink adapters
  - stream filtering/subscription policies by mode and namespace
