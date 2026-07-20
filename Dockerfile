# syntax=docker/dockerfile:1

# ─── Stage 0: Frontend Builder ───────────────────────────────────────────────
FROM node:26-alpine AS frontend-builder
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN if [ -f package-lock.json ]; then npm ci; else npm install; fi
COPY frontend/ .
RUN npm run build

# ─── Stage 1: Cargo-Chef base ────────────────────────────────────────────────
# Pin to a specific rust version for reproducible builds.
FROM lukemathwalker/cargo-chef:latest-rust-1.87 AS chef
WORKDIR /app

# ─── Stage 2: Planner ────────────────────────────────────────────────────────
FROM chef AS planner
# Copy workspace manifest + lock first (cheapest layer)
COPY Cargo.toml Cargo.lock ./
# crablet crate manifests only — desktop is NOT built in container
COPY crablet/Cargo.toml ./crablet/
# Create a minimal stub for the desktop workspace member so Cargo resolves the
# workspace without error, but we never compile it here.
RUN mkdir -p desktop/src && \
    printf '[package]\nname = "desktop"\nversion = "0.0.0"\nedition = "2021"\n' > desktop/Cargo.toml && \
    echo 'fn main() {}' > desktop/src/main.rs
COPY crablet/src/ ./crablet/src/
WORKDIR /app/crablet
RUN cargo chef prepare --recipe-path recipe.json

# ─── Stage 3: Builder ────────────────────────────────────────────────────────
FROM chef AS builder
COPY Cargo.toml Cargo.lock /app/
COPY crablet/Cargo.toml /app/crablet/
# Recreate desktop stub (same as planner — keeps workspace consistent)
RUN mkdir -p /app/desktop/src && \
    printf '[package]\nname = "desktop"\nversion = "0.0.0"\nedition = "2021"\n' > /app/desktop/Cargo.toml && \
    echo 'fn main() {}' > /app/desktop/src/main.rs
COPY --from=planner /app/crablet/recipe.json /app/crablet/recipe.json
WORKDIR /app/crablet
# Warm dependency cache — must match the final build flags exactly
RUN cargo chef cook --release --no-default-features --features web --recipe-path recipe.json

# Build the real binary
COPY crablet/ /app/crablet/
WORKDIR /app
RUN cargo build --release --no-default-features --features web -p crablet

# ─── Stage 4: Runtime ────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Create non-root user
RUN groupadd -r crablet && useradd -r -g crablet crablet

# Install runtime dependencies.
# Note: backend uses rustls → no libssl3 needed.
# ffmpeg is kept for audio feature; remove if not needed to shrink image.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    ffmpeg \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/crablet /usr/local/bin/

# Copy frontend build to static directory
COPY --from=frontend-builder /app/frontend/dist /app/frontend/dist

# Copy default skills
# COPY skills/ /app/skills/
COPY crablet/skills/ /app/skills/

# Create directory structure and set permissions
# skills, data, config, uploads
RUN mkdir -p /app/data /app/config /app/uploads && \
    chown -R crablet:crablet /app

# Switch to user
USER crablet

# Set environment variables
ENV RUST_LOG=info
ENV XDG_CONFIG_HOME=/app/config
ENV XDG_DATA_HOME=/app/data
ENV CRABLET_HOST=0.0.0.0

# Expose ports
EXPOSE 3000

# Healthcheck
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:3000/ || exit 1

# Default command
CMD ["crablet", "serve-web", "--port", "3000"]
