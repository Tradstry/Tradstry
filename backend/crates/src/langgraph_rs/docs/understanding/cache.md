# How cache work - Simple Explanation
Think of cache as a smart shortcul: if a node already solved the same input before, LangGraph can reuse that result instead of re-running expensive work.

Design-wise, everything depends on one backend-agnostic contract, `Cache (get/set/delete/clear/prune)` so memory, SQLite, and Postgres all behave the same from runtime's point of view `(cache.rs, memory, sqlite, postgres)`. 
Each cache item has a structured key `(namespace + key)` plus value, timestamps, TTL expiry, and metadata, which makes it easy to isolate per app/thread and expire safely `(types.rs)`.

At runtime, `TaskRunner` computes a deterministic cache key from `node + hash(canonicalled input JSON)`, checks cache first, emits hit/miss events, and on hit returns immediately without executing the node `(engine.rs, engine.rs)`.
Important design choice: they cache a normalized "writes envelope" (state updates) instead of full raw node output, so replay is deterministic and older cache formats still decode (legacy compatibility) `(types.rs)`.

So the core concepts are: abstraction boundary (trait), deterministic keying, TTL-based freshness, backend portability, and observability via `TaskCacheHit/TaskCacheMiss/TaskCacheStored` events.

