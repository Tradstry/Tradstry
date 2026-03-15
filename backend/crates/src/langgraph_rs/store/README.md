# `store` Purpose

This folder defines long-term key-value/search store abstractions for runtime memory.

## Design Rule
- Keep a stable backend-agnostic API first.
- Ensure deterministic semantics for list/search filters.
- Add backends incrementally while preserving trait behavior.

## File Map

### `mod.rs`
- Public export surface for store contracts/backends.

### `base/*`
- Core trait + data model + error type for store operations.

### `memory/*`
- In-memory `Store` implementation for tests/dev environments.

### `sqlite/*`
- SQLite-backed durable `Store` implementation.

### `postgres/*`
- Postgres-backed durable `Store` implementation.

### `conformance.rs`
- Shared backend conformance-style test suite.

## Current Coverage
- Implemented:
  - base store trait and typed query/item/vector model
  - in-memory/sqlite/postgres backends (`put/get/delete/list/search`)
  - embedding/vector retrieval integration points:
    - `put_embedding/get_embedding/delete_embedding`
    - `vector_search` with metric-aware scoring
  - backend conformance tests shared across backends
- Planned next:
  - external embedding model adapters + async batched indexing pipeline
  - pgvector/ANN-backed acceleration path for large datasets
