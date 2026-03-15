#!/bin/bash

# Helper script to set up environment variables on the VPS
# Usage: ./setup-env.sh [client_id] [consumer_key]

set -e

# Replace with your VPS IP 
VPS_IP="95.216.219.137"
VPS_USER="root"
SSH_KEY="$HOME/.ssh/id_ed25519_vps"
COMPOSE_DIR="/opt/tradstry"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Check if SSH key exists
if [ ! -f "$SSH_KEY" ]; then
    echo -e "${RED}❌ SSH key not found at $SSH_KEY${NC}"
    exit 1
fi

# Get credentials from arguments or prompt
if [ -n "$1" ] && [ -n "$2" ]; then
    SNAPTRADE_CLIENT_ID="$1"
    SNAPTRADE_CONSUMER_KEY="$2"
else
    echo -e "${YELLOW}📝 Enter your SnapTrade credentials:${NC}"
    read -p "SNAPTRADE_CLIENT_ID: " SNAPTRADE_CLIENT_ID
    read -sp "SNAPTRADE_CONSUMER_KEY: " SNAPTRADE_CONSUMER_KEY
    echo ""
fi

if [ -z "$SNAPTRADE_CLIENT_ID" ] || [ -z "$SNAPTRADE_CONSUMER_KEY" ]; then
    echo -e "${RED}❌ Both SNAPTRADE_CLIENT_ID and SNAPTRADE_CONSUMER_KEY are required${NC}"
    exit 1
fi

echo -e "${BLUE}📋 Setting up .env file on VPS...${NC}"

# Create .env file on VPS
ssh -i "$SSH_KEY" -o StrictHostKeyChecking=no "$VPS_USER@$VPS_IP" << ENDSSH
mkdir -p $COMPOSE_DIR
cd $COMPOSE_DIR

cat > .env << ENVEOF
SNAPTRADE_CLIENT_ID=$SNAPTRADE_CLIENT_ID
SNAPTRADE_CONSUMER_KEY=$SNAPTRADE_CONSUMER_KEY
PORT=8080
ENVEOF

echo "✅ .env file created successfully"
cat .env
ENDSSH

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Environment variables set successfully on VPS!${NC}"
    echo ""
    echo -e "${YELLOW}📋 Next steps:${NC}"
    echo -e "${BLUE}  1. Run ./deploy.sh to deploy the service${NC}"
    echo -e "${BLUE}  2. Or restart the service: ssh -i $SSH_KEY $VPS_USER@$VPS_IP 'cd $COMPOSE_DIR && docker compose -f docker-compose.yml restart snaptrade-service'${NC}"
else
    echo -e "${RED}❌ Failed to set environment variables${NC}"
    exit 1
fi

