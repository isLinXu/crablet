#!/bin/bash

# Generated: 2026-03-14T10:35Z
# Rules-Ver: 3.0.2

# Color constants
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🛠️  Crablet Debug Runner${NC}"

# 1. Load environment
if [ -f "crablet/.env" ]; then
    export $(grep -v '^#' crablet/.env | xargs)
fi

# Set default debug logs
export RUST_LOG=${RUST_LOG:-debug}
export RUST_BACKTRACE=1

echo -e "LOG LEVEL: ${GREEN}${RUST_LOG}${NC}"
echo -e "BACKTRACE: ${GREEN}Enabled${NC}"

# 2. Check for dependencies
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: cargo not found.${NC}"
    exit 1
fi

# 3. Start services in Debug Mode
echo -e "\n${YELLOW}Starting Backend in Debug mode...${NC}"
cd crablet
# Using cargo run instead of pre-built release binary
# Added --features knowledge as it's common
cargo run --features knowledge &
BACKEND_PID=$!
cd ..

echo -e "${GREEN}Backend started with PID: $BACKEND_PID${NC}"

# 4. Optional: Frontend Dev Mode
echo -e "\n${YELLOW}Do you want to start Frontend in Dev mode? (y/N)${NC}"
read -t 5 -n 1 -r REPLY
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "Starting Frontend Dev server..."
    cd frontend
    npm run dev &
    FRONTEND_PID=$!
    cd ..
    echo -e "${GREEN}Frontend Dev server started with PID: $FRONTEND_PID${NC}"
else
    echo -e "Skipping Frontend Dev mode. Using built dist if available."
fi

echo -e "\n${BLUE}Services are running. Press Ctrl+C to stop.${NC}"

# Wait for backend to finish (or Ctrl+C)
wait $BACKEND_PID
