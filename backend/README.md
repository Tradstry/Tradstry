# Tradstry Backend

Rust backend built with Actix-Web and async-graphql. Each user gets an isolated Turso (libSQL) database. AI chat uses a LangGraph-based agent system with Groq LLM, Qdrant vector search, and Jina embeddings.

## Stack

- **Runtime:** Rust (stable, edition 2024)
- **Web framework:** Actix-Web 4
- **API:** GraphQL (async-graphql) + WebSocket subscriptions
- **Database:** Turso (libSQL) per-user, Postgres for checkpoints/memory store
- **AI:** Groq LLM (via rig), Jina embeddings, Qdrant vector DB
- **Auth:** Clerk (JWT validation)
- **Images:** Cloudinary
- **Brokerage:** SnapTrade (via Go microservice)
- **Market data:** finance-query crate

## Prerequisites

- Rust 1.85+ (install from [rustup.rs](https://rustup.rs/))
- A running Postgres instance (for LangGraph checkpoints + memory store)
- Turso account + database
- Qdrant instance
- API keys: Groq, Jina, Clerk, Cloudinary

## Setup

```bash
cd backend
cp .env.example .env  # then fill in values
cargo build
```

### Environment Variables

```bash
# Database
TURSO_DB_URL=libsql://your-db.turso.io
TURSO_DB_TOKEN=your-turso-token
POSTGRES_URL=postgres://user:pass@localhost:5432/tradstry

# Auth
CLERK_SECRET_KEY=sk_live_...

# AI
GROQ_API_KEY=gsk_...
GROQ_MODEL=openai/gpt-oss-120b     # optional, this is the default

# Vector search + embeddings
QDRANT_URL=https://your-instance.qdrant.io
QDRANT_API_KEY=your-qdrant-key
JINA_API_KEY=jina_...
JINA_EMBEDDING_MODEL=jina-embeddings-v5-text-small  # optional default
JINA_RERANKER_MODEL=jina-reranker-v2-base-multilingual  # optional

# Images
CLOUDINARY_CLOUD_NAME=your-cloud
CLOUDINARY_API_KEY=your-key
CLOUDINARY_API_SECRET=your-secret

# Brokerage
SNAPTRADE_SERVICE_URL=http://localhost:9086  # or http://snaptrade-service:9086 in Docker
BROKERAGE_ENCRYPTION_KEY=32-byte-hex-key

# Server
RUST_LOG=info
CORS_ALLOWED_ORIGINS=http://localhost:3000,http://127.0.0.1:3000
```

## Running

```bash
# Development (with backtrace)
../start.sh

# Or manually
RUST_BACKTRACE=1 cargo run

# Production build
cargo build --release
./target/release/tradstry-backend
```

The server starts on `0.0.0.0:8080` by default (configurable via `PORT` env var).

## API

All data flows through a single GraphQL endpoint:

- **GraphQL:** `POST /graphql`
- **GraphQL Playground:** `GET /graphql`
- **WebSocket (subscriptions):** `GET /graphql/ws`
- **Image upload:** `POST /notebook/images/upload`
- **Image serve:** `GET /notebook/images/{id}`

### Key GraphQL operations

**Queries:** `accounts`, `journalEntries`, `playbooks`, `chatSessions`, `chatMessages`, `notebookNotes`, `userAgents`, `userPrompts`, `analyticsCore`, `analyticsRisk`

**Mutations:** `createAccount`, `createJournalEntry`, `sendChatMessage`, `createNotebookNote`, `updateNotebookNote`, `createUserAgent`, `createUserPrompt`, `notebookAutocomplete`, `notebookTransform`

**Subscriptions:** `chatStream`, `aiJobEvents`

## Project Structure

```
src/
  main.rs                     # Server setup, middleware, app data
  routes/                     # HTTP route handlers (GraphQL, images)
  graphql/                    # GraphQL resolvers (query, mutation, subscription)
  service/
    agents/                   # Groq LLM client, vector DB client
    chat/
      agent.rs                # Chat agent orchestrator
      graph.rs                # LangGraph chat graph (nodes, edges, tools)
      tools/                  # AI tool implementations (db_query, stock_quote, etc.)
      subgraphs/              # Research, report, comparison subgraphs
      assistance/             # Notebook autocomplete + text transform
      memory.rs               # Memory extraction + dedup
      memory_store.rs         # Postgres memory store wrapper
      checkpoint.rs           # Postgres checkpoint saver wrapper
      types.rs                # Stream events, Groq types
    turso/
      client.rs               # Per-user database client
      schema/                 # Declarative schema + auto-migration
        tables/               # Table definitions + CRUD functions
        logic.rs              # Schema diff + migration engine
    ai/                       # AI artifact jobs (insights, reports, mindset)
    brokerage/                # SnapTrade integration (sync, holdings, balances)
    read_service/             # Shared read helpers (users, accounts, etc.)
```

## Docker

```bash
# Build
docker build -t tradstry-backend -f Dockerfile .

# Or use the root docker-compose (includes snaptrade service)
cd ..
docker compose up
```

The Dockerfile uses `cargo-chef` for dependency caching and stable Rust 1.91. Internal port is 9086, exposed as 9099 in the root docker-compose.

## Schema Migrations

The schema is declarative in `src/service/turso/schema/tables/mod.rs`. To add/change tables:

1. Edit the `SCHEMA_SQL` constant
2. Bump `SCHEMA_VERSION` in `logic.rs`
3. Restart — the migration engine diffs and applies changes automatically

Supports: create/drop tables, add/drop columns, rename columns/tables, create/drop indexes and triggers.
