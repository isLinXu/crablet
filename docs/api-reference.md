# API 参考

Crablet 支持通过**统一 Channel trait** 接入多个平台。

## CLI 命令行接口

- `crablet chat`: 交互式聊天
- `crablet run <command>`: 单次执行
- `crablet serve-web --port <port>`: 启动 Web 服务
- `crablet gateway --port <port>`: 启动网关服务
- `crablet skill <subcommand>`: 技能管理
- `crablet knowledge <subcommand>`: 知识管理

## Web Gateway

Crablet 提供一个基于 Axum 的网关服务，支持 HTTP API、WebSocket 和 JSON-RPC。

### WebSocket 协议

连接地址: `ws://localhost:18789/ws`

消息格式 (JSON):
```json
{
  "type": "UserInput",
  "content": "Hello"
}
```

响应类型:
- `ThoughtGenerated`: ReAct 思考过程
- `ToolExecutionStarted`: 工具调用开始
- `ToolExecutionFinished`: 工具调用结束
- `ResponseGenerated`: 最终回复
- `SwarmActivity`: 群体协作事件

### HTTP API

- `POST /api/chat`: 发送聊天消息
- `GET /api/status`: 获取系统状态

(更多 API 详情请参考代码或使用 `utoipa` 生成的 OpenAPI 文档)

## 接入通道

| 平台 | 状态 | 协议 |
|:---|:---:|:---|
| **CLI** | ✅ | stdin/stdout |
| **Web UI** | ✅ | HTTP + WebSocket |
| **Telegram** | ✅ | Telegram Bot API |
| **Discord** | ✅ | Discord Gateway |
| **飞书** | 🚧 | 飞书开放平台 |
| **钉钉** | 🚧 | 钉钉开放平台 |
| **HTTP Webhook** | ✅ | HTTP POST |
