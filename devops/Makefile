# Recipes are held in `define` blocks and run through a single `bash -c` so the
# multi-line shell logic survives macOS's GNU Make 3.81 (no .ONESHELL support).
SHELL := /bin/bash
.DEFAULT_GOAL := help

ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/..)
DEVOPS := $(ROOT)/devops
export ROOT
export DEVOPS

PG_CONTAINER := tradstry-postgres
PG_IMAGE     := tradstry-postgres:pg18-pgsearch
PG_VOLUME    := tradstry_postgres_data
PG_PORT      := 5432
PG_PASSWORD  := tradstry
export PG_CONTAINER PG_IMAGE PG_VOLUME PG_PORT PG_PASSWORD

SERVER     := myserver
REMOTE_DIR := /root/tradstry
export SERVER REMOTE_DIR

.PHONY: help backend postgres frontend desktop micro deploy tag

help:
	@echo "Tradstry"
	@echo "========"
	@echo "  make backend    Start local Postgres, then the Rust backend on :7899"
	@echo "  make postgres   Start local Postgres only"
	@echo "  make frontend   Start the web frontend (bun dev)"
	@echo "  make desktop    Start the Electron desktop app in dev mode"
	@echo "  make micro      Start the SnapTrade Go microservice"
	@echo "  make deploy     Sync config and restart services on $(SERVER)"
	@echo "  make tag        Commit pending work, bump the version tag, and push"

# ---------------------------------------------------------------------------
# Local Postgres (pgvector + pg_search) — brought up if it isn't already
# running. Idempotent and safe to re-run; the persistent volume keeps your data
# across restarts.
# ---------------------------------------------------------------------------
define POSTGRES_SCRIPT
set -e
GREEN='\033[0;32m'; BLUE='\033[0;34m'; YELLOW='\033[1;33m'; NC='\033[0m'

if ! command -v docker >/dev/null 2>&1; then
  echo -e "$${YELLOW}Docker not found — skipping local Postgres; using POSTGRES_URL from backend/.env.$${NC}"
  exit 0
fi

if docker ps --format '{{.Names}}' | grep -q "^$${PG_CONTAINER}$$"; then
  echo -e "$${GREEN}Postgres '$${PG_CONTAINER}' already running.$${NC}"
elif docker ps -a --format '{{.Names}}' | grep -q "^$${PG_CONTAINER}$$"; then
  echo -e "$${BLUE}Starting existing Postgres container...$${NC}"
  docker start "$$PG_CONTAINER" >/dev/null
else
  if ! docker image inspect "$$PG_IMAGE" >/dev/null 2>&1; then
    case "$$(uname -m)" in
      arm64 | aarch64) PG_ARCH=arm64 ;;
      *) PG_ARCH=amd64 ;;
    esac
    echo -e "$${BLUE}Building $${PG_IMAGE} (pgvector/pg18 + pg_search, $${PG_ARCH})...$${NC}"
    docker build --build-arg PG_ARCH="$$PG_ARCH" -t "$$PG_IMAGE" "$$DEVOPS/docker/postgres"
  fi
  echo -e "$${BLUE}Starting Postgres on port $${PG_PORT}...$${NC}"
  docker run -d \
    --name "$$PG_CONTAINER" \
    --restart unless-stopped \
    -p "$${PG_PORT}:5432" \
    -e POSTGRES_PASSWORD="$$PG_PASSWORD" \
    -e PGDATA=/var/lib/postgresql/18/docker \
    -v "$${PG_VOLUME}:/var/lib/postgresql" \
    "$$PG_IMAGE" \
    postgres -c shared_preload_libraries=pg_search >/dev/null
fi

echo -e "$${BLUE}Waiting for Postgres to accept connections...$${NC}"
for i in $$(seq 1 60); do
  if docker exec "$$PG_CONTAINER" psql -U postgres -tAc "SELECT 1" >/dev/null 2>&1; then
    echo -e "$${GREEN}Postgres is ready.$${NC}"
    break
  fi
  if [ "$$i" -eq 60 ]; then
    echo -e "$${YELLOW}Postgres did not become ready in time. Check: docker logs $${PG_CONTAINER}$${NC}" >&2
    exit 1
  fi
  sleep 1
done

# Ensure extensions (idempotent; the backend also creates these on startup).
docker exec "$$PG_CONTAINER" psql -U postgres -q \
  -c "CREATE EXTENSION IF NOT EXISTS vector;" \
  -c "CREATE EXTENSION IF NOT EXISTS pg_search;" >/dev/null 2>&1 || true
endef
export POSTGRES_SCRIPT

define BACKEND_SCRIPT
set -e
GREEN='\033[0;32m'; BLUE='\033[0;34m'; YELLOW='\033[1;33m'; NC='\033[0m'

echo -e "$${BLUE}Starting Tradstry Backend Server$${NC}"
echo "=================================="

# dotenvy does not override existing env, so these take precedence over any
# POSTGRES_URL in backend/.env.
export POSTGRES_URL="postgres://postgres:$${PG_PASSWORD}@localhost:$${PG_PORT}/postgres"
export POSTGRES_DATABASE="$${POSTGRES_DATABASE:-dev}"
export RUST_BACKTRACE=full

cd "$$ROOT/backend"
echo "Current directory: $$(pwd)"
echo -e "$${GREEN}Starting Rust server with backtrace enabled...$${NC}"
echo -e "$${YELLOW}RUST_BACKTRACE is enabled for debugging$${NC}"
exec env RUST_LOG=info cargo run --bin tradstry-backend
endef
export BACKEND_SCRIPT

define FRONTEND_SCRIPT
cd "$$ROOT/apps/website"
[ -d node_modules ] || bun install
exec bun run dev
endef
export FRONTEND_SCRIPT

define DESKTOP_SCRIPT
set -e
GREEN='\033[0;32m'; BLUE='\033[0;34m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'

echo -e "$${BLUE}Starting Tradstry Desktop$${NC}"
echo "=================================="

if [ ! -d "$$ROOT/apps/desktop" ]; then
  echo -e "$${RED}Desktop app not found at $$ROOT/apps/desktop$${NC}" >&2
  exit 1
fi
if ! command -v bun >/dev/null 2>&1; then
  echo -e "$${RED}bun not found — install it from https://bun.sh$${NC}" >&2
  exit 1
fi

cd "$$ROOT/apps/desktop"
echo "Current directory: $$(pwd)"

if [ ! -d node_modules ]; then
  echo -e "$${BLUE}Installing dependencies (bun install)...$${NC}"
  bun install
fi

echo -e "$${YELLOW}Reminder: the backend should be running (make backend) for data to load.$${NC}"
echo -e "$${GREEN}Launching Electron desktop app (bun run dev)...$${NC}"
echo -e "$${YELLOW}First run downloads the local Electron runtime — this can take a few minutes.$${NC}"
exec bun run dev
endef
export DESKTOP_SCRIPT

define MICRO_SCRIPT
cd "$$ROOT/microservice/snaptrade-service"
exec go run main.go
endef
export MICRO_SCRIPT

define DEPLOY_SCRIPT
set -e
echo "Deploying to $$SERVER..."

echo "Syncing config files..."
ssh "$$SERVER" "mkdir -p $$REMOTE_DIR/backend $$REMOTE_DIR/microservice/snaptrade-service $$REMOTE_DIR/devops/bugsink $$REMOTE_DIR/devops/caddy $$REMOTE_DIR/devops/docker/postgres"
scp "$$DEVOPS/compose.yml" "$$SERVER:$$REMOTE_DIR/devops/compose.yml"
scp "$$DEVOPS/caddy/Caddyfile" "$$SERVER:$$REMOTE_DIR/devops/caddy/Caddyfile"
scp "$$DEVOPS/docker/postgres/Dockerfile" "$$SERVER:$$REMOTE_DIR/devops/docker/postgres/Dockerfile"

# Compose substitutes POSTGRES_PASSWORD/REDIS_PASSWORD from this at `up` time;
# without it both services abort rather than start on a default password.
scp "$$DEVOPS/.env" "$$SERVER:$$REMOTE_DIR/devops/.env"
scp "$$DEVOPS/countly.env" "$$SERVER:$$REMOTE_DIR/devops/countly.env"
scp "$$DEVOPS/countly-dashboard.env" "$$SERVER:$$REMOTE_DIR/devops/countly-dashboard.env"
scp "$$ROOT/backend/.env.production" "$$SERVER:$$REMOTE_DIR/backend/.env"
scp "$$ROOT/microservice/snaptrade-service/.env" "$$SERVER:$$REMOTE_DIR/microservice/snaptrade-service/.env"
scp "$$DEVOPS/bugsink/.env.production" "$$SERVER:$$REMOTE_DIR/devops/bugsink/.env.production"
ssh "$$SERVER" "chmod 600 $$REMOTE_DIR/devops/.env $$REMOTE_DIR/devops/countly.env $$REMOTE_DIR/devops/countly-dashboard.env $$REMOTE_DIR/backend/.env $$REMOTE_DIR/microservice/snaptrade-service/.env $$REMOTE_DIR/devops/bugsink/.env.production"

ssh "$$SERVER" bash -s <<'REMOTE'
set -e
cd /root/tradstry
compose() {
  docker compose --env-file devops/.env -f devops/compose.yml "$@"
}

echo "Pulling latest images..."
# tradstry-postgres is built on this box and lives in no registry, so its pull
# 404s. Ignore that rather than listing every other service by hand.
compose pull --ignore-pull-failures

echo "Restarting services..."
compose up -d --remove-orphans

echo ""
echo "Waiting for health checks..."
sleep 5
compose ps

# Caddy is a service in this stack now, so its config ships with the repo and
# reloads in place — no more appending vhosts to another project's Caddyfile.
compose exec -T caddy caddy reload --config /etc/caddy/Caddyfile
echo "Caddy reloaded"
REMOTE

echo ""
echo "Deploy complete."
echo ""
echo "Useful commands:"
echo "  ssh $$SERVER 'cd $$REMOTE_DIR && docker compose --env-file devops/.env -f devops/compose.yml logs -f'                    # all logs"
echo "  ssh $$SERVER 'cd $$REMOTE_DIR && docker compose --env-file devops/.env -f devops/compose.yml logs -f backend'            # backend logs"
echo "  ssh $$SERVER 'cd $$REMOTE_DIR && docker compose --env-file devops/.env -f devops/compose.yml logs -f snaptrade-service'  # snaptrade logs"
echo "  ssh $$SERVER 'cd $$REMOTE_DIR && docker compose --env-file devops/.env -f devops/compose.yml ps'                         # container status"
echo "  ssh $$SERVER 'cd $$REMOTE_DIR && docker compose --env-file devops/.env -f devops/compose.yml restart backend'            # restart backend"
endef
export DEPLOY_SCRIPT

define TAG_SCRIPT
set -e
cd "$$ROOT"

latest_tag=$$(git tag --sort=-v:refname | head -1)
if [ -z "$$latest_tag" ]; then
  latest_tag="v0.0.0"
fi
echo "Latest tag: $$latest_tag"

if [ -n "$$(git status --porcelain)" ]; then
  echo ""
  echo "Pending changes detected:"
  git status --short
  echo ""
  read -p "Commit message: " commit_msg
  if [ -z "$$commit_msg" ]; then
    echo "Error: commit message cannot be empty"
    exit 1
  fi
  git add .
  git commit -m "$$commit_msg"
  echo "Changes committed."
else
  echo "No pending changes."
fi

version="$${latest_tag#v}"
major=$$(echo "$$version" | cut -d. -f1)
minor=$$(echo "$$version" | cut -d. -f2)
patch=$$(echo "$$version" | cut -d. -f3)

# Optional 4th (build/hotfix) segment, e.g. v0.4.6.1 → build=1.
build=$$(echo "$$version" | cut -d. -f4)
if [[ "$$build" =~ ^[0-9]+$$ ]]; then
  hotfix_tag="v$${major}.$${minor}.$${patch}.$$((build + 1))"
else
  hotfix_tag="v$${major}.$${minor}.$${patch}.1"
fi

echo ""
echo "Current version: $$latest_tag"
echo "  1) patch  → v$${major}.$${minor}.$$((patch + 1))"
echo "  2) minor  → v$${major}.$$((minor + 1)).0"
echo "  3) major  → v$$((major + 1)).0.0"
echo "  4) hotfix → $${hotfix_tag}"
echo "  5) custom"
echo ""
read -p "Bump type [1/2/3/4/5] (default: 1): " bump

input="$${bump:-1}"
if [[ "$$input" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+(\.[0-9]+)?$$ ]]; then
  new_tag="v$${input#v}"
elif [ "$$input" = "1" ]; then
  new_tag="v$${major}.$${minor}.$$((patch + 1))"
elif [ "$$input" = "2" ]; then
  new_tag="v$${major}.$$((minor + 1)).0"
elif [ "$$input" = "3" ]; then
  new_tag="v$$((major + 1)).0.0"
elif [ "$$input" = "4" ]; then
  new_tag="$${hotfix_tag}"
elif [ "$$input" = "5" ]; then
  read -p "Enter version (e.g. v1.2.3 or v1.2.3.4): " new_tag
  if [[ ! "$$new_tag" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+(\.[0-9]+)?$$ ]]; then
    echo "Error: invalid version format. Use vX.Y.Z or vX.Y.Z.N"
    exit 1
  fi
  new_tag="v$${new_tag#v}"
else
  echo "Error: invalid option"
  exit 1
fi

echo ""
echo "Creating tag: $$new_tag"
git tag "$$new_tag"
git push origin "$$(git branch --show-current)" --tags
echo ""
echo "Pushed with tag $$new_tag"
endef
export TAG_SCRIPT

postgres:
	@bash -c "$$POSTGRES_SCRIPT"

backend: postgres
	@bash -c "$$BACKEND_SCRIPT"

frontend:
	@bash -c "$$FRONTEND_SCRIPT"

desktop:
	@bash -c "$$DESKTOP_SCRIPT"

micro:
	@bash -c "$$MICRO_SCRIPT"

deploy:
	@bash -c "$$DEPLOY_SCRIPT"

tag:
	@bash -c "$$TAG_SCRIPT"
