# `store/memory` Purpose

This folder provides an in-memory implementation of the store contract.

## Design Rule
- Deterministic ordering for listing/searching through map-based storage.
- Thread-safe reads/writes via `RwLock`.
- No persistence guarantees; intended for tests and local development.

## File Map

### `mod.rs`
- Public export for `InMemoryStore`.

### `store.rs`
- Implements `Store` for `InMemoryStore`:
  - `put/get/delete`
  - `list` with namespace and namespace-prefix filters
  - `search` with basic case-insensitive key/value text matching
  - `put_embedding/get_embedding/delete_embedding`
  - `vector_search` using in-memory similarity scoring
