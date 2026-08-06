#!/usr/bin/env bash
set -euo pipefail

# Phase 1: fold the hand-run Postgres, Redis and Caddy into the tradstry stack.
#
# Why this is a script and not `make deploy`: three containers currently outside
# compose hold resources the new ones need — the postgres_data/redis_data volumes
# (a volume can't be safely mounted by two live Postgres processes) and ports
# 80/443 (held by meeting-bot's Caddy). They must be stopped in the right order,
# and the Postgres password has to be rotated on the *existing* data directory,
# since POSTGRES_PASSWORD only applies when initialising an empty volume.
#
# Old containers are renamed, never deleted, so rollback is one command.
#
# Expect ~30-60s where the four *.tradstry.com domains return errors.
#
# Usage: ./devops/scripts/01-cutover.sh [ssh-host]   (default: myserver)

HOST="${1:-myserver}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REMOTE_DIR="/root/tradstry"
STAMP="$(date +%Y%m%d-%H%M%S)"

say() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }

say "Preflight"
for f in devops/compose.yml devops/.env devops/caddy/Caddyfile \
         devops/docker/postgres/Dockerfile devops/bugsink/.env.production \
         devops/countly.env devops/countly-dashboard.env \
         backend/.env.production microservice/snaptrade-service/.env; do
  [ -f "$ROOT/$f" ] || { echo "MISSING: $f"; exit 1; }
done
grep -q '^POSTGRES_URL=postgresql://postgres:.*@postgres:5432/' "$ROOT/backend/.env.production" \
  || { echo "backend/.env.production POSTGRES_URL does not point at the compose host"; exit 1; }
grep -q '^REDIS_URL=redis://:.*@redis:6379' "$ROOT/backend/.env.production" \
  || { echo "backend/.env.production REDIS_URL does not point at the compose host"; exit 1; }
echo "local config OK"

ssh "$HOST" bash -euo pipefail -s <<'PRE'
for v in postgres_data redis_data; do
  docker volume inspect "$v" >/dev/null 2>&1 || { echo "MISSING VOLUME: $v"; exit 1; }
done
docker image inspect tradstry-postgres:pg18-pgsearch >/dev/null 2>&1 \
  || { echo "MISSING IMAGE: tradstry-postgres:pg18-pgsearch"; exit 1; }
echo "remote volumes + image OK"
PRE

say "Backing up Postgres (safety net)"
ssh "$HOST" bash -euo pipefail -s -- "$STAMP" <<'BACKUP'
STAMP="$1"
mkdir -p /root/backups
docker exec postgres pg_dumpall -U postgres | gzip > "/root/backups/pg-precutover-$STAMP.sql.gz"
ls -lh "/root/backups/pg-precutover-$STAMP.sql.gz"
BACKUP

say "Copying Let's Encrypt certs into the new Caddy volume"
# Without this the new Caddy re-issues all four certs on first boot. That works,
# but it burns Let's Encrypt rate limit and leaves HTTPS broken while it runs.
ssh "$HOST" bash -euo pipefail -s <<'CERTS'
docker volume create tradstry_caddy_data >/dev/null
docker volume create tradstry_caddy_config >/dev/null
docker run --rm \
  -v meeting-bot_caddy_data:/from:ro -v tradstry_caddy_data:/to \
  alpine sh -c 'cp -a /from/. /to/ && ls /to/caddy/certificates/*/ | head'
CERTS

say "Syncing config to $HOST"
ssh "$HOST" "mkdir -p $REMOTE_DIR/{backend,microservice/snaptrade-service,devops/bugsink,devops/caddy,devops/docker/postgres}"
scp -q "$ROOT/devops/compose.yml"                     "$HOST:$REMOTE_DIR/devops/compose.yml"
scp -q "$ROOT/devops/caddy/Caddyfile"                "$HOST:$REMOTE_DIR/devops/caddy/Caddyfile"
scp -q "$ROOT/devops/docker/postgres/Dockerfile"     "$HOST:$REMOTE_DIR/devops/docker/postgres/Dockerfile"
scp -q "$ROOT/devops/.env"                           "$HOST:$REMOTE_DIR/devops/.env"
scp -q "$ROOT/devops/countly.env"                    "$HOST:$REMOTE_DIR/devops/countly.env"
scp -q "$ROOT/devops/countly-dashboard.env"          "$HOST:$REMOTE_DIR/devops/countly-dashboard.env"
scp -q "$ROOT/backend/.env.production"                "$HOST:$REMOTE_DIR/backend/.env"
scp -q "$ROOT/microservice/snaptrade-service/.env"    "$HOST:$REMOTE_DIR/microservice/snaptrade-service/.env"
scp -q "$ROOT/devops/bugsink/.env.production"         "$HOST:$REMOTE_DIR/devops/bugsink/.env.production"
ssh "$HOST" "chmod 600 $REMOTE_DIR/devops/.env $REMOTE_DIR/devops/countly.env \
             $REMOTE_DIR/devops/countly-dashboard.env $REMOTE_DIR/backend/.env \
             $REMOTE_DIR/devops/bugsink/.env.production $REMOTE_DIR/microservice/snaptrade-service/.env"

say "Cutover"
ssh "$HOST" bash -euo pipefail -s -- "$STAMP" <<'CUTOVER'
STAMP="$1"
cd /root/tradstry
compose() { docker compose --env-file devops/.env -f devops/compose.yml "$@"; }

echo "-- stopping the hand-run containers (renamed, not deleted)"
for c in postgres redis meeting-bot-caddy-1; do
  if docker inspect "$c" >/dev/null 2>&1; then
    docker stop "$c" >/dev/null
    docker rename "$c" "${c}-old-${STAMP}"
    echo "   $c -> ${c}-old-${STAMP}"
  fi
done

echo "-- starting infrastructure"
compose up -d postgres redis caddy
for i in $(seq 1 30); do
  compose exec -T postgres pg_isready -U postgres >/dev/null 2>&1 && break
  sleep 2
done
compose exec -T postgres pg_isready -U postgres

echo "-- rotating the Postgres superuser password"
# POSTGRES_PASSWORD is ignored on a pre-existing data directory, so the volume
# still carries the old password until this runs. Piped rather than `psql -c`,
# because -c does no variable interpolation, and rather than `-f -`, which
# swallowed the statement silently when stdin was already the outer heredoc.
set -a; . /root/tradstry/devops/.env; set +a
printf "ALTER USER postgres PASSWORD '%s';\n" "$POSTGRES_PASSWORD" \
  | compose exec -T postgres psql -U postgres -q -v ON_ERROR_STOP=1

# Verify over TCP the way the app connects. Without this the rotation can fail
# and the whole cutover still reports success, right up until the app crash-loops.
compose exec -T -e PGPASSWORD="$POSTGRES_PASSWORD" postgres \
  psql -h postgres -U postgres -tAc 'select 1' >/dev/null \
  || { echo "ABORT: new password does not authenticate over TCP"; exit 1; }
echo "   rotated and verified"

echo "-- starting application services"
compose pull --ignore-pull-failures
# --force-recreate on the app containers: compose does not always treat an
# env_file content change as a reason to replace a running container, so without
# this the backend keeps its old POSTGRES_URL/REDIS_URL and dies on a dead host.
compose up -d --remove-orphans
compose up -d --force-recreate backend mcp-server

echo "-- waiting for health"
sleep 20
compose ps
CUTOVER

say "Verifying"
ssh "$HOST" bash -euo pipefail -s <<'VERIFY'
fail=0
compose() { docker compose --env-file /root/tradstry/devops/.env -f /root/tradstry/devops/compose.yml "$@"; }
for u in https://backend.tradstry.com/health https://mcp.tradstry.com/health https://bugsink.tradstry.com/; do
  code=$(curl -s -o /dev/null -w '%{http_code}' -m 15 "$u" || echo 000)
  printf '  %-40s %s\n' "$u" "$code"
  case "$code" in 2*|3*) ;; *) fail=1 ;; esac
done

echo "  -- app -> postgres/redis over the compose network:"
compose exec -T backend sh -c \
  'getent hosts postgres redis' 2>/dev/null || echo "     (could not resolve — check backend logs)"

echo "  -- backend errors in the last 2 minutes:"
docker logs tradstry-backend --since 2m 2>&1 | grep -iE '"level":"(ERROR|WARN)"' | tail -5 || echo "     none"

[ "$fail" -eq 0 ] && echo "  ALL GREEN" || { echo "  FAILED — see rollback below"; exit 1; }
VERIFY

cat <<EOF

Cutover complete.

Old containers are still on the box as *-old-$STAMP (stopped). Once you are happy,
run 02-cleanup.sh, which removes them along with meeting-bot, Typesense and the
stale directories.

ROLLBACK (if something is wrong):
  ssh $HOST 'cd /root/tradstry && docker compose --env-file devops/.env -f devops/compose.yml down && \\
    docker rename postgres-old-$STAMP postgres && \\
    docker rename redis-old-$STAMP redis && \\
    docker rename meeting-bot-caddy-1-old-$STAMP meeting-bot-caddy-1 && \\
    docker start postgres redis meeting-bot-caddy-1'
  # then restore the old backend/.env from git and re-run: make deploy
  # Postgres password is already rotated, so also update POSTGRES_URL in that file.
  # Full dump if needed: /root/backups/pg-precutover-$STAMP.sql.gz
EOF
