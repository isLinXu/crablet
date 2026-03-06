# 部署指南

## Docker 部署

Crablet 提供了 Docker 镜像，支持一键部署。

### 运行容器

```bash
docker run -d \
  --name crablet \
  -p 3000:3000 \
  -e OPENAI_API_KEY=sk-xxx \
  -v ./data:/data \
  -v ./skills:/skills \
  crablet:latest
```

### Docker Compose

```yaml
version: '3.8'

services:
  crablet:
    image: crablet:latest
    ports:
      - "3000:3000"      # Web UI
      - "18789:18789"    # Gateway
    environment:
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - DATABASE_URL=sqlite:///data/crablet.db
    volumes:
      - ./data:/data
      - ./skills:/skills
    depends_on:
      - neo4j
  
  neo4j:
    image: neo4j:5
    ports:
      - "7474:7474"      # Web UI
      - "7687:7687"      # Bolt
    environment:
      - NEO4J_AUTH=neo4j/password
    volumes:
      - neo4j_data:/data

volumes:
  neo4j_data:
```

## 生产环境配置

在生产环境中，建议使用 PostgreSQL 作为数据库，并开启 Strict 安全模式。

```toml
# config.toml (生产环境)
database_url = "postgresql://user:pass@localhost/crablet"
log_level = "warn"

[safety]
level = "Strict"

[telemetry]
enabled = true
endpoint = "http://tempo:4317"

[limits]
max_concurrent_requests = 100
request_timeout = 30
```

## 监控

Crablet 支持 OpenTelemetry，可集成到 Grafana/Prometheus/Jaeger 监控体系中。

- **Jaeger**: 分布式追踪
- **Prometheus**: 指标监控 (`crablet.request.duration`, `crablet.llm.tokens` 等)
