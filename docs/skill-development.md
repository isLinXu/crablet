# 技能开发指南

Crablet 支持多种技能格式，均可通过 CLI 安装和管理。

## 技能类型

1. **可执行技能** (`skill.yaml`): 包含独立运行时的可执行程序 (Python/Node.js/Shell)
2. **OpenClaw 指令型技能** (`SKILL.md`): 纯 Prompt 指令
3. **MCP 工具**: 通过 Model Context Protocol 接入的外部工具
4. **原生 Rust 插件**: 编译进二进制的 Rust 代码

## 可执行技能

### skill.yaml 格式

```yaml
name: weather
description: Get current weather for a city using OpenMeteo API
version: 1.0.0
parameters:
  type: object
  properties:
    city:
      type: string
      description: The city to get weather for
  required: [city]
entrypoint: python3 weather.py
timeout: 10
env:
  API_KEY: ${OPENMETEO_API_KEY}
```

### Python 实现示例

```python
import sys
import json
import requests

def main():
    args = json.loads(sys.argv[1])
    city = args["city"]
    
    response = requests.get(f"https://api.open-meteo.com/v1/forecast?city={city}")
    print(json.dumps(response.json()))

if __name__ == "__main__":
    main()
```

## OpenClaw 技能

### SKILL.md 格式

```markdown
---
name: python-expert
description: Expert Python coding assistant
version: 1.0.0
---

You are a Python expert. Always use type hints and docstrings.
When writing code, follow PEP 8 conventions.
```

## 内置工具

Crablet 提供了丰富的内置工具：

- `bash`: Shell 命令执行 (受 SafetyOracle 保护)
- `file`: 文件读写 (受路径检查保护)
- `web_search`: 网络搜索 (Serper / DuckDuckGo)
- `http`: HTTP 请求
- `vision`: 图像分析
- `browser`: 无头浏览器
- `calculator`: 数学计算
- `weather`: 天气查询

## MCP 协议支持

Crablet 完整支持 [Model Context Protocol](https://modelcontextprotocol.io)。

配置 MCP 服务器：
```toml
[mcp_servers]
math_server = { command = "python3", args = ["mcp_server.py"] }
```
