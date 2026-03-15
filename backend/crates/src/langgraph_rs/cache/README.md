# `cache` Purpose

This folder implements node-result caching interfaces and backends.

## Design Rule
- Keep cache APIs backend-agnostic.
- Ensure TTL behavior is explicit and deterministic.
- Keep node-result cache payloads serializable for backend portability.

## File Map

### `mod.rs`
- Public export surface for cache contracts and backends.

### `base/*`
- Core cache trait + key/item model + errors + node-result cache conversion helpers.

### `memory/*`
- In-memory cache backend with TTL support and maintenance operations.

### `sqlite/*`
- SQLite-backed durable cache backend with TTL-aware pruning support.

### `postgres/*`
- Postgres-backed durable cache backend with TTL-aware pruning support.

### `conformance.rs`
- Shared cache conformance tests.

## Current Coverage
- Implemented:
  - cache key/item/options contracts with TTL fields
  - sync/async cache trait operations
  - backend-specific expired-entry cleanup policy (`get` may return `None` with optional eager cleanup)
  - node execution result conversion helpers for cache reuse paths
  - in-memory/sqlite/postgres backends with `get/set/delete/clear/prune_expired`
  - backend conformance tests shared across cache backends
  - runtime runner memoization integration hooks (`task_cache_miss/hit/stored`)
- Planned next:
  - cache invalidation strategies scoped by graph/node evolution versioning
  - runtime loop-level convenience APIs for cache policy defaults
