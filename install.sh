#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}🦀 Crablet One-Click Installer${NC}"

NON_INTERACTIVE=0
for arg in "$@"; do
    case "$arg" in
        --non-interactive)
            NON_INTERACTIVE=1
            ;;
        *)
            echo -e "${RED}Error: Unknown option ${arg}${NC}"
            echo "Usage: ./install.sh [--non-interactive]"
            exit 1
            ;;
    esac
done

# Function to check command existence
check_cmd() {
    if ! command -v "$1" &> /dev/null; then
        echo -e "${RED}Error: $1 is not installed.${NC}"
        return 1
    fi
    return 0
}

if [ -x "/opt/homebrew/opt/node@24/bin/node" ]; then
    export PATH="/opt/homebrew/opt/node@24/bin:$PATH"
fi

# 1. Check Rust Environment
echo -e "\n${GREEN}[1/4] Checking Rust environment...${NC}"
if check_cmd cargo; then
    echo "Rust is installed: $(cargo --version)"
else
    echo "Rust not found. Please install Rust via https://rustup.rs/"
    exit 1
fi

# 2. Check Node.js Environment
echo -e "\n${GREEN}[2/4] Checking Node.js environment...${NC}"
if check_cmd node && check_cmd npm; then
    NODE_VERSION=$(node --version | sed 's/^v//')
    NODE_MAJOR=$(echo "$NODE_VERSION" | cut -d. -f1)
    NODE_MINOR=$(echo "$NODE_VERSION" | cut -d. -f2)
    echo "Node.js is installed: v$NODE_VERSION"
    echo "npm is installed: $(npm --version)"
    if [ "$NODE_MAJOR" -lt 20 ] || \
       { [ "$NODE_MAJOR" -eq 20 ] && [ "$NODE_MINOR" -lt 19 ]; } || \
       { [ "$NODE_MAJOR" -eq 21 ]; } || \
       { [ "$NODE_MAJOR" -eq 22 ] && [ "$NODE_MINOR" -lt 13 ]; } || \
       { [ "$NODE_MAJOR" -eq 23 ]; }; then
        echo -e "${RED}Error: Unsupported Node.js version for frontend dependencies: v$NODE_VERSION${NC}"
        echo "Please use Node.js 20.19+, 22.13+, or 24+ (LTS recommended), then rerun ./install.sh"
        exit 1
    fi
else
    echo "Node.js/npm not found. Please install Node.js."
    exit 1
fi

# 3. Install Backend Dependencies (Rust)
echo -e "\n${GREEN}[3/4] Building Backend (Crablet)...${NC}"
cd crablet
# Check for sqlx-cli
if ! command -v sqlx &> /dev/null; then
    echo "Installing sqlx-cli..."
    cargo install sqlx-cli --no-default-features --features native-tls,sqlite
fi

# Create .env if not exists
if [ ! -f .env ]; then
    echo "Creating .env from template..."
    cat > .env <<EOL
# Server
PORT=3000
RUST_LOG=info
DATABASE_URL=sqlite:crablet.db?mode=rwc

# Security (Auto-generated secret)
JWT_SECRET=$(openssl rand -hex 32)

# AI Providers (Fill your keys)
DASHSCOPE_API_KEY=
OPENAI_API_KEY=
OPENAI_API_BASE=https://dashscope.aliyuncs.com/compatible-mode/v1
OPENAI_MODEL_NAME=qwen-plus
OLLAMA_MODEL=qwen2.5:14b
EOL
    echo -e "${RED}Warning: Created .env file. Please edit it with your API Keys!${NC}"
fi

if ! grep -q '^DATABASE_URL=' .env; then
    echo "DATABASE_URL not found in .env, writing default value..."
    echo "DATABASE_URL=sqlite:crablet.db?mode=rwc" >> .env
fi

DATABASE_URL=$(grep -E '^DATABASE_URL=' .env | head -n 1 | cut -d '=' -f2- | tr -d '\r')
DATABASE_URL="${DATABASE_URL%\"}"
DATABASE_URL="${DATABASE_URL#\"}"
DATABASE_URL="${DATABASE_URL%\'}"
DATABASE_URL="${DATABASE_URL#\'}"
if [ -z "$DATABASE_URL" ]; then
    echo -e "${RED}Error: DATABASE_URL is empty in .env${NC}"
    exit 1
fi
export DATABASE_URL

if [ "$NON_INTERACTIVE" -eq 0 ] && [ -t 0 ]; then
    ../settings.sh
else
    ../settings.sh --non-interactive
fi

# Setup Database
if [ ! -f crablet.db ]; then
    echo "Initializing Database..."
    sqlx database create
    sqlx migrate run
fi

echo "Compiling Release Build..."
cargo build --release
cd ..

# 4. Install Frontend Dependencies
echo -e "\n${GREEN}[4/4] Installing Frontend Dependencies...${NC}"
cd frontend
npm install
echo "Building Frontend..."
npm run build
cd ..

echo -e "\n${GREEN}✅ Installation Complete!${NC}"
echo -e "Run ${GREEN}./start.sh${NC} to start the services."
