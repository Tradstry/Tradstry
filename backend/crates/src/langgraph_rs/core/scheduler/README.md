# `core/scheduler` Purpose

This folder contains deterministic superstep planning and write-application logic.

## Design Rule
- Scheduler code must be deterministic for identical inputs.
- Channel state transitions happen through channel contracts only.
- Planning logic depends on channel versions + versions seen by nodes.

## File Map

### `mod.rs`
- Public export surface for scheduler state, planning, apply logic, and errors.

### `state.rs`
- Defines scheduler state contracts:
  - `SchedulerCheckpoint`
  - `ChannelVersions`
  - `VersionsSeen`
  - `TriggerToNodes`
  - `NodeScheduleSpec`
  - `PlannedTask` / `PlannedTaskKind`
  - `TaskWrites`
- Includes `TaskWrites::from_execution_result` to convert node execution output into scheduler writes.
- Defines runtime constants:
  - `DEFAULT_TASKS_CHANNEL` (`__pregel_tasks`)
  - `PULL_TASK_PREFIX` (`__pregel_pull`)
  - `PUSH_TASK_PREFIX` (`__pregel_push`)
  - `PUSH_WRITE_CHANNEL` (`__pregel_push`)

### `apply.rs`
- Implements `apply_writes(...)`.
- Responsibilities:
  - deterministic task-write sorting
  - versions-seen updates
  - trigger channel `consume()`
  - grouped channel `update()`
  - step bump via empty updates
  - final-step `finish()` handling
- Includes reserved write channel filtering and apply summary output.

### `plan.rs`
- Implements:
  - `build_trigger_to_nodes(...)`
  - `is_node_triggered(...)`
  - `plan_next_tasks(...)`
  - `plan_next_tasks_detailed(...)`
- Responsibilities:
  - read and validate tasks-channel payloads for push `SendPacket` fan-out
  - determine candidate pull nodes from updated channels
  - check trigger availability/version advancement
  - produce deterministic push + pull planned tasks (push first, then pull)

### `error.rs`
- Defines `SchedulerError`:
  - channel operation failures
  - send serialization failures
  - invalid tasks-channel payload shape

## Current Coverage
- Implemented:
  - deterministic write application
  - trigger map generation
  - detailed next-task planning with push (`SendPacket`) + pull tasks
  - deterministic task prefixes for push/pull
  - scheduler unit tests for apply and planning paths
- Planned next:
  - functional-call push parity (beyond `SendPacket`) if/when required
  - richer metadata required by runtime/checkpoint history APIs
