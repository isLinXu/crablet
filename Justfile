# Justfile for Crablet

# Minimal build
build-minimal:
    cargo build --manifest-path ./crablet/Cargo.toml --no-default-features --features web

# Full build
build-full:
    cargo build --manifest-path ./crablet/Cargo.toml --release

# Run tests
test:
    cargo test --manifest-path ./crablet/Cargo.toml --locked --no-default-features --features web

# Run all tests
test-all:
    cargo test --manifest-path ./crablet/Cargo.toml --locked --all-features

# Backend web suite used by CI
test-web:
    cargo test --manifest-path ./crablet/Cargo.toml --locked --no-default-features --features web

# Backend doctests used by CI
test-doc:
    cargo test --manifest-path ./crablet/Cargo.toml --locked --doc --no-default-features --features web

# Fast all-features compile validation
check-all-features:
    cargo check --manifest-path ./crablet/Cargo.toml --locked --all-features

# Single local backend quality gate (no extra system tools required)
quality:
    bash ./scripts/quality.sh

# Lint
lint:
    cargo fmt --manifest-path ./crablet/Cargo.toml --all -- --check
    cargo clippy --manifest-path ./crablet/Cargo.toml --locked --all-targets --no-default-features --features web -- -D warnings

# Audit
audit:
    cargo audit --manifest-path ./crablet/Cargo.toml
    cargo deny check --manifest-path ./crablet/Cargo.toml

# CI-oriented security audit
audit-ci:
    cd ./crablet && cargo audit --no-fetch --stale
    cd ./frontend && npm audit --omit=dev --audit-level=high

# Coverage
coverage:
    mkdir -p ./crablet/coverage
    cargo llvm-cov --manifest-path ./crablet/Cargo.toml --workspace --no-default-features --features web --html --output-dir ./crablet/coverage/html

# Generate LCOV report for CI-style gate checks
coverage-lcov:
    mkdir -p ./crablet/coverage
    cargo llvm-cov --manifest-path ./crablet/Cargo.toml --workspace --no-default-features --features web --lcov --output-path ./crablet/coverage/lcov.info

# Enforce the same LCOV threshold used in CI
coverage-gate threshold="80":
    bash ./scripts/check_lcov_threshold.sh ./crablet/coverage/lcov.info {{threshold}} backend-web

# Frontend lint
frontend-lint:
    cd ./frontend && npm run lint

# Frontend CI lint gate
frontend-lint-ci:
    cd ./frontend && npm run lint:ci

# Frontend type check
frontend-typecheck:
    cd ./frontend && npm run type-check

# Frontend tests
frontend-test:
    cd ./frontend && npm run test:ci

# Frontend coverage report
frontend-coverage:
    cd ./frontend && npm run test:coverage

# Frontend production build
frontend-build:
    cd ./frontend && npm run build

# Local CI smoke check
ci-smoke:
    just test-web
    just test-doc
    just check-all-features
    just build-minimal
    just frontend-lint-ci
    just frontend-typecheck
    just frontend-build
    just frontend-test

# Validate the docker-compose wiring with CI-safe defaults
compose-validate:
    NEO4J_PASSWORD=ci-password OPENAI_API_KEY=sk-test MOONSHOT_API_KEY=sk-test ZHIPU_API_KEY=sk-test OPENAI_API_BASE=https://api.openai.com/v1 OPENAI_MODEL_NAME=gpt-4o-mini docker compose -f docker-compose.yml config -q

# Reproduce the container build validation job locally
docker-check:
    docker build -t crablet:ci .

# Full local CI/CD smoke check
ci-smoke-full:
    just ci-smoke
    just compose-validate
    just docker-check

# Dev Server
dev:
    cargo run --manifest-path ./crablet/Cargo.toml --no-default-features --features web -- serve-web --port 3000

# Docker Build
docker-build:
    docker build -t crablet:latest .

# Up
up:
    docker compose up -d

# ═══════════════════════════════════════════════════════════════════════════
# Desktop (Tauri 2) — 统一使用 scripts/pack.sh
# ═══════════════════════════════════════════════════════════════════════════

# Build sidecar binary (release, web-only)
desktop-sidecar:
    cargo build --release -p crablet --no-default-features --features web

# Copy sidecar binary to desktop/binaries/ (auto-detect platform)
desktop-sidecar-copy: desktop-sidecar
    mkdir -p desktop/binaries
    #!/usr/bin/env bash
    set -euo pipefail
    os=$(uname -s)
    arch=$(uname -m)
    case "$os" in
        Darwin)
            target="aarch64-apple-darwin"
            if [ "$arch" = "x86_64" ]; then target="x86_64-apple-darwin"; fi
            ;;
        Linux)
            target="x86_64-unknown-linux-gnu"
            if [ "$arch" = "aarch64" ]; then target="aarch64-unknown-linux-gnu"; fi
            ;;
        MINGW*|CYGWIN*|MSYS*)
            target="x86_64-pc-windows-msvc"
            ;;
        *)
            target="unknown"
            ;;
    esac
    cp target/release/crablet "desktop/binaries/crablet-$target"
    chmod +x "desktop/binaries/crablet-$target"
    echo "Copied sidecar for $target"

# ═══════════════════════════════════════════════════════════════════════════
# 一键打包入口（统一脚本 scripts/pack.sh）
# ═══════════════════════════════════════════════════════════════════════════

# 完整打包（前端→sidecar→Tauri→签名→DMG）
desktop-pack:
    bash scripts/pack.sh

# 快速打包（跳过前端构建）
desktop-pack-quick:
    bash scripts/pack.sh --quick

# 只构建 .app（不创 DMG）
desktop-pack-app:
    bash scripts/pack.sh --app-only

# 只创建 DMG（需先有 .app）
desktop-pack-dmg:
    bash scripts/pack.sh --dmg-only

# 只签名已有 .app
desktop-pack-sign:
    bash scripts/pack.sh --sign-only

# CI 模式打包
desktop-pack-ci:
    bash scripts/pack.sh --ci

# Apple 公证（需先完整打包 + 配置 ~/.crablet-notary.env）
desktop-notarize:
    bash desktop/notarize-dmg.sh

# 版本号一致性检查（CI 用，不修改文件）
desktop-version-check:
    bash scripts/sync-version.sh --check

# Dev mode: launch Tauri dev server with hot reload
desktop-dev:
    cd desktop && cargo tauri dev

# Clean desktop build artifacts
desktop-clean:
    rm -rf desktop/binaries desktop/gen target/release/bundle

# Quick check: verify desktop + sidecar compile without full build
desktop-check:
    cargo check --manifest-path ./crablet/Cargo.toml --no-default-features --features web
    cargo check --manifest-path ./desktop/Cargo.toml

# ═══════════════════════════════════════════════════════════════════════════
# General
# ═══════════════════════════════════════════════════════════════════════════

# Clean
clean:
    cargo clean --manifest-path ./crablet/Cargo.toml
    docker compose down -v
