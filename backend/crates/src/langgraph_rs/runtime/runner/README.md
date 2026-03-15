# `runtime/runner` Purpose

This folder runs prepared tasks with retry policy and normalized failure handling.

## Design Rule
- Runner owns task execution attempts and retry decisions.
- Loop delegates execution policy to runner, then applies resulting writes.
- Retry behavior is deterministic for identical error sequences.

## File Map

### `mod.rs`
- Public export surface for runner config, error, request/result, and executor.

### `types.rs`
- Defines:
  - `RunnerConfig`
  - `RetryPolicy`
  - `RetryOn`
  - `TaskExecutionRequest`
  - `TaskExecutionResult`

### `error.rs`
- Defines `RunnerError` for:
  - terminal execution failures after retry policy
  - parent-command bubbling from node outputs

### `engine.rs`
- Implements `TaskRunner`:
  - `execute_one(...)` for single-task execution
  - `execute_many_progressive(...)` for concurrent progressive step execution
  - first-match retry policy resolution (`RetryPolicy`)
  - optional writes-oriented task memoization via `cache::base::Cache`
  - command output handling (current-graph mapping, parent-command bubbling)
  - normalized success/failure envelopes + stream events
- Includes runner unit tests for:
  - retry policy matching and exhaustion
  - cache hit/store behavior
  - parent-command bubbling
  - progressive multi-task execution

## Current Coverage
- Implemented:
  - first-class retry policy model (`RetryPolicy` / `RetryOn`)
  - compatibility `with_retry_limit(...)` shim
  - backoff + max-interval + jitter retry timing
  - deterministic attempt accounting
  - attempt/resuming metadata in execution context
  - optional cache hit/miss/store memoization path
  - writes-oriented cache envelope and stable cache keys (`node + input hash`)
  - structured task lifecycle stream emission
  - concurrent progressive runner API with bounded in-flight work
  - parent-command bubbling error path
- Planned next:
  - full Python async runner parity and cancellation orchestration
