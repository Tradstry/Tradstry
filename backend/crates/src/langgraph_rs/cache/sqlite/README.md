# `cache/sqlite` Purpose

This folder implements a SQLite cache backend.

## Design Rule
- Implements the `Cache` trait from `cache/base`.
- Prioritizes deterministic local persistence and TTL cleanup support.

## File Map

### `mod.rs`
- Public export for `SqliteCache`.

### `cache.rs`
- `SqliteCache` implementation backed by `rusqlite`.
- Initializes schema automatically.
- Supports:
  - `get/set/delete`
  - `clear_namespace`, `clear_all`, `prune_expired`
