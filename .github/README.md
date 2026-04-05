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
- **Database**: Turso (libSQL)
- **Cache**: Redis (Upstash)
- **Vector Search**: Qdrant for AI embeddings
- **AI Framework**: Rig + custom LangGraph crate
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
        ├── pr-checks.yml         # PR quality checks
        ├── ci-cd.yml             # CI/CD pipeline
        ├── release.yml           # Tagged release deploys
        └── merge-branch.yml      # Branch merge automation
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
NEXT_PUBLIC_CLERK_PUBLISHABLE_KEY=your_clerk_key
NEXT_PUBLIC_API_URL=http://localhost:9086
```

#### Backend
Create `backend/.env` (see `backend/.env.example` for all variables):
```bash
TURSO_DB_URL=your_turso_database_url
TURSO_API_TOKEN=your_turso_token
UPSTASH_REDIS_REST_URL=your_redis_url
UPSTASH_REDIS_REST_TOKEN=your_redis_token
QDRANT_URL=your_qdrant_url
QDRANT_API_KEY=your_qdrant_key
CLERK_SECRET_KEY=your_clerk_secret
```

### 3. Run Development Servers

#### Frontend (Terminal 1)
```bash
cd frontend
bun run dev
# http://localhost:3000
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
- **PR Checks**: `cargo fmt`, `cargo check`, `clippy`, `cargo build --release`, `cargo audit`
- **Releases**: Docker images built and pushed to Docker Hub on version tags (`v*.*.*`)

## License

This project is proprietary software. All rights reserved.
