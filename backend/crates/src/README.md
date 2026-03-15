# `src` Folder Purpose

This `src` directory will host the Rust implementation of the LangGraph runtime.

## What will stay here
- The current binary entrypoint (`main.rs`) for local experiments and bootstrap commands.
- The `langgraph_rs` module tree, which will contain the production runtime crates/modules.

## Why this exists
This top-level folder keeps all Rust runtime code in one place so migration from Python to Rust is easy to navigate and incrementally ship.

## What to implement in order 
1. `src/langgraph_rs/core/types` <- done
2. `src/langgraph_rs/core/channels` <- done
3. `src/langgraph_rs/core/scheduler` <- done
4. `src/langgraph_rs/checkpoint/base` <- done
5. `src/langgraph_rs/checkpoint/memory` <- done
6. `src/langgraph_rs/checkpoint/sqlite` <- done
7. `src/langgraph_rs/checkpoint/postgres` <- done
8. `src/langgraph_rs/runtime/loop` <- done
9. `src/langgraph_rs/runtime/runner` <- done
10. `src/langgraph_rs/runtime/interrupts` <- done
11. `src/langgraph_rs/runtime/streaming` <- done
12. `src/langgraph_rs/store` <- done
13. `src/langgraph_rs/core/graph` <- done
14. `src/langgraph_rs/cache` <- done
15. `src/langgraph_rs/adapters` (only after 1–7) <- done

