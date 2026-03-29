#!/bin/bash

echo "🦀 Resolving Cargo Blocking/Locking Issues..."

# 1. Kill all running cargo processes
echo "[1/3] Terminating any stuck cargo processes..."
pkill -9 -f cargo || echo "No cargo processes found."
pkill -9 -f rustc || echo "No rustc processes found."

# 2. Remove Cargo global locks
echo "[2/3] Removing global Cargo registry locks..."
rm -f ~/.cargo/registry/index/*/.cargo-index-lock
rm -f ~/.cargo/registry/cache/*/.cargo-lock
rm -f ~/.cargo/.package-cache

# 3. Remove local project artifact locks
echo "[3/3] Removing local project artifact locks..."
if [ -d "crablet/target" ]; then
    rm -f crablet/target/.rustc_info.json
    rm -rf crablet/target/debug/.fingerprint
    rm -rf crablet/target/release/.fingerprint
    echo "Local target locks cleared."
else
    echo "No crablet/target directory found."
fi

echo "✅ All Cargo locks have been cleared!"
echo "You can now safely run ./install.sh again."
