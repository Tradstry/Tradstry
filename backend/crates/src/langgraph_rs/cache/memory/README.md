# `cache/memory` Purpose

This folder provides an in-memory implementation of the cache contract.

## Design Rule
- Deterministic key ordering through map storage.
- Thread-safe access using `RwLock`.
- TTL-aware reads and explicit expired-entry pruning.

## File Map

### `mod.rs`
- Public export for `InMemoryCache`.

### `cache.rs`
- Implements `Cache` for `InMemoryCache`:
  - `get/set/delete`
  - `clear_namespace`, `clear_all`
  - `prune_expired`
