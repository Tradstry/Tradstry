# `checkpoint` Purpose

This folder defines durable checkpoint interfaces and backend implementations.

## Responsibilities
- Checkpoint model/versioning contracts.
- Saver trait definitions for sync and async operations.
- Backend implementations (memory/sqlite/postgres).

## Status
- `base`: implemented (types + trait + helpers).
- `memory`: implemented (`InMemorySaver`).
- `sqlite`: implemented (`SqliteSaver`).
- `postgres`: implemented (`PostgresSaver`).

## Conformance
- Shared backend conformance tests live in `checkpoint/conformance.rs`.
- The same behavior matrix is executed for memory/sqlite automatically.
- Postgres conformance runs when `LANGGRAPH_RS_TEST_POSTGRES_URL` is set.
