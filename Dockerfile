# syntax=docker/dockerfile:1

# Stage 1: Builder
FROM rust:1.80-slim-bookworm as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    git \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first to cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY crablet/Cargo.toml crablet/

# Create dummy source to build dependencies
RUN mkdir -p crablet/src && \
    echo "fn main() {println!(\"dummy\")}" > crablet/src/main.rs && \
    cargo build --release --package crablet

# Remove dummy source
RUN rm -rf crablet/src

# Copy actual source code
COPY . .

# Build the actual application
# Need to touch main.rs to force rebuild of the binary (dependencies are cached)
RUN touch crablet/src/main.rs && \
    cargo build --release --package crablet

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
# - ca-certificates: HTTPS
# - openssl: TLS
# - ffmpeg: Audio processing (Whisper/TTS)
# - sqlite3: DB debugging
# - curl: Healthchecks
RUN apt-get update && apt-get install -y \
    ca-certificates \
    openssl \
    ffmpeg \
    sqlite3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -ms /bin/bash crablet

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/crablet /usr/local/bin/crablet

# Create directory structure
RUN mkdir -p /app/skills /app/data /app/config /app/assets /app/templates
RUN chown -R crablet:crablet /app

# Copy static assets and templates if they exist in source
COPY --from=builder /app/crablet/templates /app/templates
# COPY --from=builder /app/crablet/assets /app/assets

# Switch to user
# Note: For Docker socket access, we might need to run as root or add user to docker group dynamically.
# For simplicity in this template, we default to root to ensure docker socket access works out of the box,
# but in production, one should use group mapping.
# USER crablet 

# Set environment variables
ENV RUST_LOG=info
ENV XDG_CONFIG_HOME=/app/config
ENV XDG_DATA_HOME=/app/data
ENV CRABLET_HOST=0.0.0.0

# Expose ports
# 3000: Web UI
# 18789: Gateway RPC
EXPOSE 3000 18789

# Healthcheck
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:18789/ || exit 1

# Default command
CMD ["crablet", "serve-web", "--port", "3000"]
