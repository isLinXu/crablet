//! Testing Module - 测试模块
//!
//! 提供测试框架、模拟对象和测试工具

pub mod framework;
pub mod mocks;

pub use framework::*;

/// 初始化测试日志
pub fn init_test_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();
}

/// 测试辅助函数
pub mod helpers {
    use std::time::Duration;
    use tokio::time::timeout;

    /// 带超时的异步测试
    pub async fn with_timeout<F, T>(duration: Duration, f: F) -> Result<T, String>
    where
        F: std::future::Future<Output = T>,
    {
        timeout(duration, f)
            .await
            .map_err(|_| "Test timed out".to_string())
    }

    /// 默认超时 5 秒
    pub async fn with_default_timeout<F, T>(f: F) -> Result<T, String>
    where
        F: std::future::Future<Output = T>,
    {
        with_timeout(Duration::from_secs(5), f).await
    }
}

/// 测试数据生成器
pub mod fixtures {
    use crate::types::{Message, ContentPart};

    /// 创建测试消息
    pub fn create_test_message(content: &str, role: &str) -> Message {
        Message {
            role: role.to_string(),
            content: Some(vec![ContentPart::Text { text: content.to_string() }]),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// 创建用户消息
    pub fn user_message(content: &str) -> Message {
        create_test_message(content, "user")
    }

    /// 创建助手消息
    pub fn assistant_message(content: &str) -> Message {
        create_test_message(content, "assistant")
    }

    /// 创建系统消息
    pub fn system_message(content: &str) -> Message {
        create_test_message(content, "system")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_helpers() {
        let result = helpers::with_default_timeout(async {
            "success"
        }).await;
        
        assert_eq!(result.unwrap(), "success");
    }

    #[test]
    fn test_fixtures() {
        let msg = fixtures::user_message("Hello");
        assert_eq!(msg.role, "user");
        assert!(msg.content.is_some());
    }
}
