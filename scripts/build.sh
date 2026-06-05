#!/bin/bash

# Generated: 2026-03-14T10:35Z
# Rules-Ver: 3.0.2

# Color constants
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔨 Crablet Builder${NC}"

# Parse arguments
BUILD_MODE="release"
FEATURES="knowledge,web"
BACKEND_ONLY=false
FRONTEND_ONLY=false
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --debug) BUILD_MODE="debug" ;;
        --release) BUILD_MODE="release" ;;
        --features) FEATURES="$2"; shift ;;
        --backend-only) BACKEND_ONLY=true ;;
        --frontend-only) FRONTEND_ONLY=true ;;
        *) echo "Unknown parameter: $1"; exit 1 ;;
    esac
    shift
done

echo -e "BUILD MODE: ${GREEN}${BUILD_MODE}${NC}"
echo -e "FEATURES:   ${GREEN}${FEATURES}${NC}"

# 1. Build Backend
if [ "$FRONTEND_ONLY" = false ]; then
    echo -e "\n${YELLOW}[1/2] Building Backend...${NC}"
    cd crablet
    if [ "$BUILD_MODE" = "release" ]; then
        cargo build --release --features "$FEATURES"
    else
        cargo build --features "$FEATURES"
    fi
    cd ..
    echo -e "${GREEN}Backend built successfully.${NC}"
fi

# 2. Build Frontend
if [ "$BACKEND_ONLY" = false ]; then
    echo -e "\n${YELLOW}[2/2] Building Frontend...${NC}"
    cd frontend
    npm install
    npm run build
    cd ..
    echo -e "${GREEN}Frontend built successfully.${NC}"
fi

echo -e "\n${GREEN}✅ Build process complete.${NC}"
