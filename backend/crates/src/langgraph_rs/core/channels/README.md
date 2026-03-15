# `core/channels` Purpose

This folder implements channel semantics used by the runtime superstep engine.

## Design Rule
- Channels own local state and update logic.
- Scheduler applies writes; channels decide whether the state changed.
- Checkpoint serialization/deserialization must be deterministic.

## File Map

### `mod.rs`
- Public export surface for channel contracts and concrete channel types.

### `base.rs`
- Defines the core `Channel` trait:
  - `get`
  - `update`
  - `consume`
  - `finish`
  - `checkpoint`
  - `from_checkpoint`
- Includes boxed-clone support for trait-object channels.

### `error.rs`
- Defines `ChannelError`:
  - `EmptyChannel`
  - `InvalidUpdate`
  - `InvalidCheckpoint`
- Provides helper constructors for update/checkpoint validation errors.

### `last_value.rs`
- `LastValue`: accepts a single update per superstep and stores latest value.
- `LastValueAfterFinish`: stores latest value but only exposes it after `finish()`.
- Includes unit tests for single-update enforcement and finish-gated behavior.

### `ephemeral.rs`
- `EphemeralValue`: stores value for one step and clears on empty update.
- Supports `guard` mode to enforce single update per superstep.
- Includes unit test for clear-on-empty semantics.

### `topic.rs`
- `Topic`: append/flatten semantics for multi-value publication.
- Supports `accumulate=true|false` behavior.
- Overrides checkpoint to persist internal list state.
- Includes unit test for array flattening behavior.

### `any_value.rs`
- `AnyValue`: stores the latest value and clears on empty updates.
- Supports permissive multi-update semantics where the last value wins.

### `untracked_value.rs`
- `UntrackedValue`: stores transient values that are never checkpointed.
- Supports `guard=true|false` mode to enforce one update per step when needed.

### `binop.rs`
- `BinaryOperatorAggregate`: applies a reducer to aggregate updates.
- Supports overwrite packets with canonical marker `{"__overwrite__": ...}`.
- Includes built-in numeric-add reducer and custom reducer constructor support.

### `barrier.rs`
- `NamedBarrierValue`: fan-in gate; available only when all named inputs are seen.
- `NamedBarrierValueAfterFinish`: same fan-in gate plus explicit `finish()` gating.
- Includes unit tests for fan-in and finish-gated release.

## Current Coverage
- Implemented:
  - `LastValue`
  - `LastValueAfterFinish`
  - `EphemeralValue`
  - `Topic`
  - `AnyValue`
  - `UntrackedValue`
  - `BinaryOperatorAggregate`
  - `NamedBarrierValue`
  - `NamedBarrierValueAfterFinish`
- Runtime persistence note:
  - `UntrackedValue` writes are filtered from persisted pending writes.
  - `SendPacket` payloads persisted via tasks-channel writes are sanitized to remove top-level
    keys mapped to `UntrackedValue` channels.
