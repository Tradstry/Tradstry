# `parity_tests` Purpose

This folder will hold parity tests against Python LangGraph semantics.

## Responsibilities
- Verify scheduler/channel/checkpoint behavior matches expected superstep results.
- Lock in interrupt/resume and command routing semantics.
- Prevent regressions during phased Rust migration.

## Current Coverage
- `any_value`: state transition parity (empty/update/clear).
- `untracked_value`: guard behavior and non-persistence parity.
- `binop`: aggregate + overwrite + overwrite-conflict parity.
- `scheduler`: golden push/pull planning + deterministic write application ordering.
- `loop_resume`: checkpoint pending-write replay without re-executing tasks.
- `checkpoint`: Python-format projection/read normalization + metadata merge contracts.
- `config_async`: durability `async` persistence ordering and flush parity.
- `interruption`: interrupt gate behavior across resume cycles.
- `large_cases`: deterministic multi-task push/pull scenario parity.
- `retry`: retryable failure handling parity.
- `serde_allowlist`: checkpoint metadata serialization filtering parity.
- `state_graph`: command + conditional routing and conditional-entry parity.
- `streaming`: `custom` and `messages` stream chunk parity.
