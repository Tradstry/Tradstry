# Tradstry Backend — End-to-End Analysis

> Latency + RAG/vector accuracy audit. Tracking doc. Nothing in here has been
> applied to code yet — each item is a finding with a proposed fix.

**Scope:** `backend/` (~49k LOC Rust). Workspace = main crate (Actix + async-graphql)
+ hand-rolled LangGraph crate + mcp-server. Data: Turso/libsql (per-user trade
journal), Postgres+pgvector (vectors + chat checkpoints), Redis (brokerage cache),
R2 (media), Voyage (embed+rerank), Gemini 3.5-flash (chat/insights).

---

## Architecture & patterns observed

- **Two databases, two access models.** Turso/libsql holds per-user
  journal/notebook/playbook data, accessed through `TursoClient::get_user_db(user_id)`
  → a fresh `Connection`. Postgres (via `sqlx::PgPool`) holds vectors + LangGraph
  checkpoints/store and is properly pooled (`client.rs:172` — `min_connections(1)`,
  `test_before_acquire`, `after_connect` NOTICE suppression). The two layers have
  opposite connection discipline.
- **GraphQL resolver pattern:** every resolver calls a local `get_user_db(ctx)`
  helper that does `get_connection()` → `ensure_user()` → `get_user_db()` again.
  89 call sites.
- **RAG indexing** (`ai/jobs.rs`): background worker leases jobs from Turso, builds
  structure-aware `Block`s → `chunk_blocks` (450/512 tok, 15% overlap) → optional
  per-doc Gemini "situating blurb" (cached in `vector_context_cache`) → deterministic
  header + blurb + raw composed into `embed_text` → one **batched** Voyage embed →
  parent/child rows upserted. Incremental reindex diffs `source_content_hash`.
- **RAG retrieval, two divergent paths:**
  - Chat `semantic_search` → `hybrid_search` (dense ANN in SQL + sparse TF in Rust
    + RRF k=60 + Voyage rerank + parent expansion). The good path.
  - Insights/report/mindset/research → `retrieve_for_queries` →
    `dense_search_documents` (dense-only, no sparse, no RRF), reranked at the end.
- **Chat = custom LangGraph** (`chat/graph.rs`): `llm` node ↔ 19 tool nodes + 3
  subgraphs, conditional routing on `current_tool_call`, recursion limit 12 (≤5 tool
  calls). Auto-compaction at 20 messages. Dispatched via `spawn_blocking`
  (`main.rs:70`).
- **LLM client** (`ai/client.rs`): single hardcoded model `gemini-3.5-flash`, native
  SSE streaming, transient retry, thought-signature threading.
- **Voyage client-side rate limiter** defaulting to 3 RPM / 10K TPM (free tier).

---

## Part A — Latency weaknesses (ranked)

### A1. 🔴 Every graph node spins up a throwaway multi-threaded Tokio runtime — this kills all warm connection pools

`chat/graph.rs:49-52` (and **28 sites** across the chat nodes + research/report/comparison subgraphs):

```rust
let rt = tokio::runtime::Runtime::new()...;
rt.block_on(async move { llm_node_async(&deps, &state).await })
```

The graph loop is synchronous (run on a `spawn_blocking` thread), and each node
execution constructs a brand-new multi-threaded runtime (worker threads = #CPUs,
fresh I/O driver + timer), runs one async op, then tears it down. A single chat turn
with 5 tool calls ≈ 10+ node executions = 10+ runtime create/destroy cycles.

The real damage isn't thread churn — it's that `reqwest::Client` and `sqlx::PgPool`
connection pools are bound to the runtime that drives their background I/O.
Connections established under one node's runtime are dead when the next node's
runtime starts. So every Gemini call, every Voyage embed, and every Postgres query
inside the graph pays a cold TCP+TLS handshake. This is almost certainly the dominant
chat-latency source the memory note flagged ("latency lives in DB round-trips / cold
connections").

**Fix:** run nodes on the ambient runtime via a single shared `Handle`
(`Handle::current().block_on()` captured once, or make the langgraph runner async).
One runtime for the whole turn → pools stay warm.

### A2. ✅ ADDRESSED — Turso opens a fresh remote connection (+ a PRAGMA round-trip) on every `get_user_db`, with no pooling

> **Status (2026-05-29):** Addressed via embedded-replica migration. `TursoClient`
> now runs a libsql embedded replica (local-file reads, remote writes) behind a
> hardcoded toggle; `get_connection()`/`get_user_db()` and the per-call `PRAGMA` now
> hit the local file (µs) instead of the network, so the per-request connection +
> PRAGMA round-trip cost is eliminated without touching the 89 call sites. Backend +
> MCP server both migrated; MCP syncs on-read + every 60s. Code merged to the working
> tree (build/clippy/tests green); VPS host-dir creation + prod validation pending.
> Design: `docs/superpowers/specs/2026-05-29-turso-embedded-replica-design.md`.
> Plan: `docs/superpowers/plans/2026-05-29-turso-embedded-replica.md`.
> The connection-reuse refactor (A2 fix (a)–(c)) is now largely unnecessary since
> reads are local; revisit only if profiling still shows per-call overhead.

`turso/client.rs:69-81`:

```rust
pub fn get_connection(&self) -> Result<Connection> { self.db.connect() ... }
pub async fn get_user_db(&self, user_id: &str) -> Result<UserDb> {
    let conn = self.get_connection()?;
    conn.execute("PRAGMA foreign_keys = ON", ...).await?;  // extra remote round-trip
    Ok(UserDb::new(conn, ...))
}
```

`Builder::new_remote` (not `new_remote_replica`) means remote-only — every `.connect()`
is a network session, and every `get_user_db` adds a `PRAGMA` round-trip before any
real query. The GraphQL helper compounds it: `get_connection()` for `ensure_user`
then `get_user_db()` again = 2 connections + an `ensure_user` write per authenticated
request, before business logic.

**Fixes:** (a) cache/reuse a connection per `TursoClient` (libsql `Connection` is
cheap to clone and multiplexes); (b) drop the per-call `PRAGMA` (set once on the
cached connection); (c) collapse the double-connect; (d) strongly consider
`new_remote_replica` (embedded replica) — local reads at memory speed, async
write-back — which would erase most read latency entirely. Worth its own decision
(see "Turso embedded replica evaluation" below).

### A3. 🔴 N+1 on the `tags` field resolver

`graphql/journal.rs` `#[ComplexObject] tags()` calls `get_user_db(ctx)` per
`JournalEntry`. A query for 100 trades + their tags = 1 list query + 100 tag queries,
each opening a fresh Turso connection (A2). With remote RTT this is tens of seconds.

**Fix:** async-graphql `DataLoader` to batch tag lookups into one
`tags_for_trades(ids)` query (the batch function already exists — used in `jobs.rs:241`).

### A4. 🟠 `retrieve_for_queries` embeds queries sequentially through a rate-limited client

`ai/jobs.rs:1054-1095`: loops 3 queries, each `embed_text(query).await` sequentially,
then `dense_search_documents` sequentially. With the Voyage limiter at 3 RPM (A6),
three sequential embeds can serialize into up to ~60s of pure sleeping.

**Fix:** one batched `embed_texts([q1,q2,q3])` call (1 request instead of 3), then
`tokio::try_join!` / `futures::join_all` the searches.

### A5. 🟠 Sequential awaits in hot read/sync paths

- `read_service/analytics.rs`: aggregate → biggest_win → biggest_loss run serially
  though the two extremes are independent (`try_join!`).
- `brokerage/sync.rs`: `sync_transactions().await` then `sync_holdings().await` per
  account, serially across all accounts.
- `graphql/notebook.rs`: R2 presign loop and R2 delete loop `.await` one object at a
  time (`join_all` them).

### A6. 🟠 Voyage limiter at 3 RPM / 10K TPM is a latency cliff on the live path

`vector_database/client.rs:24-25`. Every chat `semantic_search` and every chat turn's
memory retrieval (`agent.rs:165` → embeds the user message) calls Voyage. At 3 RPM the
limiter (`acquire`) can sleep the request for tens of seconds. This is config, but it
gates the whole live RAG path — flagging because it's invisible until traffic hits it.

**Action:** confirm the paid tier (2000 RPM) is set in prod, and treat the constants
as the real ceiling.

### A7. 🟡 Memory retrieval added to every single chat turn

`agent.rs:163-204` does an embed + vector search on every message before the LLM
starts (serial, in the request path). Fine when warm, but stacks on A1+A6. Consider
gating it or running it concurrently with graph setup.

---

## Part B — RAG / vector accuracy weaknesses (web-researched, cited)

### B1. 🔴 `hnsw.ef_search` is never set → default 40 silently caps recall

You `ORDER BY dense <=> $1 LIMIT prefetch` where `prefetch = top_k*4` (`client.rs:768`).
Per Crunchy Data, with default `ef_search=40` the index cannot return more than 40
rows, and recall degrades well before that. Whenever `top_k ≥ 10`, your prefetch (≥40)
is truncated. **Fix:** `SET hnsw.ef_search = 100–200` per search session (or via
`after_connect`). Highest-ROI accuracy fix, ~zero cost.
[crunchydata.com/blog/hnsw-indexes-with-postgres-and-pgvector, github.com/pgvector/pgvector]

### B2. 🔴 Per-tenant filter + HNSW = post-filter recall cliff

`hybrid_search`/`dense_search_documents` filter `user_id=$ AND account_id=$` after the
ANN scan. pgvector filters post-scan, so HNSW finds 40 global-nearest, then the WHERE
drops most → you can get far fewer than `top_k`, even zero, for users who are a small
fraction of the table. **Fix:** enable pgvector 0.8 iterative scan —
`SET hnsw.iterative_scan = relaxed_order` (+ raise `hnsw.max_scan_tuples`,
`hnsw.scan_mem_multiplier`); AWS reports up to 100× completeness on selective filters.
At scale, partition by tenant.
[aws.amazon.com/blogs/database/...pgvector-0-8-0..., pgvector README]

### B3. 🔴 The "hybrid" search is dense-retrieval-then-sparse-reorder, not true hybrid

`fuse_rrf` only ever sees the dense top-N candidates (the SQL `ORDER BY dense`). A
chunk that's a strong lexical match but mediocre dense match never enters the candidate
set, so sparse/RRF can't rescue it. True hybrid unions dense-top-N and sparse-top-N
before fusing. Combined with B4, your lexical recall is structurally limited.

### B4. 🔴 The sparse vector (FNV-hashed term-frequency) is the weakest link

`sparse.rs`: raw TF, FNV-hashed into u32. It lacks IDF (rare discriminating terms don't
outweigh common ones), TF saturation, and length normalization — the three things that
make BM25 work — plus hash collisions conflate tokens. Also `text_to_sparse_vector`
returns weights but `hybrid_search` discards query weights (`_query_val`) and matches
presence-only (`sparse_dot_with_set`). **Fix:** replace with real BM25 — ParadeDB
`pg_search` (native BM25 index, coexists with pgvector). Anthropic's Contextual
Retrieval shows Contextual-Embeddings + Contextual-BM25 cuts retrieval failures
35%→49%; you're leaving that gain on the table.
[paradedb.com/blog/hybrid-search-in-postgresql, anthropic.com/news/contextual-retrieval]

### B5. 🟠 Contextual blurb is applied to the embedding but not to the lexical/sparse text

`context.rs::compose_embedded_text` enriches the embedded text, and sparse is computed
from that same `embed_text` (`upsert_documents:576`) — good — but since the sparse
engine itself is weak (B4), the contextual-BM25 benefit isn't realized. Fix lands
together with B4: feed the contextualized chunk text to a real BM25 index.

### B6. 🟠 Reranker is fed too few candidates

`hybrid_search` reranks only the fused `prefetch = top_k*4` set. Voyage's own rerank-2.5
methodology and Anthropic's pipeline use retrieve ~100–150 → rerank → top ~10–20. With
small `top_k`, you feed the reranker ~32–80; widen the prefetch floor to ~100–150 so
rerank-2.5 has real signal. rerank-2.5 handles 32K context / ≤1000 docs.
[docs.voyageai.com/docs/reranker, anthropic.com/news/contextual-retrieval]

### B7. 🟠 `chars/4` token estimate mis-sizes chunks

`chunking.rs::estimate_tokens` = `chars/4`. Real ratio drifts with
numbers/punctuation/code (your trade bodies are number-dense), so your "512 max" isn't
really 512 — inconsistent chunk content hurts retrieval consistency and risks silent
truncation. **Fix:** use Voyage's tokenizer / `/tokenize` endpoint (or tiktoken `o200k`
as a fast proxy). Cheap relative to embedding cost. [docs.voyageai.com/docs/tokenization]

### B8. 🟡 Two retrieval paths with different quality

Chat uses full `hybrid_search`; insights/report/mindset/research use dense-only
`dense_search_documents` (`jobs.rs:1063`). Your highest-value artifacts (the AI reports)
run on the weaker retriever. **Fix:** route them through `hybrid_search` too.

### B9. 🟡 HNSW build params are low for 2048-dim

`m=16`, `ef_construction=64` (defaults). For 2048-dim, research suggests `m=24–48`,
`ef_construction=128–256`, with `maintenance_work_mem` sized to hold the graph +
parallel build. Keep halfvec (fp16 is effectively free on recall) and 2048-dim, and
keep the 450/512 @15% chunking and RRF k=60 — all confirmed good.
[pgvector README, voyageai halfvec/Matryoshka docs]

---

## Suggested sequencing

**Tier 1 — biggest latency, lowest risk**
1. A1: single shared runtime `Handle` for graph nodes (kills cold-connection tax everywhere in chat).
2. A2: reuse one Turso connection + drop per-call `PRAGMA` + collapse double-connect.
3. B1: `SET hnsw.ef_search = 150` + B2: `iterative_scan = relaxed_order` (few lines in `after_connect`/per-query; large recall win).

**Tier 2 — structural**
4. A3: DataLoader for `tags`.
5. A4/A5: batch embeds + `try_join!` the sequential awaits.
6. B6/B8: widen rerank prefetch + route AI artifacts through `hybrid_search`.

**Tier 3 — deeper accuracy**
7. B4/B3/B5: replace FNV-TF with ParadeDB `pg_search` BM25 + true union hybrid + contextual-BM25.
8. B7: real tokenizer.
9. B9: rebuild HNSW with tuned params.
10. A2 stretch: evaluate embedded-replica Turso (see below).

---

## Turso embedded replica (`new_remote_replica`) — evaluation

**Current model.** `TursoClient` holds one remote `Database` (`Builder::new_remote`);
`get_user_db(user_id)` just wraps a `Connection` to that **single shared DB**,
row-scoped by `user_id` (not a database-per-user). That single-DB fact is what makes
embedded replica clean: you replicate one database, not N.

**What changes conceptually.** `Builder::new_remote_replica(local_path, url, token)`
keeps a full local libsql/SQLite file synced from the remote. Reads hit the local file;
writes go to the remote primary and reflect locally after a sync.

| Aspect | Remote-only (today) | Embedded replica |
|---|---|---|
| Read latency | network RTT per query (A2 cost) | local file, **sub-ms** |
| Write latency | network RTT | network RTT (unchanged) |
| Cold-connection tax | every `.connect()` | gone for reads |
| `PRAGMA`/`SELECT 1` round-trips | network | local |
| Consistency | always fresh | read-your-writes only if you sync |

**Consistency — the thing to get right.** Reads see local state as of the last sync.
Options: manual `sync()` after writes (read-your-own-writes for the writing request),
`sync_interval` (periodic background pull, bounded staleness), or `read_your_writes`
mode (waits for the replica to catch up to the last written frame on read). For a
trading journal this fits well: writes are user-initiated and infrequent vs. reads
(dashboards, analytics, RAG reindex source pulls). After a mutation, call `sync()`
(or rely on `read_your_writes`); everything else tolerates seconds of staleness.

**Interactions.**
- **Reindex worker** (`jobs.rs`) reads Turso to build vectors → local reads, big win on full reindex.
- **GraphQL read resolvers** (89 `get_user_db` sites) all get faster; A2/A3 pain mostly evaporates (local N+1 is cheap — but still fix double-connect + DataLoader).
- **Multi-instance caveat:** one container = one replica file = fine. Multiple backend replicas each need their own local file + sync cadence (they don't share a file). Not a blocker at current scale; it's what bounds horizontal scaling.
- **Disk:** the replica is a full DB copy on the container disk → needs a **persistent volume** or it re-syncs from scratch each deploy.

**Risks to validate before committing.**
1. libsql `0.9.29` replica API surface (`new_remote_replica` + sync/`read_your_writes` ergonomics on this exact version — the API has churned across 0.x).
2. Migrations on boot (`migrate()` in `TursoClient::new`) run against the primary; confirm DDL replicates cleanly and the first sync picks up schema.
3. Write-heavy bursts (brokerage sync inserting many transactions) still pay remote write latency; a big burst means a larger subsequent sync.
4. Container restart cold-start: first boot with an empty replica file does a one-time full sync before reads are local → needs the persistent volume.

**Bottom line.** For a read-dominated, single-instance, single-shared-DB workload,
embedded replica is the highest-leverage latency change available — it turns A2 from
"network RTT per read" into "sub-ms local read" and makes remaining N+1 issues
low-stakes. Cost is operational: persistent replica volume, deliberate post-write
`sync()` discipline, and a horizontal-scaling constraint. Treat it as its own focused
change with measured before/after, **separate** from the in-place pooling fix (the safe
fallback if replica validation hits a snag on this libsql version).
