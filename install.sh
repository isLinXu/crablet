#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}🦀 Crablet One-Click Installer${NC}"

# Function to check command existence
check_cmd() {
    if ! command -v "$1" &> /dev/null; then
        echo -e "${RED}Error: $1 is not installed.${NC}"
        return 1
    fi
    return 0
}

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
    echo "Node.js is installed: $(node --version)"
    echo "npm is installed: $(npm --version)"
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
    # Basic template, user should edit it
    cat > .env <<EOL
# Server
PORT=3000
RUST_LOG=info
DATABASE_URL=sqlite:crablet.db?mode=rwc

# Security (Auto-generated secret)
JWT_SECRET=$(openssl rand -hex 32)

# AI Providers (Fill your keys)
DASHSCOPE_API_KEY=
OPENAI_API_BASE=https://dashscope.aliyuncs.com/compatible-mode/v1
OLLAMA_MODEL=qwen2.5:14b
EOL
    echo -e "${RED}Warning: Created .env file. Please edit it with your API Keys!${NC}"
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
