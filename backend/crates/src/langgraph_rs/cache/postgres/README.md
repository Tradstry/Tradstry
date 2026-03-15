# `cache/postgres` Purpose

This folder implements a Postgres cache backend.

## Design Rule
- Implements the `Cache` trait from `cache/base`.
- Targets durable multi-run caching with TTL-aware cleanup.

## File Map

### `mod.rs`
- Public export for `PostgresCache`.

### `cache.rs`
- `PostgresCache` implementation backed by the `postgres` crate.
- Initializes schema automatically.
- Supports:
  - `get/set/delete`
  - `clear_namespace`, `clear_all`, `prune_expired`
- Includes tests that run when `LANGGRAPH_RS_TEST_POSTGRES_URL` is set.
