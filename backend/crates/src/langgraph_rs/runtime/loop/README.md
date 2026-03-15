# `runtime/loop` Purpose

This folder implements the main deterministic execution loop for graph runs.

## Design Rule
- The loop coordinates scheduler + channels + checkpointer.
- Task execution is delegated through a runner trait.
- Supersteps are deterministic for identical checkpoint/channel inputs.

## File Map

### `mod.rs`
- Public export surface for loop engine, errors, and loop contracts.
- Internal persistence worker module is wired for async durability execution.

### `types.rs`
- Defines:
  - `LoopConfig`
  - loop-level retry policy and concurrency options
  - `LoopInput`
  - `DurabilityMode`
  - `StreamParityMode`
  - `parity_stream_modes` selector for Python-compatible chunk emission by mode
  - `LoopNodeRunner`
  - `LoopStatus`
  - `LoopTaskReport`
  - `LoopRunSummary`

### `error.rs`
- Defines `LoopError` for:
  - scheduler/checkpoint/channel errors
  - missing node specs
  - task execution failures
  - bubbled parent commands from node execution

### `engine.rs`
- Implements `LoopEngine`:
  - checkpoint load/restore
  - input/command mapping and null-task write application
  - pending-write replay from checkpoint tuples
  - superstep planning (`plan_next_tasks_detailed`)
  - same-step push acceptance for send packets
  - progressive task execution via runner APIs (including bounded concurrency)
  - write application (`apply_writes`)
  - per-task write persistence path before superstep barrier checkpoint
  - durability-aware checkpoint persistence (`sync` / `async` / `exit`)
  - async durability now uses a scoped background persistence worker with FIFO payload processing
  - strict per-payload checkpoint-then-writes ordering and end-of-run worker flush
- Includes loop unit tests for:
  - normal execution and persistence
  - recursion limit behavior
  - task failure propagation
  - command routing and replay paths

## Current Coverage
- Implemented:
  - deterministic sync loop kernel
  - checkpoint restore + pending-write replay
  - scheduler-integrated planning/apply cycle
  - backend-agnostic runner interface
  - runner-driven progressive execution integration
  - loop-level `max_concurrency` / retry-policy wiring
  - command input routing (`LoopInput::Command`)
  - command outputs from nodes (current-graph writes + parent-command bubbling)
  - optional Python-compatible output chunk emission in dual-mode:
    - `values`
    - `updates`
    - `tasks`
    - `checkpoints`
    - `debug` wrappers
    - `custom`
    - `messages`
  - explicit parity chunk mode selection via `LoopConfig.parity_stream_modes`
  - runtime streaming emission hooks (`run_with_stream`)
  - cache-aware loop entrypoint (`run_with_stream_and_cache`) for runner-level task memoization
  - interrupt selector integration with update-since-last-interrupt gate
  - resume signal stream emission (`resume_applied`)
  - durability mode support with final output snapshots
  - background async checkpoint persistence worker with deterministic flush-on-return behavior
- Planned next:
  - parent-graph command routing for loop input in nested graph contexts
  - nested graph interruption/suppression parity
