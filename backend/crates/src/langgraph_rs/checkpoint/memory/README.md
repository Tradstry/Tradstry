# `checkpoint/memory` Purpose

This folder provides an in-memory checkpoint saver implementation.

## Design Rule
- This backend implements `CheckpointSaver` from `checkpoint/base`.
- It favors deterministic behavior for tests and local development over durability.

## File Map

### `mod.rs`
- Public export for `InMemorySaver`.

### `saver.rs`
- `InMemorySaver` implementation backed by `RwLock` + `BTreeMap`.
- Supports:
  - `put`, `get_tuple`, `list`, `put_writes`, `delete_thread`
  - optional capabilities: `delete_for_runs`, `copy_thread`, `prune`
- Maintains deterministic ordering for checkpoint and write iteration.
- Includes unit tests for core behavior and optional capability paths.

## Current Coverage
- Implemented:
  - checkpoint persistence in memory
  - parent checkpoint linkage
  - pending-write storage with reserved index handling
  - filtering (`before`, metadata, limit)
  - maintenance ops (`delete_thread`, `delete_for_runs`, `copy_thread`, `prune`)
- Limitations:
  - no process durability
  - no cross-process sharing
