# Justfile for Crablet

# Minimal build
build-minimal:
    cargo build --no-default-features --features web

# Full build
build-full:
    cargo build --release

# Run tests
test:
    cargo test --release --no-default-features --features web

# Run all tests
test-all:
    cargo test --release --all-features

# Lint
lint:
    cargo fmt --check
    cargo clippy -- -D warnings

# Audit
audit:
    cargo audit
    cargo deny check

# Coverage
coverage:
    cargo llvm-cov --html --open

# Dev Server
dev:
    cargo run --no-default-features --features web -- serve-web --port 3000

# Docker Build
docker-build:
    docker build -t crablet:latest .

# Up
up:
    docker-compose up -d

# Clean
clean:
    cargo clean
    docker-compose down -v
