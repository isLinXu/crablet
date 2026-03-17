# Crablet 渠道集成指南

本指南介绍如何为 Crablet 添加新的消息渠道（如企业微信、Slack 等）。

## 架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                      Channel Trait                          │
├─────────────────────────────────────────────────────────────┤
│  + connect() -> Result<Connection, Error>                   │
│  + disconnect() -> Result<(), Error>                        │
│  + send_message(to, content) -> Result<(), Error>           │
│  + on_message(callback) -> Result<(), Error>                │
│  + get_channel_type() -> ChannelType                        │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│  WeCom        │    │  Slack        │    │  WhatsApp     │
│  (企业微信)    │    │               │    │               │
└───────────────┘    └───────────────┘    └───────────────┘
```

## 实现步骤

### 1. 创建渠道模块

在 `crablet/src/channels/` 下创建新的渠道目录：

```bash
mkdir -p crablet/src/channels/enterprise/wecom
```

### 2. 实现 Channel Trait

```rust
// crablet/src/channels/enterprise/wecom/mod.rs

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::channels::{Channel, ChannelConfig, ChannelType, Message};
use crate::events::EventBus;

pub struct WeComChannel {
    config: WeComConfig,
    event_bus: EventBus,
    message_tx: Option<mpsc::Sender<Message>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeComConfig {
    pub corp_id: String,
    pub corp_secret: String,
    pub agent_id: String,
    pub webhook_url: Option<String>,
    pub encrypt_token: Option<String>,
    pub encoding_aes_key: Option<String>,
}

#[async_trait]
impl Channel for WeComChannel {
    type Config = WeComConfig;
    
    fn new(config: Self::Config, event_bus: EventBus) -> Self {
        Self {
            config,
            event_bus,
            message_tx: None,
        }
    }
    
    async fn connect(&mut self) -> Result<(), ChannelError> {
        // 1. 获取 access_token
        let token = self.get_access_token().await?;
        
        // 2. 启动消息接收服务（Webhook 或长轮询）
        self.start_message_server(token).await?;
        
        tracing::info!("WeCom channel connected: corp_id={}", self.config.corp_id);
        Ok(())
    }
    
    async fn disconnect(&mut self) -> Result<(), ChannelError> {
        // 清理资源
        if let Some(tx) = self.message_tx.take() {
            drop(tx);
        }
        tracing::info!("WeCom channel disconnected");
        Ok(())
    }
    
    async fn send_message(&self, to: &str, content: &str) -> Result<(), ChannelError> {
        let token = self.get_access_token().await?;
        
        let message = WeComMessage {
            touser: to.to_string(),
            msgtype: "text".to_string(),
            agentid: self.config.agent_id.clone(),
            text: TextContent {
                content: content.to_string(),
            },
            safe: 0,
        };
        
        let response = reqwest::Client::new()
            .post(format!(
                "https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={}",
                token
            ))
            .json(&message)
            .send()
            .await?;
            
        if response.status().is_success() {
            let result: WeComResponse = response.json().await?;
            if result.errcode == 0 {
                return Ok(());
            } else {
                return Err(ChannelError::ApiError(result.errmsg));
            }
        }
        
        Err(ChannelError::HttpError(response.status()))
    }
    
    async fn on_message<F>(&self, callback: F) -> Result<(), ChannelError>
    where
        F: Fn(Message) + Send + Sync + 'static,
    {
        // 设置消息处理回调
        // 实际实现中，这会在收到 Webhook 或长轮询消息时触发
        Ok(())
    }
    
    fn get_channel_type(&self) -> ChannelType {
        ChannelType::WeCom
    }
}

impl WeComChannel {
    async fn get_access_token(&self) -> Result<String, ChannelError> {
        let url = format!(
            "https://qyapi.weixin.qq.com/cgi-bin/gettoken?corpid={}&corpsecret={}",
            self.config.corp_id, self.config.corp_secret
        );
        
        let response = reqwest::get(&url).await?;
        let result: AccessTokenResponse = response.json().await?;
        
        if result.errcode == 0 {
            Ok(result.access_token)
        } else {
            Err(ChannelError::AuthError(result.errmsg))
        }
    }
    
    async fn start_message_server(&mut self, token: String) -> Result<(), ChannelError> {
        // 启动 HTTP 服务器接收企业微信的 Webhook 推送
        // 或使用长轮询方式接收消息
        
        let (tx, mut rx) = mpsc::channel::<Message>(100);
        self.message_tx = Some(tx);
        
        let event_bus = self.event_bus.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                event_bus.publish(message).await;
            }
        });
        
        Ok(())
    }
}

// 企业微信 API 数据结构
#[derive(Serialize)]
struct WeComMessage {
    touser: String,
    msgtype: String,
    agentid: String,
    text: TextContent,
    safe: i32,
}

#[derive(Serialize)]
struct TextContent {
    content: String,
}

#[derive(Deserialize)]
struct WeComResponse {
    errcode: i32,
    errmsg: String,
}

#[derive(Deserialize)]
struct AccessTokenResponse {
    errcode: i32,
    errmsg: String,
    access_token: String,
    expires_in: i32,
}
```

### 3. Webhook 处理器

```rust
// crablet/src/channels/enterprise/wecom/webhook.rs

use axum::{
    extract::{Extension, Json},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Router,
};
use serde::Deserialize;

use crate::channels::enterprise::wecom::WeComChannel;
use crate::events::EventBus;

#[derive(Deserialize)]
struct WeComWebhookPayload {
    tousername: String,
    fromusername: String,
    createtime: i64,
    msgtype: String,
    content: Option<String>,
    msgid: String,
}

pub fn wecom_webhook_routes() -> Router {
    Router::new()
        .route("/webhook/wecom", post(wecom_webhook_handler))
        .route("/webhook/wecom/verify", post(wecom_verify_handler))
}

async fn wecom_webhook_handler(
    Extension(event_bus): Extension<EventBus>,
    Json(payload): Json<WeComWebhookPayload>,
) -> impl IntoResponse {
    // 解密消息（如果启用了加密）
    // 验证消息签名
    
    let message = Message {
        channel_type: ChannelType::WeCom,
        from: payload.fromusername,
        content: payload.content.unwrap_or_default(),
        timestamp: payload.createtime,
        message_id: payload.msgid,
    };
    
    // 发布到事件总线
    event_bus.publish(message).await;
    
    // 返回成功响应
    StatusCode::OK
}

async fn wecom_verify_handler(
    Query(params): Query<VerifyParams>,
) -> impl IntoResponse {
    // 处理企业微信的 URL 验证请求
    // 返回 echostr 以完成验证
    params.echostr
}

#[derive(Deserialize)]
struct VerifyParams {
    msg_signature: String,
    timestamp: String,
    nonce: String,
    echostr: String,
}
```

### 4. 配置文件

```toml
# config.toml

[channels.wecom]
enabled = true
corp_id = "your-corp-id"
corp_secret = "your-corp-secret"
agent_id = "your-agent-id"
webhook_url = "https://your-domain.com/webhook/wecom"
encrypt_token = "your-token"  # 可选，用于消息加密
encoding_aes_key = "your-aes-key"  # 可选，用于消息加密
```

### 5. 注册渠道

```rust
// crablet/src/channels/mod.rs

pub mod enterprise {
    pub mod wecom;
}

pub enum ChannelType {
    Cli,
    Telegram,
    Discord,
    DingTalk,
    Feishu,
    WeCom,  // 新增
    Slack,  // 新增
    Webhook,
}

pub struct ChannelManager {
    channels: HashMap<ChannelType, Box<dyn Channel>>,
}

impl ChannelManager {
    pub async fn init_channels(&mut self, config: &Config, event_bus: EventBus) -> Result<(), Error> {
        // 初始化企业微信渠道
        if config.channels.wecom.enabled {
            let wecom = WeComChannel::new(config.channels.wecom.clone(), event_bus.clone());
            wecom.connect().await?;
            self.channels.insert(ChannelType::WeCom, Box::new(wecom));
        }
        
        // 初始化其他渠道...
        
        Ok(())
    }
}
```

## 测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_wecom_send_message() {
        let config = WeComConfig {
            corp_id: "test-corp-id".to_string(),
            corp_secret: "test-secret".to_string(),
            agent_id: "test-agent-id".to_string(),
            webhook_url: None,
            encrypt_token: None,
            encoding_aes_key: None,
        };
        
        let event_bus = EventBus::new();
        let mut channel = WeComChannel::new(config, event_bus);
        
        // 使用 mock 服务器测试
        // ...
    }
}
```

## 最佳实践

### 1. 错误处理
- 所有网络操作都要有超时设置
- 实现指数退避重试机制
- 详细记录错误日志

### 2. 性能优化
- 使用连接池复用 HTTP 连接
- 批量发送消息减少 API 调用
- 异步处理消息接收

### 3. 安全性
- 验证 Webhook 签名
- 敏感信息加密存储
- 使用 HTTPS 传输

### 4. 可观测性
- 记录渠道指标（发送/接收消息数）
- 监控渠道健康状态
- 设置告警阈值

## 参考实现

查看现有渠道实现：
- Telegram: `crablet/src/channels/international/telegram.rs`
- DingTalk: `crablet/src/channels/domestic/dingtalk.rs`
- Feishu: `crablet/src/channels/domestic/feishu.rs`

## 常见问题

### Q: 如何处理消息加解密？
A: 使用企业微信提供的加密库，在 Webhook 处理器中解密消息。

### Q: 如何支持富媒体消息？
A: 实现对应的消息类型（图片、语音、视频等），参考企业微信 API 文档。

### Q: 如何处理并发消息？
A: 使用 Tokio 的 mpsc 通道，在独立任务中处理消息。

---

*最后更新：2026年3月*
