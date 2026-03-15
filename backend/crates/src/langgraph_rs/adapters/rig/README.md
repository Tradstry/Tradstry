# `adapters/rig` Purpose

This folder provides `rig`-focused adapter constructors for executable graph nodes.

## Responsibilities
- Wrap Rig prompts/agents/tools into runtime node handlers.
- Translate Rig outputs into runtime write tuples and stream events.
- Keep Rig-specific error and retry mapping isolated from core.

## File Map

### `mod.rs`
- Public export surface for Rig adapter types.

### `node.rs`
- Defines `RigNodeAdapter` closure wrappers:
  - direct node-result handler
  - text/value handler constructors that map provider outputs into channel writes
  - provider-error mapping helper (`rig adapter error -> NodeExecutionError`)

## Current Coverage
- Implemented:
  - sync closure-based Rig adapter constructors
  - output-to-channel write mapping helpers
- Planned next:
  - direct bindings for specific `rig-core` agent/prompt interfaces
