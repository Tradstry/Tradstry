# Tradstry - AI-Powered Trading Journal & Analytics Platform

Tradstry is a comprehensive trading journal and analytics platform that helps traders track, analyze, and improve their trading performance using AI-powered insights and real-time analytics.

## Overview

Tradstry combines advanced journaling capabilities with sophisticated analytics to transform how traders make decisions. The platform integrates with brokerage accounts, provides real-time market data, and uses AI to generate personalized trading insights and reports.

### Key Features

- **Real-time Analytics**: Comprehensive performance tracking with risk metrics, P&L analysis, and market correlation insights
- **AI-Powered Insights**: Automated behavioral analysis, pattern recognition, and personalized recommendations
- **Advanced Journaling**: Rich text editor (Lexical), trade tagging, playbook creation, and multimedia support
- **Brokerage Integration**: Direct connection to trading accounts via SnapTrade for automatic trade importing
- **Market Data**: Live quotes, historical data, technical indicators, and news aggregation
- **AI Chat**: Interactive AI assistant for trading analysis and strategy discussions
- **Responsive Design**: Full-featured web application with mobile support

## Tech Stack

### Frontend (`/frontend`)
- **Framework**: Next.js 16 with React 19
- **Language**: TypeScript
- **Styling**: Tailwind CSS v4
- **UI Library**: Radix UI / shadcn components
- **State Management**: Zustand
- **Data Fetching**: TanStack Query
- **Rich Text Editor**: Lexical
- **Charts**: Recharts
- **Linting & Formatting**: Biome
- **Package Manager**: Bun

### Backend (`/backend`)
- **Language**: Rust (Edition 2024)
- **Web Framework**: Actix-web
- **API**: GraphQL (async-graphql) + REST
- **Database**: Turso (libSQL) for app data, Postgres for LangGraph checkpoints/memory
- **Vector Search**: Qdrant for AI embeddings
- **LLM**: Groq (default model: `openai/gpt-oss-120b`)
- **Embeddings & Reranking**: Jina
- **AI Framework**: custom LangGraph crate (`backend/crates`)
- **Authentication**: Clerk
- **File Storage**: Cloudinary

### Microservices (`/microservice`)
- **SnapTrade Service**: Go-based brokerage integration service for account syncing and trade importing

### Infrastructure
- **Deployment**: Docker with multi-stage builds
- **Reverse Proxy**: Caddy with automatic HTTPS
- **Orchestration**: Docker Compose
- **CI/CD**: GitHub Actions
- **Frontend Hosting**: Vercel
- **Monitoring**: Health check endpoints

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Next.js App   │────│   Rust Backend  │────│     Database     │
│   (Vercel)      │    │   (Actix-web)   │    │   (Turso/SQL)    │
│                 │    │                 │    │                 │
│ • Landing Pages │    │ • GraphQL API   │    │ • User Data      │
│ • Dashboard     │    │ • REST API      │    │ • Trade Records  │
│ • Analytics UI  │    │ • Auth (Clerk)  │    └─────────────────┘
│ • Journaling    │    │ • AI Services   │            │
└─────────────────┘    └────────┬────────┘            │
                                │                      │
                    ┌───────────┼───────────┐          │
                    │           │           │          │
              ┌─────┴─────┐ ┌──┴──┐ ┌──────┴──────┐   │
              │  SnapTrade │ │Qdrant│ │  AI Services │   │
              │  Service   │ │     │ │             │   │
              │  (Go)      │ │     │ │ • Rig       │   │
              └────────────┘ └─────┘ │ • LangGraph │   │
                                     └─────────────┘   │
```

## Project Structure

```
tradstry/
├── frontend/                     # Next.js frontend
│   ├── src/
│   │   ├── app/                  # App Router pages
│   │   ├── components/           # React components
│   │   ├── hooks/                # Custom React hooks
│   │   ├── lib/                  # Utilities and services
│   │   └── middleware.ts         # Auth middleware (Clerk)
│   ├── public/                   # Static assets
│   ├── package.json              # Dependencies (Bun)
│   └── biome.json                # Linting & formatting config
├── backend/                      # Rust backend
│   ├── src/                      # Application source
│   ├── crates/                   # Workspace crates (LangGraph)
│   ├── Cargo.toml                # Rust dependencies
│   └── dockerfile                # Multi-stage Docker build
├── microservice/
│   └── snaptrade-service/        # Go brokerage integration
├── caddy/
│   └── Caddyfile                 # Reverse proxy config
├── docker-compose.yml            # Production orchestration
└── .github/
    └── workflows/                # CI/CD pipelines
        ├── commit-check.yml      # PR quality checks (fmt, clippy, build, audit)
        └── release.yml           # Tagged release Docker builds
```

## Local Development Setup

### Prerequisites

- **Bun**: Package manager for frontend
- **Rust**: 1.85+ with Cargo
- **Docker**: For production services
- **Git**: For version control

### 1. Clone and Install

```bash
git clone <repository-url>
cd tradstry

# Install frontend dependencies
cd frontend
bun install

# Build backend
cd ../backend
cargo build
```

### 2. Environment Configuration

#### Frontend
Create `frontend/.env.local`:
```bash
NEXT_PUBLIC_CLERK_PUBLISHABLE_KEY=your_clerk_publishable_key
CLERK_SECRET_KEY=your_clerk_secret_key
NEXT_PUBLIC_BACKEND_URL=http://localhost:9086
```

#### Backend
Create `backend/.env` (see `backend/.env.example` for the full list):
```bash
# Database — Turso (libSQL)
TURSO_DB_URL=libsql://your-db.turso.io
TURSO_DB_TOKEN=your_turso_token

# Database — Postgres (LangGraph checkpoints + memory store)
POSTGRES_URL=postgres://user:pass@localhost:5432/tradstry

# Auth — Clerk
CLERK_SECRET_KEY=sk_live_...

# AI — Groq LLM
GROQ_API_KEY=gsk_...
GROQ_MODEL=openai/gpt-oss-120b

# Vector Search — Qdrant
QDRANT_URL=https://your-instance.qdrant.io:6334
QDRANT_API_KEY=your_qdrant_key

# Embeddings + Reranking — Jina
JINA_API_KEY=jina_...
JINA_EMBEDDING_MODEL=jina-embeddings-v5-text-small
JINA_RERANKER_MODEL=jina-reranker-v2-base-multilingual

# Images — Cloudinary
CLOUDINARY_CLOUD_NAME=your_cloud_name
CLOUDINARY_API_KEY=your_api_key
CLOUDINARY_API_SECRET=your_api_secret

# Brokerage — SnapTrade (via Go microservice)
SNAPTRADE_SERVICE_URL=http://localhost:9087
BROKERAGE_ENCRYPTION_KEY=your_encryption_key

# Server
RUST_LOG=info
CORS_ALLOWED_ORIGINS=http://localhost:3038,http://127.0.0.1:3038
```

#### SnapTrade Microservice
Create `microservice/snaptrade-service/.env`:
```bash
SNAPTRADE_CLIENT_ID=your_snaptrade_client_id
SNAPTRADE_CONSUMER_KEY=your_snaptrade_consumer_key
```

### 3. Run Development Servers

#### Frontend (Terminal 1)
```bash
cd frontend
bun run dev
# http://localhost:3038
```

#### Backend (Terminal 2)
```bash
cd backend
cargo run
# http://localhost:9086
```

### 4. Production (Docker)
```bash
docker compose up --build
```

## Development Commands

```bash
# Frontend
cd frontend
bun run dev              # Start development server
bun run build            # Build for production
bun run start            # Start production server
bun run lint             # Run Biome linter
bun run format           # Format code with Biome

# Backend
cd backend
cargo build              # Build application
cargo run                # Run development server
cargo test               # Run tests
cargo fmt                # Format code
cargo clippy             # Run linter
cargo build --release    # Production build
```

## Deployment

### Production
- **Frontend**: Vercel (Next.js)
- **Backend**: Docker container on VPS via Docker Compose
- **Reverse Proxy**: Caddy (automatic HTTPS via Let's Encrypt)
- **Database**: Turso cloud

### CI/CD
- **PR Checks** (`commit-check.yml`): `cargo fmt`, `cargo check`, `cargo clippy`, `cargo build --release`, `cargo audit`
- **Releases** (`release.yml`): Backend and SnapTrade service Docker images built and pushed to Docker Hub (`johnsonf/tradstry-backend`, `johnsonf/snaptrade-service`) on version tags (`v*.*.*`)

## License

This project is proprietary software. All rights reserved.
