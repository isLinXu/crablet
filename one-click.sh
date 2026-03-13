#!/bin/bash
set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT_DIR"

if [ ! -f "./install.sh" ] || [ ! -f "./start.sh" ]; then
    echo -e "${RED}Error: install.sh or start.sh not found in project root.${NC}"
    exit 1
fi

START_ONLY=0
NON_INTERACTIVE=0
for arg in "$@"; do
    case "$arg" in
        --start-only)
            START_ONLY=1
            ;;
        --non-interactive)
            NON_INTERACTIVE=1
            ;;
        *)
            echo -e "${RED}Error: Unknown option ${arg}${NC}"
            echo "Usage: ./one-click.sh [--start-only] [--non-interactive]"
            exit 1
            ;;
    esac
done

if [ "$START_ONLY" -eq 0 ]; then
    echo -e "${GREEN}📦 Running installation...${NC}"
    if [ "$NON_INTERACTIVE" -eq 1 ]; then
        ./install.sh --non-interactive
    else
        ./install.sh
    fi
fi

echo -e "${GREEN}🚀 Starting services...${NC}"
./start.sh
