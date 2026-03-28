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
        local pids
        pids=$(lsof -ti :"$port" | tr '\n' ' ' | xargs)
        if [ -n "$pids" ]; then
            echo -e "${RED}⚠️  Port $port is in use by PID(s) ${pids}. Killing process...${NC}"
            kill -9 $pids
            sleep 1
        fi
    else
        echo -e "${RED}Warning: 'lsof' not found. Cannot check port $port usage automatically.${NC}"
    fi
}

# Default ports (can be overridden by environment variables)
WEB_PORT=${CRABLET_WEB_PORT:-3333}
GATEWAY_PORT=${CRABLET_GATEWAY_PORT:-18790}

# Check ports before starting
check_and_kill_port $WEB_PORT
check_and_kill_port $GATEWAY_PORT

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
echo -e "${BLUE}[1/2] Starting Web Server (Port $WEB_PORT)...${NC}"
(cd crablet && CRABLET_SERVE_WEB_START_GATEWAY=false $BINARY serve-web --port $WEB_PORT) &
WEB_PID=$!

sleep 2
if ! kill -0 "$WEB_PID" 2>/dev/null; then
    echo -e "${RED}Error: Web Server failed to start. Please check logs above.${NC}"
    wait "$WEB_PID"
    exit 1
fi

echo -e "${BLUE}[2/2] Starting Gateway (Port $GATEWAY_PORT)...${NC}"
(cd crablet && CRABLET_AUTH_MODE=off $BINARY gateway --port $GATEWAY_PORT) &
GATEWAY_PID=$!
sleep 1
if ! kill -0 "$GATEWAY_PID" 2>/dev/null; then
    echo -e "${RED}Error: Gateway failed to start. Please check logs above.${NC}"
    kill "$WEB_PID" 2>/dev/null
    wait "$GATEWAY_PID"
    exit 1
fi

echo -e "${GREEN}✨ All services started!${NC}"
echo -e "Frontend/Web UI: ${BLUE}http://localhost:$WEB_PORT${NC}"
echo -e "Gateway API:     ${BLUE}http://localhost:$GATEWAY_PORT${NC}"
echo -e "Press Ctrl+C to stop."

# Wait for both processes
wait $WEB_PID $GATEWAY_PID
