---
title: Docker
description: Containerized deployment with Docker and Docker Compose
---

# :whale: Docker Deployment

## Quick Start

```bash
docker run -d \
  --name crablet \
  -p 18790:18790 \
  -e OPENAI_API_KEY=sk-xxx \
  -v ./data:/data \
  -v ./skills:/skills \
  crablet:latest
```

## Docker Compose

```yaml
version: '3.8'

services:
  crablet:
    image: crablet:latest
    ports:
      - "18790:18790"      # Web UI
      - "18789:18789"      # Gateway
    environment:
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - DATABASE_URL=sqlite:///data/crablet.db
      - RUST_LOG=info
    volumes:
      - ./data:/data
      - ./skills:/skills
      - ./config:/root/.config/crablet
    depends_on:
      - neo4j
    restart: unless-stopped

  neo4j:
    image: neo4j:5
    ports:
      - "7474:7474"
      - "7687:7687"
    environment:
      - NEO4J_AUTH=neo4j/password
    volumes:
      - neo4j_data:/data

volumes:
  neo4j_data:
```

## Building Custom Images

```dockerfile
FROM rust:1.82-alpine AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --no-default-features --features web,knowledge,telemetry

FROM alpine:3.19
COPY --from=builder /app/target/release/crablet /usr/local/bin/
COPY config/ /etc/crablet/
EXPOSE 18790
ENTRYPOINT ["crablet", "serve-web", "--port", "18790"]
```

## Image Sizes

| Variant | Size | Startup |
|:--------|:-----|:--------|
| Full (all features) | ~18 MB | ~480ms |
| Minimal (web only) | ~8 MB | ~200ms |
| Alpine-based | ~12 MB | ~350ms |
