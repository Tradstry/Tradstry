# `store/sqlite` Purpose

This folder implements a SQLite store backend.

## Design Rule
- Implements the `Store` trait from `store/base`.
- Prioritizes local durability and deterministic behavior.

## File Map

### `mod.rs`
- Public export for `SqliteStore`.

### `store.rs`
- `SqliteStore` implementation backed by `rusqlite`.
- Initializes schema automatically.
- Supports:
  - `put`, `get`, `delete`, `list`, `search`
  - `put_embedding`, `get_embedding`, `delete_embedding`, `vector_search`
- Includes unit tests for local roundtrip and search behavior.
