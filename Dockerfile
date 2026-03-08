# syntax=docker/dockerfile:1

# Stage: Frontend Builder
FROM node:20-alpine AS frontend-builder
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ .
RUN npm run build

# Stage 1: Chef (Pre-computation)
FROM lukemathwalker/cargo-chef:latest-rust-1.88 AS chef
WORKDIR /app

# Stage 2: Planner
FROM chef AS planner
COPY crablet/Cargo.toml crablet/Cargo.lock ./
COPY crablet/src/ ./src/
# Only copy necessary files for recipe generation
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching layer!
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY crablet/ ./
RUN cargo build --release

# Stage 4: Runtime
FROM debian:bookworm-slim AS runtime

# Create non-root user
RUN groupadd -r crablet && useradd -r -g crablet crablet

# Install runtime dependencies
# ffmpeg for audio, ca-certificates for https, curl for healthcheck
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    ffmpeg \
    curl \
    python3 \
    python3-pip \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/crablet /usr/local/bin/

# Copy frontend build to the path expected by ServeDir("frontend/dist")
COPY --from=frontend-builder /app/frontend/dist /app/frontend/dist

# Copy default skills
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
  CMD curl -f http://localhost:3000/health || exit 1

# Default command - Start both gateway and web server
CMD ["/bin/sh", "-lc", "crablet gateway --host 0.0.0.0 --port 18789 & exec crablet serve-web --port 3000"]
