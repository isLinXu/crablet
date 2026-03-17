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
UNINSTALL=0
DEBUG=0
BUILD=0
for arg in "$@"; do
    case "$arg" in
        --start-only)
            START_ONLY=1
            ;;
        --non-interactive)
            NON_INTERACTIVE=1
            ;;
        --uninstall)
            UNINSTALL=1
            ;;
        --debug)
            DEBUG=1
            ;;
        --build)
            BUILD=1
            ;;
        --status)
            ./status.sh
            exit 0
            ;;
        --clean)
            ./clean.sh
            exit 0
            ;;
        *)
            echo -e "${RED}Error: Unknown option ${arg}${NC}"
            echo "Usage: ./one-click.sh [--start-only] [--non-interactive] [--uninstall] [--status] [--clean] [--debug] [--build]"
            exit 1
            ;;
    esac
done

if [ "$UNINSTALL" -eq 1 ]; then
    echo -e "${RED}⚠️  Running uninstallation...${NC}"
    ./uninstall.sh
    exit 0
fi

if [ "$DEBUG" -eq 1 ]; then
    echo -e "${YELLOW}🛠️  Running in debug mode...${NC}"
    ./debug.sh
    exit 0
fi

if [ "$BUILD" -eq 1 ]; then
    echo -e "${YELLOW}🔨 Running build...${NC}"
    ./build.sh
    exit 0
fi

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
