# `core` Purpose

This folder holds the deterministic, framework-agnostic graph kernel.

## Responsibilities
- Graph definitions and compiled graph structures.
- Channel abstractions and state transition rules.
- Task planning and write-application algorithms.

## Scope rule
No provider-specific code should live here.

## Status
- `channels`: implemented with deterministic state update contracts.
- `scheduler`: implemented planning/apply loop primitives.
- `types`: implemented shared runtime-native contracts.
- `graph`: implemented builder + compile + validation bridge to runtime loop.
