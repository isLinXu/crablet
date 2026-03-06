#!/bin/bash

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if .env exists
if [ ! -f crablet/.env ]; then
    echo -e "${RED}Error: crablet/.env file not found. Please run ./install.sh first.${NC}"
    exit 1
fi

echo -e "${GREEN}🚀 Starting Crablet Services...${NC}"

# Function to kill child processes on exit
cleanup() {
    echo -e "\n${RED}🛑 Stopping services...${NC}"
    kill $(jobs -p) 2>/dev/null
    exit
}
trap cleanup SIGINT SIGTERM

# Start Web Server (Static + Basic API)
echo -e "${BLUE}[1/2] Starting Web Server (Port 3000)...${NC}"
(cd crablet && ./target/release/crablet serve-web --port 3000) &
WEB_PID=$!

# Wait for Web Server to initialize (simple sleep or health check)
sleep 2

# Start Gateway (Streaming + Advanced API)
echo -e "${BLUE}[2/2] Starting Gateway (Port 18789)...${NC}"
(cd crablet && ./target/release/crablet gateway --port 18789) &
GATEWAY_PID=$!

echo -e "${GREEN}✨ All services started!${NC}"
echo -e "Frontend/Web UI: ${BLUE}http://localhost:3000${NC}"
echo -e "Gateway API:     ${BLUE}http://localhost:18789${NC}"
echo -e "Press Ctrl+C to stop."

# Wait for both processes
wait $WEB_PID $GATEWAY_PID
