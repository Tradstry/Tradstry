# `checkpoint/postgres` Purpose

This folder implements a Postgres checkpoint backend.

## Design Rule
- This backend implements `CheckpointSaver` from `checkpoint/base`.
- It targets durable multi-run storage with transactional semantics.

## File Map

### `mod.rs`
- Public export for `PostgresSaver`.

### `saver.rs`
- `PostgresSaver` implementation backed by the `postgres` crate.
- Creates and maintains checkpoint/write schema automatically.
- Supports:
  - `put`, `get_tuple`, `list`, `put_writes`, `delete_thread`
  - optional capabilities: `delete_for_runs`, `copy_thread`, `prune`
- Includes tests that run when `LANGGRAPH_RS_TEST_POSTGRES_URL` is set.

## Current Coverage
- Implemented:
  - postgres schema for checkpoints + writes
  - parent checkpoint linkage
  - reserved write index behavior for special channels
  - filtering (`before`, metadata, limit)
  - maintenance ops (`delete_thread`, `delete_for_runs`, `copy_thread`, `prune`)
- Limitations:
  - test suite requires an external postgres instance via env var
  - metadata filtering currently runs in Rust after row load
