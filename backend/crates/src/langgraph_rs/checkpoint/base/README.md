# `checkpoint/base` Purpose

This folder defines the canonical checkpoint contracts used by runtime and storage backends.

## Design Rule
- `checkpoint/base` owns shared types and saver APIs.
- Backend modules (`memory`, `sqlite`, `postgres`) should implement `CheckpointSaver`.
- Runtime/scheduler code should depend on these contracts, not backend-specific logic.

## File Map

### `mod.rs`
- Public export surface for checkpoint base types, errors, saver trait, and helper functions.

### `error.rs`
- Defines `CheckpointError`:
  - channel failures
  - invalid config
  - not-implemented operations
  - unsupported optional capabilities
  - storage/serialization failures

### `id.rs`
- Checkpoint identity helpers:
  - `next_checkpoint_id(clock_seq)`
  - `next_uuid6_checkpoint_id(clock_seq)`
  - `next_checkpoint_id_with_strategy(clock_seq, strategy)`
  - `now_timestamp_string()`
- Supports `CheckpointIdStrategy`:
  - `LegacyMonotonic` (default)
  - `Uuid6` (interop mode)

### `types.rs`
- Shared checkpoint data model:
  - `Checkpoint`
  - `CheckpointConfig`
  - `CheckpointMetadata`
  - `CheckpointTuple`
  - `PendingWrite`
  - `ListCheckpointsQuery`
  - `PruneStrategy`
- Defines version/type aliases:
  - `ChannelVersions`
  - `VersionsSeen`
  - `CheckpointParents`
  - `MetadataMap`
- Includes helper utilities:
  - `empty_checkpoint`
  - `empty_checkpoint_with_config`
  - `copy_checkpoint`
  - `create_checkpoint`
  - `create_checkpoint_with_config`
  - `get_checkpoint_id`
  - `get_checkpoint_metadata`
  - `get_serializable_checkpoint_metadata`
  - `deserialize_checkpoint_json`
  - `normalize_checkpoint_for_read`
  - `project_checkpoint_for_storage`
  - `write_idx_for_channel`
  - `is_excluded_metadata_key`
- Compatibility policy:
  - `CheckpointWireFormat` (`RustV1`, `PythonV2`, `PythonV4`)
  - `CheckpointReadCompatibility` (default reads v1/v2/v4)
  - `CheckpointCompatibilityPolicy` (`read_compat`, `write_format`, `id_strategy`)

### `saver.rs`
- Defines `CheckpointSaver` trait with sync + async methods:
  - `get/get_tuple/list`
  - `put/put_writes`
  - `delete_thread`
  - optional capabilities (`delete_for_runs`, `copy_thread`, `prune`)
- Async methods default to sync behavior.
- Provides default version increment logic via `get_next_version`.

## Current Coverage
- Implemented:
  - canonical checkpoint types and metadata model
  - checkpoint read-compat normalization for Rust v1 + Python-like v2/v4
  - opt-in write projection to Rust v1 / Python v2 / Python v4 formats
  - configurable ID strategy (`legacy` default, `uuid6` optional)
  - write-index reservation mapping for special channels
  - checkpoint creation/copy helpers
  - Python-style metadata merge/exclusion/sanitization + `writes` stripping
  - base saver contract with capability fallbacks
  - unit tests for ID monotonicity and key invariants
- Planned next:
  - additional mixed-backend interop migration fixtures for long-term compatibility hardening
