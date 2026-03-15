# `langgraph_rs` Purpose

This is the root namespace for the Rust rewrite of `langgraph/libs/langgraph`.

## Responsibilities
- Define module boundaries for core graph logic, runtime loop, checkpointing, adapters, and tests.
- Provide a clean migration target where features can land in phases without breaking the entry binary.

## Design intent
This root module will become the stable home for the LangGraph runtime kernel in Rust.
