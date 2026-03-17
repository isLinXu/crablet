#!/bin/bash

# Generated: 2026-03-14T10:30Z
# Rules-Ver: 3.0.2

# Color constants
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔍 Checking Crablet Service Status${NC}"

# Check Backend
echo -e "\n${YELLOW}[1/3] Backend Service Check...${NC}"
BACKEND_PID=$(pgrep -x "crablet" | head -n 1)
if [ -z "$BACKEND_PID" ]; then
    # Try pgrep -f if -x fails (e.g. if it's a script or has different process name)
    BACKEND_PID=$(pgrep -f "target/.*/crablet" | head -n 1)
fi

if [ -n "$BACKEND_PID" ]; then
    echo -e "${GREEN}✅ Backend (Gateway) is running (PID: $BACKEND_PID)${NC}"
    # Try to ping port 18789 (Gateway)
    if curl -s http://127.0.0.1:18789/health > /dev/null; then
        echo -e "${GREEN}✅ Gateway API is healthy${NC}"
    else
        echo -e "${RED}⚠️ Gateway process found but port 18789 is not responding.${NC}"
    fi
else
    echo -e "${RED}❌ Backend (Gateway) is NOT running${NC}"
fi

# Check Frontend
echo -e "\n${YELLOW}[2/3] Frontend Service Check...${NC}"
if [ -n "$BACKEND_PID" ]; then
    if curl -s http://localhost:3000 > /dev/null; then
        echo -e "${GREEN}✅ Web UI is accessible at http://localhost:3000${NC}"
    else
        # Check if it's running in dev mode
        DEV_PID=$(pgrep -f "vite" | head -n 1)
        if [ -n "$DEV_PID" ]; then
             echo -e "${GREEN}✅ Web UI is running in DEV mode (Vite)${NC}"
        else
             echo -e "${RED}⚠️ Web UI port (3000) is NOT responding.${NC}"
        fi
    fi
else
    echo -e "${RED}❌ Web UI process NOT found (Backend is down)${NC}"
fi

# Check MCP Servers (optional)
echo -e "\n${YELLOW}[3/3] MCP Server Check...${NC}"
if pgrep -f "math_server" > /dev/null; then
    echo -e "${GREEN}✅ Math MCP Server is running${NC}"
else
    echo -e "${RED}❌ Math MCP Server is NOT running${NC}"
fi

if pgrep -f "test_server" > /dev/null; then
    echo -e "${GREEN}✅ Test MCP Server is running${NC}"
else
    echo -e "${RED}❌ Test MCP Server is NOT running${NC}"
fi

echo -e "\n${BLUE}💡 Tip: Use ./start.sh to launch all services.${NC}"
