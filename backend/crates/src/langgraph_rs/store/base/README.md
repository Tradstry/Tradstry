# `store/base` Purpose

This folder defines the canonical store contract shared by all store backends.

## Design Rule
- Keep the contract backend-agnostic and deterministic.
- Support both sync and async call shapes.
- Keep filtering/search semantics stable across backends.

## File Map

### `mod.rs`
- Public export surface for store errors, trait, and core data types.

### `error.rs`
- Defines `StoreError` for validation, storage, and capability boundaries.

### `types.rs`
- Defines:
  - `NamespacePath`
  - `StoreItem`
  - `StoreListQuery`
  - `StoreSearchQuery`
  - `EmbeddingVector`
  - `StoreVectorQuery`
  - `StoreScoredItem`
  - `VectorMetric`
- Includes namespace prefix matching helper and timestamp helper.
- Includes vector scoring helpers for cosine/dot/euclidean retrieval.

### `store.rs`
- Defines the `Store` trait:
  - `put/get/delete`
  - `list/search`
  - `put_embedding/get_embedding/delete_embedding`
  - `vector_search`
  - convenience composition (`put_with_embedding`)
  - async wrappers for all of the above
