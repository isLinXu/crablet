#!/bin/bash

# Generated: 2026-03-14T10:30Z
# Rules-Ver: 3.0.2

# Color constants
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🧹 Crablet Deep Cleaner${NC}"
echo -e "This will remove build artifacts and free up disk space."

# 1. Clean Backend
echo -e "\n${YELLOW}[1/3] Cleaning Backend (Cargo)...${NC}"
if [ -d "crablet" ]; then
    cd crablet
    cargo clean
    cd ..
fi
echo "Backend target directory removed."

# 2. Clean Frontend
echo -e "\n${YELLOW}[2/3] Cleaning Frontend (npm)...${NC}"
if [ -d "frontend" ]; then
    rm -rf frontend/dist
    rm -rf frontend/node_modules
fi
echo "Frontend dist and node_modules removed."

# 3. Clean Temp Files
echo -e "\n${YELLOW}[3/3] Cleaning Temporary Files...${NC}"
rm -rf /tmp/crablet-target
rm -f .DS_Store
find . -name "*.log" -delete
find . -name "*.tmp" -delete
echo "Temporary files and logs removed."

echo -e "\n${GREEN}✅ Deep clean complete. Use ./install.sh to rebuild.${NC}"
df -h . | grep -v Filesystem
