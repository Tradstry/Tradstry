#!/usr/bin/env bash
set -euo pipefail

# One-command local Redis for Tradstry dev (OrbStack/Docker).
#
# The backend caches brokerage transactions/holdings/balances in Redis. Point
# local dev at THIS, never at prod Redis — otherwise local reads/writes hit the
# shared production cache and you get stale, slow (timing-out) responses.
#
# Host port 6381 avoids collisions with other local Redis containers (6379/6380).
# Then set in backend/.env:
#   REDIS_URL=redis://localhost:6381

CONTAINER="tradstry-redis"
PORT=6381
IMAGE="redis:7-alpine"

if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER}$"; then
  echo "Recreating ${CONTAINER}..."
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
fi

echo "Starting ${CONTAINER} on port ${PORT}..."
docker run -d \
  --name "$CONTAINER" \
  --restart unless-stopped \
  -p "${PORT}:6379" \
  "$IMAGE" >/dev/null

for _ in $(seq 1 15); do
  if [ "$(docker exec "$CONTAINER" redis-cli ping 2>/dev/null)" = "PONG" ]; then
    echo "Redis is up: redis://localhost:${PORT}"
    exit 0
  fi
  sleep 1
done

echo "Redis did not become ready in time." >&2
exit 1
