#!/bin/bash

# Tradstry Desktop Startup Script
# Navigates to the desktop app (tradstry/) and starts it in dev mode via Tauri.

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}Starting Tradstry Desktop${NC}"
echo "=================================="

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DESKTOP_DIR="$SCRIPT_DIR/tradstry"

if [ ! -d "$DESKTOP_DIR" ]; then
  echo -e "${RED}Desktop app not found at $DESKTOP_DIR${NC}" >&2
  exit 1
fi

if ! command -v bun >/dev/null 2>&1; then
  echo -e "${RED}bun not found — install it from https://bun.sh${NC}" >&2
  exit 1
fi

# Navigate to the desktop app directory
echo -e "${BLUE}Navigating to desktop app directory...${NC}"
cd "$DESKTOP_DIR"
echo "Current directory: $(pwd)"

# Install JS dependencies if they're missing
if [ ! -d node_modules ]; then
  echo -e "${BLUE}Installing dependencies (bun install)...${NC}"
  bun install
fi

# The desktop app talks to the backend — make sure it's running (./backend.sh)
echo -e "${YELLOW}Reminder: the backend should be running (./backend.sh) for data to load.${NC}"

# Launch the Tauri dev app. First run compiles the Rust side (a few minutes).
echo -e "${GREEN}Launching Tauri desktop app (bun run tauri dev)...${NC}"
echo -e "${YELLOW}First run compiles the Rust side — this can take a few minutes.${NC}"
exec bun run tauri dev
