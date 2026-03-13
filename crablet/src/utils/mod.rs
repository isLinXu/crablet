//! Utils Module - 工具模块
//!
//! 提供通用的工具函数和数据结构

pub mod lock_free;
pub mod cache;

use std::time::{SystemTime, UNIX_EPOCH};

/// 获取当前时间戳（毫秒）
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// 获取当前时间戳（秒）
pub fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// 字符串截断
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// 计算字符串的近似token数（简化版）
pub fn estimate_tokens(text: &str) -> usize {
    // 粗略估计：英文约4字符/token，中文约1.5字符/token
    let char_count = text.chars().count();
    let chinese_chars = text.chars().filter(|c| (*c as u32) > 0x4E00 && (*c as u32) < 0x9FFF).count();
    let english_chars = char_count - chinese_chars;
    
    (english_chars / 4 + chinese_chars * 2 / 3).max(1)
}

/// 安全的除法
pub fn safe_divide<T: Into<f64>>(numerator: T, denominator: T) -> f64 {
    let n: f64 = numerator.into();
    let d: f64 = denominator.into();
    if d == 0.0 {
        0.0
    } else {
        n / d
    }
}

/// 计算百分位数
pub fn percentile(sorted_data: &[f64], p: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }
    
    let index = (p / 100.0) * (sorted_data.len() - 1) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;
    
    if lower == upper {
        sorted_data[lower]
    } else {
        let weight = index - lower as f64;
        sorted_data[lower] * (1.0 - weight) + sorted_data[upper] * weight
    }
}

/// 生成唯一ID
pub fn generate_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 生成短ID（8字符）
pub fn generate_short_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let uuid = uuid::Uuid::new_v4().to_string();
    let mut hasher = DefaultHasher::new();
    uuid.hash(&mut hasher);
    format!("{:08x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 5), "hello...");
    }

    #[test]
    fn test_estimate_tokens() {
        let english = "Hello world, this is a test.";
        let chinese = "你好世界，这是一个测试。";
        
        assert!(estimate_tokens(english) > 0);
        assert!(estimate_tokens(chinese) > 0);
    }

    #[test]
    fn test_safe_divide() {
        assert_eq!(safe_divide(10.0, 2.0), 5.0);
        assert_eq!(safe_divide(10.0, 0.0), 0.0);
    }

    #[test]
    fn test_percentile() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(&data, 0.0), 1.0);
        assert_eq!(percentile(&data, 50.0), 3.0);
        assert_eq!(percentile(&data, 100.0), 5.0);
    }
}
