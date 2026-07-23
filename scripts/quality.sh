#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$repo_root/crablet/Cargo.toml"

cargo fmt --manifest-path "$manifest" --all -- --check
cargo check --manifest-path "$manifest" --locked
cargo check --manifest-path "$manifest" --locked --no-default-features
cargo test --manifest-path "$manifest" --locked --lib --no-default-features --features web
