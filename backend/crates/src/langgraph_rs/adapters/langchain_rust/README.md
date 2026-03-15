# `adapters/langchain_rust` Purpose

This folder provides `langchain-rust`-oriented adapter constructors for runtime nodes.

## Responsibilities
- Adapt `Chain`-style execution into graph task contracts.
- Map memory/tool outputs into runtime state updates.
- Contain compatibility glue for crate-specific data formats.

## File Map

### `mod.rs`
- Public export surface for LangChain adapter types.

### `node.rs`
- Defines `LangChainNodeAdapter` closure wrappers:
  - direct node-result handler
  - value/message handler constructors that map outputs into channel writes
  - provider-error mapping helper (`langchain-rust adapter error -> NodeExecutionError`)

## Current Coverage
- Implemented:
  - sync closure-based LangChain adapter constructors
  - output-to-channel write mapping helpers for value and message arrays
- Planned next:
  - concrete integration to selected `langchain-rust` chain/agent abstractions
