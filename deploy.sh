#!/bin/bash

set -e

SERVER="myserver"
REMOTE_DIR="/root/tradstry"
CADDY_CONTAINER="meeting-bot-caddy-1"
CADDYFILE="/opt/meeting-bot/Caddyfile"

echo "Deploying to $SERVER..."

# Copy docker-compose.yml to server
echo "Syncing config files..."
ssh "$SERVER" "mkdir -p $REMOTE_DIR/backend $REMOTE_DIR/microservice/snaptrade-service"
scp docker-compose.yml "$SERVER:$REMOTE_DIR/docker-compose.yml"

# Copy env files
scp backend/.env.production "$SERVER:$REMOTE_DIR/backend/.env"
scp microservice/snaptrade-service/.env "$SERVER:$REMOTE_DIR/microservice/snaptrade-service/.env"

# Deploy on server
ssh "$SERVER" bash -s <<'EOF'
set -e
cd /root/tradstry

echo "Pulling latest images..."
docker compose pull

echo "Restarting services..."
docker compose up -d --remove-orphans

echo ""
echo "Waiting for health checks..."
sleep 5
docker compose ps

# Add backend.tradstry.com to shared Caddy if not already there
CADDYFILE="/opt/meeting-bot/Caddyfile"
if ! grep -q "backend.tradstry.com" "$CADDYFILE"; then
  echo "" >> "$CADDYFILE"
  echo "backend.tradstry.com {" >> "$CADDYFILE"
  echo "	reverse_proxy tradstry-backend:7899" >> "$CADDYFILE"
  echo "}" >> "$CADDYFILE"
  echo "Added backend.tradstry.com to Caddyfile"
fi

# Connect Caddy to tradstry network so it can reach the backend
docker network connect tradstry_tradstry-network meeting-bot-caddy-1 2>/dev/null || true

# Reload Caddy config
docker exec meeting-bot-caddy-1 caddy reload --config /etc/caddy/Caddyfile
echo "Caddy reloaded"
EOF

echo ""
echo "Deploy complete."
echo ""
echo "Useful commands:"
echo "  ssh $SERVER 'cd $REMOTE_DIR && docker compose logs -f'              # all logs"
echo "  ssh $SERVER 'cd $REMOTE_DIR && docker compose logs -f backend'      # backend logs"
echo "  ssh $SERVER 'cd $REMOTE_DIR && docker compose logs -f snaptrade-service'  # snaptrade logs"
echo "  ssh $SERVER 'cd $REMOTE_DIR && docker compose ps'                   # container status"
echo "  ssh $SERVER 'cd $REMOTE_DIR && docker compose restart backend'      # restart backend"
