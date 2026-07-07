#!/bin/bash

# Tradstry Backend Startup Script
# This script navigates to the backend directory and starts the Rust server.

set -e  # Exit on any error

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}Starting Tradstry Backend Server${NC}"
echo "=================================="

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKEND_DIR="$SCRIPT_DIR/backend"

# ---------------------------------------------------------------------------
# Local Postgres (pgvector + pg_search) — bring it up if it isn't already
# running, then point the backend at it. Idempotent and safe to re-run; the
# persistent volume keeps your data across restarts.
# ---------------------------------------------------------------------------
PG_CONTAINER="tradstry-postgres"
PG_IMAGE="tradstry-postgres:pg18-pgsearch"
PG_VOLUME="tradstry_postgres_data"
PG_PORT=5432
PG_PASSWORD="tradstry"  # local dev only

ensure_postgres() {
  if ! command -v docker >/dev/null 2>&1; then
    echo -e "${YELLOW}Docker not found — skipping local Postgres; using POSTGRES_URL from backend/.env.${NC}"
    return 0
  fi

  if docker ps --format '{{.Names}}' | grep -q "^${PG_CONTAINER}$"; then
    echo -e "${GREEN}Postgres '${PG_CONTAINER}' already running.${NC}"
  elif docker ps -a --format '{{.Names}}' | grep -q "^${PG_CONTAINER}$"; then
    echo -e "${BLUE}Starting existing Postgres container...${NC}"
    docker start "$PG_CONTAINER" >/dev/null
  else
    if ! docker image inspect "$PG_IMAGE" >/dev/null 2>&1; then
      case "$(uname -m)" in
        arm64 | aarch64) PG_ARCH=arm64 ;;
        *) PG_ARCH=amd64 ;;
      esac
      echo -e "${BLUE}Building ${PG_IMAGE} (pgvector/pg18 + pg_search, ${PG_ARCH})...${NC}"
      docker build --build-arg PG_ARCH="$PG_ARCH" -t "$PG_IMAGE" "$SCRIPT_DIR/scripts/postgres"
    fi
    echo -e "${BLUE}Starting Postgres on port ${PG_PORT}...${NC}"
    docker run -d \
      --name "$PG_CONTAINER" \
      --restart unless-stopped \
      -p "${PG_PORT}:5432" \
      -e POSTGRES_PASSWORD="$PG_PASSWORD" \
      -e PGDATA=/var/lib/postgresql/18/docker \
      -v "${PG_VOLUME}:/var/lib/postgresql" \
      "$PG_IMAGE" \
      postgres -c shared_preload_libraries=pg_search >/dev/null
  fi

  echo -e "${BLUE}Waiting for Postgres to accept connections...${NC}"
  for i in $(seq 1 60); do
    if docker exec "$PG_CONTAINER" psql -U postgres -tAc "SELECT 1" >/dev/null 2>&1; then
      echo -e "${GREEN}Postgres is ready.${NC}"
      break
    fi
    if [ "$i" -eq 60 ]; then
      echo -e "${YELLOW}Postgres did not become ready in time. Check: docker logs ${PG_CONTAINER}${NC}" >&2
      exit 1
    fi
    sleep 1
  done

  # Ensure extensions (idempotent; the backend also creates these on startup).
  docker exec "$PG_CONTAINER" psql -U postgres -q \
    -c "CREATE EXTENSION IF NOT EXISTS vector;" \
    -c "CREATE EXTENSION IF NOT EXISTS pg_search;" >/dev/null 2>&1 || true
}

ensure_postgres

# Point the backend at the local DB. dotenvy does not override existing env,
# so these exports take precedence over any POSTGRES_URL in backend/.env.
export POSTGRES_URL="postgres://postgres:${PG_PASSWORD}@localhost:${PG_PORT}/postgres"
export POSTGRES_DATABASE="${POSTGRES_DATABASE:-dev}"

# Navigate to backend directory
echo -e "${BLUE}Navigating to backend directory...${NC}"
cd "$BACKEND_DIR"
echo "Current directory: $(pwd)"

# Start the server with backtrace enabled
echo -e "${GREEN}Starting Rust server with backtrace enabled...${NC}"
echo -e "${YELLOW}RUST_BACKTRACE is enabled for debugging${NC}"
export PORT=9000
export RUST_BACKTRACE=full
RUST_LOG=info cargo run --bin tradstry-backend
