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

# Function to check and kill port usage
check_and_kill_port() {
    local port=$1
    if command -v lsof >/dev/null 2>&1; then
        local pid=$(lsof -ti :$port)
        if [ -n "$pid" ]; then
            echo -e "${RED}⚠️  Port $port is in use by PID $pid. Killing process...${NC}"
            kill -9 $pid
            sleep 1
        fi
    else
        echo -e "${RED}Warning: 'lsof' not found. Cannot check port $port usage automatically.${NC}"
    fi
}

# Check ports before starting
check_and_kill_port 3000
check_and_kill_port 18789

# Function to kill child processes on exit
cleanup() {
    echo -e "\n${RED}🛑 Stopping services...${NC}"
    kill $(jobs -p) 2>/dev/null
    exit
}
trap cleanup SIGINT SIGTERM

# Determine binary path (Release > Debug)
if [ -f "crablet/target/release/crablet" ]; then
    BINARY="./target/release/crablet"
    echo -e "${GREEN}Using Release Build${NC}"
elif [ -f "crablet/target/debug/crablet" ]; then
    BINARY="./target/debug/crablet"
    echo -e "${RED}Warning: Release build not found. Using Debug build (slower).${NC}"
else
    echo -e "${RED}Error: Crablet binary not found. Please run ./install.sh first.${NC}"
    exit 1
fi

# Start Web Server (Static + Basic API)
echo -e "${BLUE}[1/2] Starting Web Server (Port 3000)...${NC}"
(cd crablet && $BINARY serve-web --port 3000) &
WEB_PID=$!

# Wait for Web Server to initialize (simple sleep or health check)
sleep 2

# Start Gateway (Streaming + Advanced API)
echo -e "${BLUE}[2/2] Starting Gateway (Port 18789)...${NC}"
(cd crablet && $BINARY gateway --port 18789) &
GATEWAY_PID=$!

echo -e "${GREEN}✨ All services started!${NC}"
echo -e "Frontend/Web UI: ${BLUE}http://localhost:3000${NC}"
echo -e "Gateway API:     ${BLUE}http://localhost:18789${NC}"
echo -e "Press Ctrl+C to stop."

# Wait for both processes
wait $WEB_PID $GATEWAY_PID
