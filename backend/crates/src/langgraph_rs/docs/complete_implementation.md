# complete_implementation

## Scope
This document compares the current Rust `langgraph_rs` implementation with Python LangGraph behavior and identifies:
1. What is missing in Rust that exists in Python.
2. What should be improved in Rust based on Python runtime/channel behavior.

Comparison baseline in this repo:
- Rust: `src/langgraph_rs/**`
- Python: `langgraph/libs/langgraph/langgraph/**` and `langgraph/libs/checkpoint/langgraph/checkpoint/**`

---

## Executive Summary
`langgraph_rs` now has a solid deterministic parity kernel for channels, scheduler push/pull planning, and runtime-loop command/pending-write/push flow. The biggest remaining gaps are:
- streaming/output payload parity (`values`/`messages`/`custom`),
- nested-graph command/interrupt bubbling semantics,
- full Python async runner cancellation/timeout orchestration semantics.

---

## File-by-File Parity Matrix

### Channels -> done
| Rust | Python | Status | Missing / Improvement |
|---|---|---|---|
| `src/langgraph_rs/core/channels/mod.rs` | `langgraph/libs/langgraph/langgraph/channels/__init__.py` | Implemented | Exports include `AnyValue`, `UntrackedValue`, `BinaryOperatorAggregate`. |
| `src/langgraph_rs/core/channels/last_value.rs` | `.../channels/last_value.py` | Mostly parity | Core behavior matches with parity-style invalid-update diagnostics. |
| `src/langgraph_rs/core/channels/ephemeral.rs` | `.../channels/ephemeral_value.py` | Mostly parity | Behavior matches. Ensure consistent checkpoint empty sentinel behavior when adding full Python compatibility layer. |
| `src/langgraph_rs/core/channels/topic.rs` | `.../channels/topic.py` | Mostly parity | Core flatten/accumulate behavior matches; includes legacy tuple-like checkpoint restore branch. |
| `src/langgraph_rs/core/channels/barrier.rs` | `.../channels/named_barrier_value.py` | Mostly parity | Core fan-in behavior matches. Keep checkpoint format compatibility in mind if cross-language checkpoint restore is needed. |
| `src/langgraph_rs/core/channels/any_value.rs` | `.../channels/any_value.py` | Implemented | `AnyValue` semantics implemented with clear-on-empty update and checkpoint round-trip tests. |
| `src/langgraph_rs/core/channels/untracked_value.rs` | `.../channels/untracked_value.py` | Implemented | Non-checkpointed channel + guard semantics implemented; runtime persistence filtering/sanitization integrated. |
| `src/langgraph_rs/core/channels/binop.rs` | `.../channels/binop.py` | Implemented | `BinaryOperatorAggregate` implemented with overwrite parity (`{"__overwrite__": ...}` and typed helper support). |

### Scheduler / Task Planning <- done
| Rust | Python | Status | Missing / Improvement |
|---|---|---|---|
| `src/langgraph_rs/core/scheduler/apply.rs` | `langgraph/libs/langgraph/langgraph/pregel/_algo.py::apply_writes` | Implemented | Reserved channel parity includes `__pregel_push`; deterministic channel update/version behavior covered by tests. |
| `src/langgraph_rs/core/scheduler/plan.rs` | `.../pregel/_algo.py::prepare_next_tasks` | Implemented | Push (`Send`) + pull planning implemented via `plan_next_tasks_detailed` with deterministic ordering and compatibility wrapper. |
| `src/langgraph_rs/core/scheduler/state.rs` | `.../pregel/_algo.py`, `.../types.py` | Implemented | Planned-task model supports pull and push-send task kinds with deterministic path/id metadata. |

### Runtime Loop <- done
| Rust | Python | Status | Missing / Improvement |
|---|---|---|---|
| `src/langgraph_rs/runtime/loop/engine.rs` | `langgraph/libs/langgraph/langgraph/pregel/_loop.py` | Implemented | `LoopInput::Command` path with command routing (`goto`, `resume`, `update`) via runtime IO mapping. |
| `src/langgraph_rs/runtime/loop/engine.rs` | `.../pregel/_loop.py` | Implemented | Restores `CheckpointTuple.pending_writes`, applies null-task writes on resume/input, and replays task-scoped writes by task id. |
| `src/langgraph_rs/runtime/loop/engine.rs` | `.../pregel/_loop.py` | Implemented | Same-superstep push acceptance for `SendPacket` outputs with deterministic push task ids/paths and queue scheduling. |
| `src/langgraph_rs/runtime/loop/types.rs` | `langgraph/libs/langgraph/langgraph/types.py` | Implemented | `DurabilityMode` (`sync` / `async` / `exit`) and config wiring are available and tested. |
| `src/langgraph_rs/runtime/interrupts/policy.rs` | `.../pregel/_algo.py::should_interrupt` | Implemented | Update-since-last-interrupt gate is implemented and used by loop before/after interrupt checks. |
| `src/langgraph_rs/runtime/loop/engine.rs` | `.../pregel/_loop.py::_suppress_interrupt` | Partial | Added final-output snapshot and durability-aware final flush path; nested graph suppression parity is still future work. |

### Runner / Retry / Concurrency <- done
| Rust | Python | Status | Missing / Improvement |
|---|---|---|---|
| `src/langgraph_rs/runtime/runner/engine.rs` | `langgraph/libs/langgraph/langgraph/pregel/_runner.py` | Improved | Added concurrent progressive execution API (`execute_many_progressive`) with bounded in-flight execution and same-step dynamic task injection support from loop callbacks. |
| `src/langgraph_rs/runtime/runner/engine.rs` | `.../pregel/_retry.py` | Improved | Added ordered first-match retry policy resolution with `retry_on`, backoff, max interval, and jitter. |
| `src/langgraph_rs/runtime/runner/types.rs` | `.../types.py::RetryPolicy` | Implemented | Added first-class `RetryPolicy` + `RetryOn`; `with_retry_limit(...)` remains compatibility shim. |
| `src/langgraph_rs/runtime/runner/engine.rs` + `src/langgraph_rs/runtime/loop/engine.rs` | `.../_retry.py` | Improved | Added parent-command bubbling via `RunnerError::ParentCommand` -> `LoopError::ParentCommand`; retry context now sets `ExecutionContext.resuming` for attempts > 1. |
| `src/langgraph_rs/runtime/runner/engine.rs` + `src/langgraph_rs/cache/base/types.rs` | `.../_runner.py` + `.../_loop.py` cache path | Improved | Cache key no longer couples task id/step; cache payload is writes-oriented and success-only, with compatibility decode for legacy envelopes. |

### Command / IO Mapping <- done
| Rust | Python | Status | Missing / Improvement |
|---|---|---|---|
| `src/langgraph_rs/core/types/command.rs` + `src/langgraph_rs/runtime/loop/engine.rs` | `langgraph/libs/langgraph/langgraph/types.py::Command` | Implemented (root graph) | Root-graph command ingestion (`update`, `goto`, `resume`) is wired. Parent-graph loop-input routing is explicitly deferred (typed rejection maintained). |
| `src/langgraph_rs/runtime/io/map.rs` | `langgraph/libs/langgraph/langgraph/pregel/_io.py` | Implemented (dual-mode parity) | Includes `map_command`, `map_input_writes`, `map_output_values`, and `map_output_updates`; loop can emit Python-compatible `values`/`updates` chunks in dual stream mode while preserving runtime lifecycle events. |
| `src/langgraph_rs/core/types/interrupt.rs` | `langgraph/libs/langgraph/langgraph/types.py::Interrupt` | Improved | Supports stable namespace-derived IDs (`interrupt_id_from_namespace`) with UUID fallback (`new_with_namespace(None)`) for compatibility where namespace context is absent. |

### Checkpointing <- done
| Rust | Python | Status | Missing / Improvement |
|---|---|---|---|
| `src/langgraph_rs/checkpoint/base/types.rs` (`CHECKPOINT_FORMAT_VERSION=1`) | `langgraph/libs/checkpoint/.../base/__init__.py` (`LATEST_VERSION=2`) and `langgraph/libs/langgraph/langgraph/pregel/_checkpoint.py` (`LATEST_VERSION=4`) | Implemented (policy-based interop) | Checkpoint read normalization accepts v1/v2/v4; write format is configurable (`RustV1` default, opt-in `PythonV2`/`PythonV4`). |
| `src/langgraph_rs/checkpoint/base/id.rs` | Python `uuid6` usage in checkpoint modules | Implemented (configurable) | Legacy monotonic IDs remain default; `CheckpointIdStrategy::Uuid6` is available and wired through config-aware checkpoint creation. |
| `src/langgraph_rs/checkpoint/base/types.rs::get_serializable_checkpoint_metadata` | Python `get_checkpoint_metadata` + `get_serializable_checkpoint_metadata` | Implemented | Metadata merge now includes `config.metadata` + `config.configurable` fill-only semantics, excluded/`__*` key filtering, scalar-only normalization, null-byte sanitization, and `writes` removal. |
| `src/langgraph_rs/runtime/loop/engine.rs` + savers | `.../pregel/_loop.py` + checkpoint put ordering | Implemented (worker queue) | Durability modes (`sync` / `async` / `exit`) persist checkpoint->writes ordering; async mode now uses a scoped background worker with FIFO processing and guaranteed flush before run return. |

### Streaming / Output Semantics <- done (node-message scope)
| Rust | Python | Status | Missing / Improvement |
|---|---|---|---|
| `src/langgraph_rs/runtime/streaming/event.rs` + `src/langgraph_rs/runtime/io/map.rs` + loop | `.../pregel/_loop.py` + `.../pregel/_io.py` stream outputs | Implemented (additive parity mode) | Dual parity mode now supports Python-compatible `values`, `updates`, `tasks`, `checkpoints`, and `debug` wrapper payloads while preserving structured runtime lifecycle events. |
| `src/langgraph_rs/runtime/streaming/event.rs` + loop | Python stream modes (`values`, `updates`, `tasks`, `checkpoints`, `debug`, `messages`, `custom`) | Implemented (node-level scope) | Parity mode selection is configurable via `LoopConfig.parity_stream_modes`; `messages` is currently node-result event based (token callback plumbing remains deferred). |
| `src/langgraph_rs/core/types/execution.rs` (`custom_events`) | Python stream writer/custom mode | Implemented | Loop emits `NodeExecutionResult.custom_events` in `StreamMode::Custom` as raw payloads, preserving completion order. |

### Graph API / StateGraph Layer <- done (additive v1)
| Rust | Python | Status | Missing / Improvement |
|---|---|---|---|
| `src/langgraph_rs/core/graph/state.rs`, `state_schema.rs`, `state_compiled.rs`, `managed.rs` | `langgraph/libs/langgraph/langgraph/graph/state.py`, `pregel/main.py` | Implemented (runtime-schema v1) | Added `StateGraph`, `CompiledStateGraph`, `StateSchema`, field-level reducer/channel binding, direct/conditional routing, and managed built-ins injection while preserving low-level `GraphBuilder` API. |
| `src/langgraph_rs/core/graph/state_compiled.rs` + runtime loop/scheduler read-channel support | Python conditional routing + runtime command/goto/send machinery | Implemented (root graph parity) | Node-return command routing and conditional branch path routing coexist with deterministic dedupe/order. Parent-graph nested routing remains explicitly deferred. |

### Managed Values / Runtime Context <- done
| Rust | Python | Status | Missing / Improvement |
|---|---|---|---|
| `src/langgraph_rs/core/managed/**` + `src/langgraph_rs/core/graph/managed.rs` | `langgraph/libs/langgraph/langgraph/managed/**` | Implemented (built-in scope) | Managed built-ins (`is_last_step`, `remaining_steps`) are available and injected into node input via scheduler read channels; managed values remain non-checkpointed. |
| `src/langgraph_rs/core/types/execution.rs` + runtime loop task context | Python runtime context/configurable injection (`__pregel_read`, `__pregel_send`, `__pregel_call`, etc.) | Implemented (root runtime scope) | Runtime capabilities are injected through `ExecutionContext` and exercised by loop tests; nested parent-graph routing/capabilities are still deferred. |

### Parity Test Coverage <- done
| Rust | Python | Status | Missing / Improvement | 
|---|---|---|---|
| `src/langgraph_rs/parity_tests/*.rs` | Python behavior across channels/pregel/checkpoint | Improved | Parity tests now cover channels, scheduler push/pull planning, checkpoint interop/metadata, and pending-write replay; cross-runtime golden fixture comparison remains future work. |

---

## High-Priority Remaining Features (Implementation Order)

1. **Checkpoint interop hardening**
- Add broader mixed-version backend fixtures (v1/v2/v4 in same store).
- Add migration docs/tooling for existing deployments moving to interop write modes.

2. **Nested graph/parent command parity**
- Implement parent-graph command bubbling and nested suppression semantics.

3. **Async runner parity**
- Align timeout/cancellation/panic-on-failure orchestration with Python async runner behavior.

---

## Rust Improvements (Beyond Strict Parity)

1. **Centralize constants in one Rust module**
- Mirror Python internal constants (`TASKS`, `RESUME`, `INTERRUPT`, `RETURN`, `PUSH`, `PULL`, etc.).
- Avoid divergent literals across scheduler/checkpoint/runtime.

2. **Harden checkpoint compatibility rollout**
- Add migration playbooks for switching write format/ID strategy per environment.
- Add backend conformance tests for mixed legacy + interop checkpoint inventories.

3. **Strengthen typed error taxonomy**
- Add parity-aligned invalid update/concurrency error categories and clearer migration diagnostics.

4. **Harden deterministic ordering contracts**
- Ensure path sort semantics and task-id derivation are stable under push/pull mixed workloads and retries.

5. **Build real parity CI suite**
- Golden tests that execute equivalent Python and Rust scenarios and compare terminal state/events/checkpoint invariants.

---

## Suggested Acceptance Criteria

### Channels
- `AnyValue`, `UntrackedValue`, `BinaryOperatorAggregate` pass behavior-equivalent tests vs Python cases.

### Scheduler/Loop
- `Send` packets execute either same-step (`accept_push`) or via planned tasks from `__pregel_tasks` on subsequent planning/replay paths.
- `Command(resume/goto/update)` affects graph state exactly as expected.
- Pending writes replay correctly after resume.

### Interrupt/Retry
- Interrupt behavior matches Python for updated-since-last-interrupt gate.
- Retry policy supports backoff/jitter/retry_on semantics.

### Checkpoint/Streaming
- Durability modes (`sync`/`async`/`exit`) implemented and tested.
- Stream payloads support parity modes and include custom events.

### Tests
- `src/langgraph_rs/parity_tests` contains executable tests for all above categories in CI.

---

## Notes on Current Strengths in Rust
- Deterministic scheduler apply/planning foundation is solid.
- Storage backends (memory/sqlite/postgres) for cache/checkpoint/store are already substantial.
- Runtime event model is clean and strongly typed.

The next milestone should focus on **runner + streaming parity depth**, where behavioral drift from Python is currently highest.

---

## Strict Python -> Rust Test File Matrix (as of 2026-03-04)

Legend:
- `exact`: direct equivalent behavior is covered by Rust tests.
- `partial`: related behavior is tested in Rust, but not at full Python suite depth.
- `missing`: no meaningful Rust equivalent test coverage yet.

### `langgraph/libs/langgraph/tests/*`
| Python test file | Rust equivalent test files | Coverage | Notes |
|---|---|---|---|
| `test_algo.py` | `src/langgraph_rs/core/scheduler/apply.rs`, `src/langgraph_rs/core/scheduler/plan.rs`, `src/langgraph_rs/parity_tests/scheduler.rs` | partial | Scheduler/apply planning is covered; full Pregel algorithm suite depth still differs. |
| `test_channels.py` | `src/langgraph_rs/core/channels/*.rs`, `src/langgraph_rs/parity_tests/any_value.rs`, `src/langgraph_rs/parity_tests/binop.rs`, `src/langgraph_rs/parity_tests/untracked_value.rs` | partial | Major channel behaviors are tested; Python suite breadth is wider. |
| `test_checkpoint_migration.py` | `src/langgraph_rs/checkpoint/base/types.rs`, `src/langgraph_rs/parity_tests/checkpoint.rs` | partial | Interop version normalization exists; full migration fixture coverage still limited. |
| `test_config_async.py` | `src/langgraph_rs/runtime/loop/engine.rs`, `src/langgraph_rs/runtime/runner/engine.rs`, `src/langgraph_rs/parity_tests/config_async.rs` | partial | Dedicated durability-async ordering parity test now exists; broader async orchestration parity is still limited. |
| `test_deprecation.py` | - | missing | No Rust deprecation test suite equivalent. |
| `test_interrupt_migration.py` | `src/langgraph_rs/core/types/interrupt.rs`, `src/langgraph_rs/runtime/loop/engine.rs` | partial | Interrupt semantics are tested; migration-specific parity coverage is limited. |
| `test_interruption.py` | `src/langgraph_rs/runtime/interrupts/policy.rs`, `src/langgraph_rs/runtime/interrupts/selector.rs`, `src/langgraph_rs/runtime/loop/engine.rs`, `src/langgraph_rs/parity_tests/interruption.rs` | partial | Core before/after interruption flow is covered, including parity gate behavior across resume cycles. |
| `test_large_cases.py` | `src/langgraph_rs/parity_tests/large_cases.rs` | partial | Added deterministic multi-task push/pull parity scenario. |
| `test_large_cases_async.py` | - | missing | No async large-case golden scenario suite equivalent. |
| `test_managed_values.py` | `src/langgraph_rs/core/managed/mod.rs`, `src/langgraph_rs/core/graph/state_compiled.rs`, `src/langgraph_rs/runtime/loop/engine.rs` | partial | Managed value injection is covered; Python suite includes broader end-to-end variants. |
| `test_messages_state.py` | `src/langgraph_rs/runtime/streaming/event.rs`, `src/langgraph_rs/runtime/loop/engine.rs`, `src/langgraph_rs/parity_tests/streaming.rs` | partial | Message/custom stream payload behavior now has explicit parity coverage. |
| `test_parent_command.py` | `src/langgraph_rs/runtime/runner/engine.rs`, `src/langgraph_rs/runtime/loop/engine.rs` | partial | Parent-command bubbling/rejection tests exist, nested parity still incomplete. |
| `test_parent_command_async.py` | `src/langgraph_rs/runtime/runner/engine.rs`, `src/langgraph_rs/runtime/loop/engine.rs` | missing | No async-specific parent-command parity suite. |
| `test_pregel.py` | `src/langgraph_rs/runtime/loop/engine.rs`, `src/langgraph_rs/runtime/runner/engine.rs`, `src/langgraph_rs/parity_tests/scheduler.rs`, `src/langgraph_rs/parity_tests/loop_resume.rs`, `src/langgraph_rs/parity_tests/large_cases.rs` | partial | Core loop/runner semantics are covered with added large-case parity; full Python Pregel fixture scope still exceeds current suite. |
| `test_pregel_async.py` | `src/langgraph_rs/runtime/loop/engine.rs`, `src/langgraph_rs/runtime/runner/engine.rs` | missing | No explicit async Pregel parity suite. |
| `test_pydantic.py` | - | missing | No Rust equivalent for Python/Pydantic validation coverage. |
| `test_remote_graph.py` | - | missing | No remote-graph parity suite in Rust. |
| `test_retry.py` | `src/langgraph_rs/runtime/runner/retry.rs`, `src/langgraph_rs/runtime/runner/engine.rs`, `src/langgraph_rs/runtime/loop/engine.rs`, `src/langgraph_rs/parity_tests/retry.rs` | partial | Retry/backoff behavior is tested with dedicated parity coverage; Python edge-case breadth is still larger. |
| `test_runnable.py` | - | missing | No runnable API parity suite equivalent. |
| `test_runtime.py` | `src/langgraph_rs/runtime/loop/engine.rs`, `src/langgraph_rs/runtime/io/map.rs`, `src/langgraph_rs/runtime/streaming/emitter.rs`, `src/langgraph_rs/parity_tests/streaming.rs`, `src/langgraph_rs/parity_tests/config_async.rs` | partial | Runtime lifecycle/events are heavily tested with additional parity cases for streaming and async durability; Python runtime suite is still broader. |
| `test_serde_allowlist.py` | `src/langgraph_rs/parity_tests/serde_allowlist.rs` | partial | Added dedicated allowlist-style metadata serialization parity coverage. |
| `test_state.py` | `src/langgraph_rs/core/graph/state.rs`, `src/langgraph_rs/core/graph/state_schema.rs`, `src/langgraph_rs/core/graph/state_compiled.rs`, `src/langgraph_rs/parity_tests/state_graph.rs` | partial | StateGraph compile/route behavior now has dedicated parity tests; Python state edge-cases remain broader. |
| `test_subgraph_persistence.py` | `src/langgraph_rs/runtime/loop/engine.rs`, `src/langgraph_rs/checkpoint/*/saver.rs` | missing | Subgraph persistence parity is explicitly deferred in Rust docs. |
| `test_subgraph_persistence_async.py` | - | missing | No async subgraph persistence parity suite. |
| `test_tracing_interops.py` | - | missing | No tracing interop parity suite equivalent. |
| `test_type_checking.py` | - | missing | No type-checking parity suite equivalent. |
| `test_utils.py` | - | missing | No direct utilities parity suite equivalent. |

### `langgraph/libs/checkpoint/tests/*`
| Python test file | Rust equivalent test files | Coverage | Notes |
|---|---|---|---|
| `test_encrypted.py` | - | missing | Rust checkpoint serde encryption parity tests not implemented. |
| `test_jsonplus.py` | - | missing | No JSONPlus serializer parity suite in Rust. |
| `test_memory.py` | `src/langgraph_rs/checkpoint/memory/saver.rs`, `src/langgraph_rs/checkpoint/conformance.rs` | partial | Memory saver behavior covered, but not Python serializer-specific depth. |
| `test_redis_cache.py` | - | missing | No Redis cache backend tests in Rust tree. |
| `test_store.py` | `src/langgraph_rs/store/base/store.rs`, `src/langgraph_rs/store/conformance.rs`, `src/langgraph_rs/store/memory/store.rs` | partial | Core store behavior covered; suite structure and API shape differ. |

### `langgraph/libs/checkpoint-postgres/tests/*`
| Python test file | Rust equivalent test files | Coverage | Notes |
|---|---|---|---|
| `test_async.py` | `src/langgraph_rs/checkpoint/postgres/saver.rs` | partial | Postgres saver tested; no dedicated async Postgres saver parity suite. |
| `test_async_store.py` | `src/langgraph_rs/store/postgres/store.rs`, `src/langgraph_rs/store/base/store.rs` | partial | Async trait wrappers exist; parity with Python async store tests is incomplete. |
| `test_store.py` | `src/langgraph_rs/store/postgres/store.rs`, `src/langgraph_rs/store/conformance.rs` | partial | Postgres store operations are tested, suite depth differs. |
| `test_sync.py` | `src/langgraph_rs/checkpoint/postgres/saver.rs`, `src/langgraph_rs/checkpoint/conformance.rs` | partial | Sync checkpoint operations are covered, not as broad as Python sync suite. |

### `langgraph/libs/checkpoint-sqlite/tests/*`
| Python test file | Rust equivalent test files | Coverage | Notes |
|---|---|---|---|
| `test_aiosqlite.py` | `src/langgraph_rs/checkpoint/sqlite/saver.rs` | missing | No aiosqlite-style async parity suite. |
| `test_async_store.py` | `src/langgraph_rs/store/sqlite/store.rs`, `src/langgraph_rs/store/base/store.rs` | partial | Async trait wrappers exist; no dedicated async sqlite store parity suite. |
| `test_sqlite.py` | `src/langgraph_rs/checkpoint/sqlite/saver.rs`, `src/langgraph_rs/checkpoint/conformance.rs` | partial | SQLite checkpoint behavior is tested, but Python suite depth is broader. |
| `test_store.py` | `src/langgraph_rs/store/sqlite/store.rs`, `src/langgraph_rs/store/conformance.rs` | partial | SQLite store behavior is tested with a narrower suite. |
| `test_ttl.py` | `src/langgraph_rs/cache/sqlite/cache.rs`, `src/langgraph_rs/cache/memory/cache.rs`, `src/langgraph_rs/cache/conformance.rs` | partial | TTL behavior exists in cache layer; no direct checkpoint-sqlite TTL parity suite. |

### `langgraph/libs/prebuilt/tests/*`
| Python test file | Rust equivalent test files | Coverage | Notes |
|---|---|---|---|
| `test_deprecation.py` | - | missing | No prebuilt package/test surface in Rust tree. |
| `test_on_tool_call.py` | - | missing | No prebuilt tool-calling parity suite. |
| `test_react_agent.py` | - | missing | No prebuilt React agent layer in Rust tree. |
| `test_react_agent_graph.py` | - | missing | No prebuilt agent graph parity tests. |
| `test_tool_node.py` | - | missing | No prebuilt ToolNode parity suite. |
| `test_tool_node_interceptor_unregistered.py` | - | missing | Missing with prebuilt surface. |
| `test_tool_node_validation_error_filtering.py` | - | missing | Missing with prebuilt surface. |
| `test_validation_node.py` | - | missing | Missing with prebuilt surface. |

### `langgraph/libs/sdk-py/tests/*`
| Python test file | Rust equivalent test files | Coverage | Notes |
|---|---|---|---|
| `test_api_parity.py` | - | missing | No Rust SDK parity suite in this repo tree. |
| `test_assistants_client.py` | - | missing | No assistants client test surface in Rust tree. |
| `test_client_exports.py` | - | missing | No SDK export parity suite in Rust tree. |
| `test_client_stream.py` | - | missing | No SDK client streaming parity suite in Rust tree. |
| `test_crons_client.py` | - | missing | No SDK crons client parity suite. |
| `test_encryption.py` | - | missing | No SDK encryption test suite. |
| `test_errors.py` | - | missing | No SDK error-surface parity suite. |
| `test_serde.py` | - | missing | No SDK serde parity suite. |
| `test_serde_schema.py` | - | missing | No SDK schema parity suite. |
| `test_skip_auto_load_api_key.py` | - | missing | No SDK config/env parity suite. |

### `langgraph/libs/cli/tests/*` and JS example tests
| Python/JS test file | Rust equivalent test files | Coverage | Notes |
|---|---|---|---|
| `langgraph/libs/cli/tests/integration_tests/test_cli.py` | - | missing | No Rust CLI package parity suite in this repo tree. |
| `langgraph/libs/cli/tests/unit_tests/cli/test_cli.py` | - | missing | Missing with CLI surface. |
| `langgraph/libs/cli/tests/unit_tests/cli/test_templates.py` | - | missing | Missing with CLI surface. |
| `langgraph/libs/cli/tests/unit_tests/test_config.py` | - | missing | Missing with CLI surface. |
| `langgraph/libs/cli/tests/unit_tests/test_docker.py` | - | missing | Missing with CLI surface. |
| `langgraph/libs/cli/tests/unit_tests/test_util.py` | - | missing | Missing with CLI surface. |
| `langgraph/libs/cli/js-examples/tests/agent.test.ts` | - | missing | No JS CLI example parity tests in Rust tree. |
| `langgraph/libs/cli/js-examples/tests/graph.int.test.ts` | - | missing | No JS CLI example parity tests in Rust tree. |
