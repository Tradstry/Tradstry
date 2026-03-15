# `runtime` Purpose

This folder executes compiled graphs using a Pregel-style superstep loop.

## Responsibilities
- Drive tick/after-tick lifecycle.
- Coordinate concurrency, retries, and task completion.
- Emit stream events and enforce interrupt behavior.

## Status
- `loop`: implemented deterministic loop engine with command input mapping, pending-write replay,
  same-step send-push acceptance, and durability modes (`sync` / `async` / `exit`).
- `runner`: implemented retry-policy aware task execution with concurrent progressive API, parent-command bubbling, and writes-oriented cache memoization.
- `interrupts`: implemented selector/policy helpers with update-since-last-interrupt gating.
- `streaming`: implemented structured stream event model + sink trait, including interrupt/resume and task-cache lifecycle events.
