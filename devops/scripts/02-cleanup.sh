#!/usr/bin/env bash
set -euo pipefail

# Phase 2: remove what the cutover made redundant, plus the dead projects.
#
# Run this ONLY after 01-cutover.sh has been verified green — it deletes the
# renamed *-old-* containers that are the rollback path, and the meeting-bot
# Caddy volume that holds the original TLS certificates.
#
# Removed here:
#   - the *-old-* containers left by the cutover
#   - meeting-bot: containers, volumes, image, /opt/meeting-bot
#   - Typesense: container, volume, image (zero references in the codebase,
#     zero active connections when audited)
#   - /root/Zaned, /opt/zaned, /root/migrate-zaned-warehouse
#   - remote-dev residue: .cursor-server, .warp, .claude, .claude.json
#   - dangling images and build cache
#
# Usage: ./devops/scripts/02-cleanup.sh [ssh-host]   (default: myserver)

HOST="${1:-myserver}"
say() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }

say "Confirm the new stack is healthy before deleting the rollback path"
ssh "$HOST" bash -euo pipefail -s <<'GUARD'
cd /root/tradstry
compose() { docker compose --env-file devops/.env -f devops/compose.yml "$@"; }
for svc in postgres redis caddy backend mcp-server; do
  state=$(compose ps --format json "$svc" 2>/dev/null | python3 -c 'import sys,json
d=[json.loads(l) for l in sys.stdin if l.strip()]
print(d[0].get("State","missing") if d else "missing")' 2>/dev/null || echo missing)
  printf '  %-14s %s\n' "$svc" "$state"
  [ "$state" = "running" ] || { echo "ABORT: $svc is not running"; exit 1; }
done
for u in https://backend.tradstry.com/health https://mcp.tradstry.com/health; do
  code=$(curl -s -o /dev/null -w '%{http_code}' -m 15 "$u" || echo 000)
  printf '  %-40s %s\n' "$u" "$code"
  case "$code" in 2*|3*) ;; *) echo "ABORT: $u returned $code"; exit 1 ;; esac
done
echo "  healthy — safe to proceed"
GUARD

say "Disk before"
ssh "$HOST" 'df -h / | tail -1; docker system df'

say "Removing superseded and dead containers"
ssh "$HOST" bash -euo pipefail -s <<'RM_CONTAINERS'
# The renamed originals from the cutover.
for c in $(docker ps -a --format '{{.Names}}' | grep -- '-old-' || true); do
  docker rm -f "$c" >/dev/null && echo "  removed $c"
done

# meeting-bot — the app has been down for months and meet.tradstry.com 502s.
if [ -d /opt/meeting-bot ]; then
  (cd /opt/meeting-bot && docker compose down --volumes --remove-orphans 2>/dev/null) || true
fi
for c in meeting-bot-caddy-1 meeting-bot-backend-1; do
  docker rm -f "$c" >/dev/null 2>&1 && echo "  removed $c" || true
done

# Typesense — nothing in the Tradstry codebase references it.
docker rm -f typesense >/dev/null 2>&1 && echo "  removed typesense" || true
RM_CONTAINERS

say "Removing volumes, images and networks"
ssh "$HOST" bash -euo pipefail -s <<'RM_RESOURCES'
for v in typesense_data meeting-bot_caddy_data meeting-bot_caddy_config; do
  docker volume rm "$v" >/dev/null 2>&1 && echo "  volume $v" || true
done

for i in typesense/typesense:27.1 johnsonf/meeting-bot:latest redis:latest tradstry-claude:latest; do
  docker image rm "$i" >/dev/null 2>&1 && echo "  image $i" || true
done

docker network rm meeting-bot_default >/dev/null 2>&1 && echo "  network meeting-bot_default" || true

# Volumes with hex names, unattached to any container — the 4 unused ones seen
# in the audit. Named volumes still in use are never touched by this.
docker volume ls -qf dangling=true | grep -E '^[0-9a-f]{64}$' | while read -r v; do
  docker volume rm "$v" >/dev/null 2>&1 && echo "  orphan volume ${v:0:12}" || true
done
RM_RESOURCES

say "Removing directories"
ssh "$HOST" bash -euo pipefail -s <<'RM_DIRS'
for d in /opt/meeting-bot /root/Zaned /opt/zaned /root/migrate-zaned-warehouse \
         /root/.cursor-server /root/.warp /root/.claude; do
  if [ -e "$d" ]; then
    sz=$(du -sh "$d" 2>/dev/null | cut -f1)
    rm -rf "$d" && echo "  $d ($sz)"
  fi
done
rm -f /root/.claude.json && echo "  /root/.claude.json"
RM_DIRS

say "Reclaiming Docker images and build cache"
ssh "$HOST" 'docker image prune -af; docker builder prune -af'

say "Capping logs"
ssh "$HOST" bash -euo pipefail -s <<'LOGS'
# 464 MB of these are failed SSH logins; phase 3 stops them being written at all.
: > /var/log/btmp
rm -f /var/log/btmp.1
journalctl --vacuum-size=200M
mkdir -p /etc/systemd/journald.conf.d
cat > /etc/systemd/journald.conf.d/size.conf <<'CONF'
[Journal]
SystemMaxUse=200M
CONF
systemctl restart systemd-journald
LOGS

say "Disk after"
ssh "$HOST" 'df -h / | tail -1; docker system df; echo; docker ps --format "table {{.Names}}\t{{.Status}}"'
