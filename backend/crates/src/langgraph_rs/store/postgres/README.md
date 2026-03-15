# `store/postgres` Purpose

This folder implements a Postgres store backend.

## Design Rule
- Implements the `Store` trait from `store/base`.
- Targets durable multi-run memory persistence.

## File Map

### `mod.rs`
- Public export for `PostgresStore`.

### `store.rs`
- `PostgresStore` implementation backed by the `postgres` crate.
- Initializes schema automatically.
- Supports:
  - `put`, `get`, `delete`, `list`, `search`
  - `put_embedding`, `get_embedding`, `delete_embedding`, `vector_search`
- Includes tests that run when `LANGGRAPH_RS_TEST_POSTGRES_URL` is set.
