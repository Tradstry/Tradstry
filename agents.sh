#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_DIR="$ROOT_DIR/backend/agents_service"
HOST="${AGENTS_SERVICE_HOST:-0.0.0.0}"
REQUESTED_PORT="${AGENTS_SERVICE_PORT:-8091}"

cd "$SERVICE_DIR"

export PYTHONPATH="$SERVICE_DIR/src${PYTHONPATH:+:$PYTHONPATH}"

printf 'Running strict mypy before startup...\n'
uv run mypy --config-file mypy-strict.ini src

PORT="$(
  python3 - <<'PY' "$REQUESTED_PORT"
import socket
import sys

start_port = int(sys.argv[1])

for port in range(start_port, start_port + 20):
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            sock.bind(("127.0.0.1", port))
        except OSError:
            continue
        print(port)
        break
else:
    raise SystemExit("No free port found in the next 20 ports.")
PY
)"

if [[ "$PORT" != "$REQUESTED_PORT" ]]; then
  printf 'Port %s is already in use, starting agents_service on %s instead.\n' "$REQUESTED_PORT" "$PORT"
fi

exec uv run uvicorn main:app \
  --app-dir src \
  --host "$HOST" \
  --port "$PORT" \
  --reload \
  --reload-dir src
