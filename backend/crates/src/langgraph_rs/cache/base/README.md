# `cache/base` Purpose

This folder defines the canonical cache contract shared by cache backends.

## Design Rule
- Keep API backend-agnostic and deterministic.
- Model TTL semantics explicitly at the contract layer.
- Keep runtime integration payloads serializable.

## File Map

### `mod.rs`
- Public export surface for cache trait, errors, and data model.

### `error.rs`
- Defines `CacheError` for validation, capability, serialization, and storage boundaries.

### `types.rs`
- Defines:
  - `CacheKey`
  - `CacheSetOptions`
  - `CacheItem`
  - node-result cache envelope conversion helpers
- Includes namespace prefix helper and timestamp helper.

### `cache.rs`
- Defines `Cache` trait:
  - `get/set/delete`
  - optional maintenance operations (`clear_namespace`, `clear_all`, `prune_expired`)
  - async wrappers
