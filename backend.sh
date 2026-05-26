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
