#!/bin/bash

# Generated: 2026-03-14T10:30Z
# Rules-Ver: 3.0.2

# Color constants
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${RED}🗑️  Crablet Uninstaller${NC}"
echo -e "This will stop services and remove build artifacts."

# Parse arguments
FULL_CLEAN=false
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --full) FULL_CLEAN=true ;;
        *) echo "Unknown parameter: $1"; exit 1 ;;
    esac
    shift
done

# 1. Stop running services
echo -e "\n${YELLOW}[1/4] Stopping services...${NC}"
pkill -f "crablet" || true
pkill -f "node" || true
echo "Services stopped."

# 2. Clean Backend
echo -e "\n${YELLOW}[2/4] Cleaning Backend (Cargo)...${NC}"
if [ -d "crablet" ]; then
    cd crablet
    cargo clean
    cd ..
fi
echo "Backend artifacts removed."

# 3. Clean Frontend
echo -e "\n${YELLOW}[3/4] Cleaning Frontend (npm)...${NC}"
if [ -d "frontend" ]; then
    rm -rf frontend/dist
    rm -rf frontend/node_modules
fi
echo "Frontend artifacts removed."

# 4. Remove Database and Environment (Optional)
if [ "$FULL_CLEAN" = true ]; then
    echo -e "\n${RED}[4/4] Performing FULL clean...${NC}"
    rm -f crablet/crablet.db
    rm -f crablet/crablet.db-shm
    rm -f crablet/crablet.db-wal
    rm -f crablet/.env
    echo "Database and environment files removed."
else
    echo -e "\n${YELLOW}[4/4] Skipping database and environment files.${NC}"
    echo "Use --full to remove .env and crablet.db"
fi

echo -e "\n${GREEN}✅ Uninstallation complete.${NC}"
