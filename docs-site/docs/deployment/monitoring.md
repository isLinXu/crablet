---
title: Monitoring
description: OpenTelemetry, Prometheus, Grafana, and Jaeger integration
---

# :chart_line: Monitoring

Crablet integrates with industry-standard observability tools for full-stack monitoring.

## OpenTelemetry Integration

```toml
[telemetry]
enabled = true
endpoint = "http://tempo:4317"
service_name = "crablet"
sample_rate = 1.0  # 1.0 = 100%, reduce in production
```

## Available Metrics

| Metric | Type | Description |
|:-------|:-----|:------------|
| `crablet.request.duration` | Histogram | Request latency distribution |
| `crablet.llm.tokens` | Counter | Token consumption per model |
| `crablet.llm.latency` | Histogram | LLM API call latency |
| `crablet.tool.calls` | Counter | Tool invocations by type |
| `crablet.tool.errors` | Counter | Tool execution failures |
| `crablet.memory.operations` | Counter | Memory read/write operations |
| `crablet.sessions.active` | Gauge | Current active sessions |
| `crablet.agents.spawned` | Counter | Sub-agents spawned |

## Prometheus Integration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'crablet'
    static_configs:
      - targets: ['crablet:18790']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

## Grafana Dashboard

Import the pre-built Crablet dashboard for:

- Request throughput and latency percentiles
- LLM token consumption and cost tracking
- Tool usage heatmaps
- Memory layer utilization
- Active sessions and agent spawns

## Jaeger Distributed Tracing

Trace individual requests through the full stack:

```
User Message → Channel → Cognitive Router → System 2 → LLM → Tool → Response
     1ms          2ms          5ms          850ms     120ms   50ms
```

## Alerting Rules

```yaml
# alerts.yaml
groups:
  - name: crablet
    rules:
      - alert: HighErrorRate
        expr: rate(crablet_tool_errors[5m]) > 0.1
        for: 5m
        labels:
          severity: warning
        
      - alert: LLMCostSpike
        expr: increase(crablet_llm_tokens[1h]) > 1000000
        for: 1h
        labels:
          severity: warning
```
