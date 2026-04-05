# Backend Development Guide

Development guide for the Tradstry backend — a Rust trading journal platform with AI-powered analysis.

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│ Frontend (Next.js)                                  │
│   GraphQL queries/mutations/subscriptions            │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│ Backend (Rust / Actix-Web)                          │
│                                                      │
│  GraphQL API (async-graphql)                         │
│    ├── accounts, journal, playbook, notebook         │
│    ├── brokerage (SnapTrade sync)                    │
│    ├── chat (LangGraph AI agent)                     │
│    ├── ai (insights, reports, mindset generation)    │
│    ├── analytics (trade metrics)                     │
│    └── user_agents (custom agent management)         │
│                                                      │
│  Background Services                                 │
│    ├── AI Worker Loop (job processing)               │
│    ├── Brokerage Sync Scheduler (market hours)       │
│    └── Memory Extraction (after each chat turn)      │
│                                                      │
│  External Integrations                               │
│    ├── Turso (SQLite) — app data                     │
│    ├── Postgres — LangGraph checkpoints + memory     │
│    ├── Qdrant — vector search (hybrid + memories)    │
│    ├── Groq — LLM (gpt-oss-120b)                    │
│    ├── Jina — text embeddings (v3, 1024d)            │
│    └── Clerk — authentication                        │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│ Go Microservice (snaptrade-service)                  │
│   Wraps SnapTrade SDK for brokerage data fetching    │
│   Port 9056                                          │
└─────────────────────────────────────────────────────┘
```

## Project Structure

### Root Directory

- `Cargo.toml` / `Cargo.lock` — Rust project manifest
- `crates/` — Local LangGraph crate (state machine framework for AI agent)
- `src/` — Main Rust source code
- `.env` / `.env.example` — Environment variables
- `start.sh` — Development startup script

### Source Code (`src/`)

- **`main.rs`** — Entry point. Sets up HTTP server, middleware, dependency injection, background services.

- **`graphql/`** — GraphQL resolvers and types:
  - `mod.rs` — Schema builder, Query/Mutation/Subscription composition
  - `accounts.rs` — Account CRUD
  - `journal.rs` — Journal entries + brokerage trade linking
  - `playbook.rs` — Trading playbooks
  - `notebook.rs` — Rich-text notes with trade linking
  - `brokerage.rs` — Brokerage connection, sync, transactions/holdings/balances
  - `chat.rs` — Chat sessions, messages (from checkpoints), send message
  - `ai.rs` — AI insights, reports, mindset generation
  - `analytics.rs` — Trade performance metrics
  - `user_agents.rs` — Custom agent management (list, delete)
  - `users.rs` — User profile

- **`routes/`** — HTTP route configuration (GraphQL endpoint, health checks)

- **`service/`** — Core business logic:
  - `turso/` — Database client (Turso/libSQL), schema management, auto-migration
    - `schema/tables/` — Table definitions and CRUD for each entity
    - `schema/logic.rs` — Schema versioning and migration engine
  - `chat/` — AI chat system:
    - `agent.rs` — Chat agent entry point (memory retrieval, graph execution, memory extraction)
    - `graph.rs` — LangGraph StateGraph definition (LLM node, tool nodes, subgraph nodes, conditional routing)
    - `checkpoint.rs` — Postgres checkpoint saver with async-safe blocking wrapper
    - `memory.rs` — Memory extraction, retrieval, and Qdrant backfill
    - `memory_store.rs` — PostgresStore with async-safe blocking wrapper
    - `tools/` — AI tool implementations:
      - `db_query.rs` — Query trades, journal entries, playbooks from DB
      - `semantic_search.rs` — Hybrid vector search across trading data
      - `analytics_calc.rs` — Compute performance metrics
      - `recall_memory.rs` — Search cross-session AI memories
      - `create_agent.rs` — Start custom agent creation interview
      - `save_agent.rs` — Validate and store custom agent definition
      - `run_agent.rs` — Execute a saved custom agent
      - `edit_agent.rs` — Modify an existing agent
    - `subgraphs/` — Specialist AI subgraphs:
      - `research.rs` — Multi-step trade research (fetch → metrics → patterns → synthesize)
      - `report.rs` — Structured performance report generation
      - `comparison.rs` — Side-by-side trade comparison
    - `agents/` — Dynamic agent system:
      - `definition.rs` — AgentDefinition, AgentStep types
      - `compiler.rs` — Compiles agent definitions into LangGraph StateGraphs at runtime
      - `runner.rs` — Loads, compiles, and executes saved agents
    - `sessions.rs` — Chat session CRUD
    - `types.rs` — Chat event types, Groq message types
  - `ai/` — Background AI job processing:
    - `jobs.rs` — Worker loop, reindex, insights, reports, mindset generation
    - `db.rs` — Job queue database operations
    - `types.rs` — Job types, event types, artifact types
  - `agents/` — External AI service clients:
    - `client.rs` — Groq LLM client (streaming chat, prompt)
    - `vector_database/client.rs` — Qdrant + Jina client (embeddings, hybrid search, memories)
    - `vector_database/sparse.rs` — BM25-style sparse vector generation
  - `brokerage/` — Brokerage integration:
    - `client.rs` — HTTP client for Go microservice
    - `transaction.rs` — Sync transactions and holdings from SnapTrade
    - `sync.rs` — Background sync scheduler (market hours, weekends)
    - `db.rs` — Encryption for SnapTrade credentials
  - `auth/` — Clerk JWT validation
  - `cloudinary/` — Image upload for notebook
  - `read_service/` — Thin wrappers around table functions for GraphQL resolvers

### LangGraph Crate (`crates/`)

Local Rust port of LangGraph — state machine framework for the AI agent:

- `core/graph/` — StateGraph builder, compiler, subgraph composition
- `core/channels/` — State channels (LastValue, Topic, AnyValue, etc.)
- `core/types/` — Execution context, node results, commands, interrupts
- `runtime/loop/` — Main execution loop engine
- `runtime/runner/` — Task execution with retry policies
- `checkpoint/` — State persistence (Postgres, SQLite, InMemory)
- `store/` — Key-value storage (Postgres, SQLite, InMemory)
- `cache/` — Node result caching
- `adapters/` — Framework integrations (Rig, LangChain)

### Go Microservice (`microservice/snaptrade-service/`)

Thin adapter around the SnapTrade Go SDK:

- `main.go` — Fiber HTTP server, route registration
- `client/snaptrade.go` — SnapTrade SDK wrapper
- `handlers/` — REST endpoint handlers (users, connections, transactions, holdings, accounts)

## Prerequisites

- **Rust** — Latest stable (install via [rustup](https://rustup.rs/))
- **Go** — 1.24+ (for snaptrade-service)
- **Turso account** — App database
- **Postgres instance** — LangGraph checkpoints + memory store
- **Qdrant instance** — Vector search
- **Groq API key** — LLM
- **Jina API key** — Text embeddings
- **Clerk account** — Authentication

## Environment Variables

Create `.env` from `.env.example`. Key variables:

| Variable | Purpose |
|----------|---------|
| `TURSO_DATABASE_URL` | Turso database URL |
| `TURSO_AUTH_TOKEN` | Turso auth token |
| `CLERK_SECRET_KEY` | Clerk JWT validation |
| `GROQ_API_KEY` | Groq LLM API |
| `JINA_API_KEY` | Jina embeddings API |
| `JINA_EMBEDDING_MODEL` | Model name (default: `jina-embeddings-v3`) |
| `QDRANT_URL` | Qdrant vector DB URL |
| `QDRANT_API_KEY` | Qdrant API key |
| `POSTGRES_URL` | Postgres connection string (checkpoints + memory) |
| `SNAPTRADE_SERVICE_URL` | Go microservice URL (default: `http://localhost:9056`) |
| `BROKERAGE_ENCRYPTION_KEY` | Base64-encoded 32-byte key for SnapTrade secret encryption |
| `CLOUDINARY_CLOUD_NAME` | Cloudinary for notebook images |
| `CLOUDINARY_API_KEY` | Cloudinary API key |
| `CLOUDINARY_API_SECRET` | Cloudinary API secret |

## Running Locally

### Backend (Rust)

```bash
cd backend
cp .env.example .env  # Configure variables
cargo run
```

Server starts on `0.0.0.0:8080`. For development with backtrace:

```bash
RUST_BACKTRACE=1 cargo run
```

Or use the start script:

```bash
./start.sh
```

### Go Microservice

```bash
cd microservice/snaptrade-service
cp .env.template .env  # Configure SNAPTRADE_CLIENT_ID and SNAPTRADE_CONSUMER_KEY
go run main.go
```

Runs on port `9056`.

### Frontend

```bash
cd frontend
npm install
npm run dev
```

Runs on `http://localhost:3000`.

## Database

**Turso (SQLite)** — main app data. Schema auto-migrates on startup via `schema/logic.rs`. Current version: `1.6`.

Tables: `users`, `accounts`, `journal_entries`, `playbooks`, `notebook_notes`, `notebook_note_trades`, `notebook_images`, `chat_sessions`, `brokerage_transactions`, `brokerage_holdings`, `brokerage_balances`, `journal_brokerage_links`, `user_agents`, `ai_jobs`, `ai_artifacts`, `ai_source_documents`.

**Postgres** — LangGraph state. Auto-creates tables on startup:
- `checkpoints` / `writes` — conversation state per session
- `store_items` / `store_embeddings` — cross-session AI memories

## Qdrant Collections

| Collection | Purpose | Vectors |
|-----------|---------|---------|
| `tradstry_hybrid` | Journal entries, playbooks, notebook notes | Dense (Jina 1024d) + Sparse (BM25) |
| `tradstry_memories` | Cross-session AI memories | Dense (Jina 1024d) + Sparse (BM25) |

Both auto-created on startup via `ensure_hybrid_collection()` and `ensure_memories_collection()`.

## AI System

### Chat Agent (LangGraph)

The chat agent is a `StateGraph` with channels (`messages`, `current_tool_call`, `iteration`) and nodes:

```
START → llm_node → (conditional router) → tool_nodes / subgraph_nodes → llm_node → ... → END
```

**Tool nodes:** db_query, semantic_search, analytics_calc, recall_memory, create_agent, save_agent, run_agent, edit_agent

**Subgraph nodes:** research (4-step pipeline), report (4-step pipeline), comparison (3-step pipeline)

**Checkpointing:** Conversation state persisted to Postgres. Messages live in the `messages` Topic channel.

**Memory system:** After each turn, memories are extracted via LLM and stored in PostgresStore + Qdrant. On session start, relevant memories are injected into the system prompt.

### Background Services

- **AI Worker Loop** — polls `ai_jobs` table every 2s, processes reindex/insights/report/mindset jobs
- **Brokerage Sync** — syncs connected brokerages on market hours (weekday 9am-4:30pm ET, Saturday 1am ET)
- **Memory Extraction** — fire-and-forget after each chat turn

### Custom Agents

Users create agents through conversational interviews. Agent definitions (tool pipeline + goal + output style) are stored in `user_agents` table and compiled into `StateGraph`s at runtime.

## Testing

```bash
# Backend tests
cargo test

# LangGraph crate tests
cd crates && cargo test

# All tests
cargo test --workspace
```

## Schema Versioning

Schema version is in `src/service/turso/schema/logic.rs`. Bump `SCHEMA_VERSION` when changing `SCHEMA_SQL` in `tables/mod.rs`. The migrator auto-detects changes (new tables, new columns, dropped columns, indexes) and applies them on startup.
