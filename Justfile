# Justfile for Crablet

# Minimal build
build-minimal:
    cargo build --manifest-path ./crablet/Cargo.toml --no-default-features --features web

# Full build
build-full:
    cargo build --manifest-path ./crablet/Cargo.toml --release

# Run tests
test:
    cargo test --manifest-path ./crablet/Cargo.toml --release --no-default-features --features web

# Run all tests
test-all:
    cargo test --manifest-path ./crablet/Cargo.toml --release --all-features

# Backend web suite used by CI
test-web:
    cargo test --manifest-path ./crablet/Cargo.toml --no-default-features --features web

# Backend doctests used by CI
test-doc:
    cargo test --manifest-path ./crablet/Cargo.toml --doc --no-default-features --features web

# Fast all-features compile validation
check-all-features:
    cargo check --manifest-path ./crablet/Cargo.toml --all-features

# Lint
lint:
    cargo fmt --manifest-path ./crablet/Cargo.toml --all -- --check
    cargo clippy --manifest-path ./crablet/Cargo.toml --all-targets --no-default-features --features web -- -D warnings

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
    docker-compose up -d

# Clean
clean:
    cargo clean --manifest-path ./crablet/Cargo.toml
    docker-compose down -v
