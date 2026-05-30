#!/usr/bin/env bash
set -euo pipefail

# One-command bring-up of Tradstry's Postgres on a VPS.
#
# Builds the custom image (pgvector/pgvector:pg18 + ParadeDB pg_search for BM25),
# (re)starts the `postgres` container reusing the persistent `postgres_data`
# volume, runs it with shared_preload_libraries=pg_search, and enables the
# `vector` + `pg_search` extensions.
#
# Switching VPS? Pass the new SSH host alias as $1 (default: myserver) and run
# this once. Existing data in the `postgres_data` volume is preserved across
# re-runs (the container is recreated, the volume is not).
#
# Usage:
#   ./scripts/postgres/postgres.sh            # uses HOST=myserver
#   ./scripts/postgres/postgres.sh newserver  # target a different SSH alias
#
# Note: POSTGRES_PASSWORD only takes effect on FIRST init of an empty volume.
# On an existing data volume the stored password is unchanged; enter the
# current one so the printed connection URL is correct.

HOST="${1:-myserver}"
CONTAINER="postgres"
PORT=5432
IMAGE="tradstry-postgres:pg18-pgsearch"
REMOTE_DIR="/root/tradstry/scripts/postgres"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

read -rsp "PostgreSQL password (used only on first init): " PG_PASSWORD
echo

echo "Copying Dockerfile to ${HOST}..."
ssh "$HOST" "mkdir -p '$REMOTE_DIR'"
scp -q "$SCRIPT_DIR/Dockerfile" "$HOST:$REMOTE_DIR/Dockerfile"

ssh "$HOST" bash -s -- "$CONTAINER" "$PORT" "$PG_PASSWORD" "$IMAGE" "$REMOTE_DIR" <<'REMOTE'
set -euo pipefail
CONTAINER="$1"; PORT="$2"; PG_PASSWORD="$3"; IMAGE="$4"; REMOTE_DIR="$5"

echo "Building ${IMAGE} (pgvector/pg18 + pg_search)..."
docker build -t "$IMAGE" -f "$REMOTE_DIR/Dockerfile" "$REMOTE_DIR"

if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER}$"; then
  echo "Recreating existing ${CONTAINER} container (postgres_data volume is preserved)..."
  docker stop "$CONTAINER" 2>/dev/null || true
  docker rm "$CONTAINER" 2>/dev/null || true
fi

echo "Starting ${CONTAINER} on port ${PORT} with shared_preload_libraries=pg_search..."
docker run -d \
  --name "$CONTAINER" \
  --restart unless-stopped \
  -p "${PORT}:5432" \
  -e POSTGRES_PASSWORD="$PG_PASSWORD" \
  -e PGDATA=/var/lib/postgresql/18/docker \
  -v postgres_data:/var/lib/postgresql \
  "$IMAGE" \
  postgres -c shared_preload_libraries=pg_search

echo "Waiting for PostgreSQL..."
for i in $(seq 1 30); do
  if docker exec "$CONTAINER" psql -U postgres -tAc "SELECT 1" >/dev/null 2>&1; then
    echo "PostgreSQL is up."
    break
  fi
  sleep 2
done

echo "Enabling extensions..."
docker exec "$CONTAINER" psql -U postgres -c "CREATE EXTENSION IF NOT EXISTS vector;"
docker exec "$CONTAINER" psql -U postgres -c "CREATE EXTENSION IF NOT EXISTS pg_search;"
docker exec "$CONTAINER" psql -U postgres -tAc \
  "SELECT extname, extversion FROM pg_extension WHERE extname IN ('vector','pg_search') ORDER BY extname;"
REMOTE

HOSTNAME=$(ssh "$HOST" "hostname -f")

echo ""
echo "=== Tradstry Postgres (pgvector + pg_search) is running on ${HOST} ==="
echo "POSTGRES_URL=postgresql://postgres:${PG_PASSWORD}@${HOSTNAME}:${PORT}/postgres"
echo ""
echo "Set this as POSTGRES_URL in backend/.env.production."
