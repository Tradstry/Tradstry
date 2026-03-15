# `checkpoint/sqlite` Purpose

This folder implements a SQLite checkpoint backend.

## Design Rule
- This backend implements `CheckpointSaver` from `checkpoint/base`.
- It prioritizes deterministic behavior and local durability over distributed scale.

## File Map

### `mod.rs`
- Public export for `SqliteSaver`.

### `saver.rs`
- `SqliteSaver` implementation backed by `rusqlite`.
- Creates and maintains checkpoint/write schema automatically.
- Supports:
  - `put`, `get_tuple`, `list`, `put_writes`, `delete_thread`
  - optional capabilities: `delete_for_runs`, `copy_thread`, `prune`
- Includes unit tests for core and optional capability paths.

## Current Coverage
- Implemented:
  - sqlite schema for checkpoints + writes
  - parent checkpoint linkage
  - reserved write index behavior for special channels
  - filtering (`before`, metadata, limit)
  - maintenance ops (`delete_thread`, `delete_for_runs`, `copy_thread`, `prune`)
- Limitations:
  - current metadata filtering happens in Rust after row load
  - no background compaction strategy yet
