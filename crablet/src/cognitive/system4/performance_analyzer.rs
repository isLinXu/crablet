//! Performance Analyzer - 性能分析引擎
//!
//! 分析 System1-3 的执行性能，识别瓶颈和优化机会

use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// 系统类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CognitiveSystemType {
    System1,
    System2,
    System3,
}

impl CognitiveSystemType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CognitiveSystemType::System1 => "system1",
            CognitiveSystemType::System2 => "system2",
            CognitiveSystemType::System3 => "system3",
        }
    }
}

/// 执行指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub system_type: CognitiveSystemType,
    pub query: String,
    pub latency_ms: u64,
    pub success: bool,
    pub token_count: usize,
    pub tool_calls: usize,
    pub agent_count: usize, // For System3
    pub timestamp: DateTime<Utc>,
    pub context_length: usize,
    pub retry_count: u32,
}

/// 性能统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub error_rate: f64,
    pub throughput_per_minute: f64,
}

/// 瓶颈分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleneckAnalysis {
    pub system_type: CognitiveSystemType,
    pub bottleneck_type: BottleneckType,
    pub severity: Severity,
    pub description: String,
    pub affected_queries: Vec<String>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BottleneckType {
    Latency,
    ErrorRate,
    ResourceExhaustion,
    ContextLength,
    TokenUsage,
    ConcurrencyLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// 性能趋势
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTrend {
    pub system_type: CognitiveSystemType,
    pub metric: String,
    pub direction: TrendDirection,
    pub change_percent: f64,
    pub window_hours: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Stable,
    Degrading,
}

/// 性能报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub generated_at: DateTime<Utc>,
    pub window_hours: u32,
    pub overall_stats: HashMap<CognitiveSystemType, PerformanceStats>,
    pub bottlenecks: Vec<BottleneckAnalysis>,
    pub trends: Vec<PerformanceTrend>,
    pub insights: Vec<String>,
}

/// 性能分析引擎
pub struct PerformanceAnalyzer {
    metrics_history: Arc<RwLock<Vec<ExecutionMetrics>>>,
    max_history_size: usize,
}

impl PerformanceAnalyzer {
    pub fn new() -> Self {
        Self {
            metrics_history: Arc::new(RwLock::new(Vec::with_capacity(10000))),
            max_history_size: 100000,
        }
    }

    pub fn with_capacity(max_history_size: usize) -> Self {
        Self {
            metrics_history: Arc::new(RwLock::new(Vec::with_capacity(max_history_size.min(10000)))),
            max_history_size,
        }
    }

    /// 记录执行指标
    pub async fn record_metrics(&self, metrics: ExecutionMetrics) {
        let mut history = self.metrics_history.write().await;
        
        // 限制历史大小
        if history.len() >= self.max_history_size {
            // 移除最旧的 10%
            let remove_count = self.max_history_size / 10;
            history.drain(0..remove_count);
        }
        
        history.push(metrics);
        debug!("Recorded execution metrics, history size: {}", history.len());
    }

    /// 生成性能报告
    pub async fn generate_report(&self, window_hours: u32) -> PerformanceReport {
        let history = self.metrics_history.read().await;
        let cutoff_time = Utc::now() - chrono::Duration::hours(window_hours as i64);
        
        // 过滤时间窗口内的数据
        let recent_metrics: Vec<_> = history
            .iter()
            .filter(|m| m.timestamp >= cutoff_time)
            .cloned()
            .collect();

        let mut overall_stats = HashMap::new();
        let mut bottlenecks = Vec::new();
        let mut trends = Vec::new();

        // 分析每个系统
        for system_type in [CognitiveSystemType::System1, CognitiveSystemType::System2, CognitiveSystemType::System3] {
            let system_metrics: Vec<_> = recent_metrics
                .iter()
                .filter(|m| m.system_type == system_type)
                .collect();

            if system_metrics.is_empty() {
                continue;
            }

            let stats = self.calculate_stats(&system_metrics);
            overall_stats.insert(system_type, stats.clone());

            // 检测瓶颈
            if let Some(bottleneck) = self.detect_bottleneck(system_type, &stats, &system_metrics) {
                bottlenecks.push(bottleneck);
            }

            // 分析趋势
            if let Some(trend) = self.analyze_trend(system_type, &system_metrics, window_hours) {
                trends.push(trend);
            }
        }

        // 生成洞察
        let insights = self.generate_insights(&overall_stats, &bottlenecks, &trends);

        PerformanceReport {
            generated_at: Utc::now(),
            window_hours,
            overall_stats,
            bottlenecks,
            trends,
            insights,
        }
    }

    /// 计算统计指标
    fn calculate_stats(&self, metrics: &[&ExecutionMetrics]) -> PerformanceStats {
        let total = metrics.len() as u64;
        let successful = metrics.iter().filter(|m| m.success).count() as u64;
        let failed = total - successful;

        let latencies: Vec<u64> = metrics.iter().map(|m| m.latency_ms).collect();
        let avg_latency = if !latencies.is_empty() {
            latencies.iter().sum::<u64>() as f64 / latencies.len() as f64
        } else {
            0.0
        };

        let mut sorted_latencies = latencies.clone();
        sorted_latencies.sort_unstable();

        let p50 = self.percentile(&sorted_latencies, 0.5);
        let p95 = self.percentile(&sorted_latencies, 0.95);
        let p99 = self.percentile(&sorted_latencies, 0.99);

        let error_rate = if total > 0 {
            failed as f64 / total as f64
        } else {
            0.0
        };

        // 计算吞吐量（每分钟）
        let throughput = if metrics.len() >= 2 {
            let timestamps: Vec<_> = metrics.iter().map(|m| m.timestamp).collect();
            let first = timestamps.iter().min().copied().unwrap_or_else(Utc::now);
            let last = timestamps.iter().max().copied().unwrap_or_else(Utc::now);
            let time_span = (last - first).num_seconds() as f64 / 60.0;
            if time_span > 0.0 {
                total as f64 / time_span
            } else {
                total as f64
            }
        } else {
            total as f64
        };

        PerformanceStats {
            total_requests: total,
            successful_requests: successful,
            failed_requests: failed,
            avg_latency_ms: avg_latency,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            error_rate,
            throughput_per_minute: throughput,
        }
    }

    fn percentile(&self, sorted_data: &[u64], percentile: f64) -> f64 {
        if sorted_data.is_empty() {
            return 0.0;
        }
        let index = (percentile * (sorted_data.len() - 1) as f64).round() as usize;
        sorted_data[index.min(sorted_data.len() - 1)] as f64
    }

    /// 检测瓶颈
    fn detect_bottleneck(
        &self,
        system_type: CognitiveSystemType,
        stats: &PerformanceStats,
        metrics: &[&ExecutionMetrics],
    ) -> Option<BottleneckAnalysis> {
        // 检查错误率
        if stats.error_rate > 0.1 {
            return Some(BottleneckAnalysis {
                system_type,
                bottleneck_type: BottleneckType::ErrorRate,
                severity: if stats.error_rate > 0.3 {
                    Severity::Critical
                } else {
                    Severity::High
                },
                description: format!("Error rate is {:.1}%", stats.error_rate * 100.0),
                affected_queries: self.get_affected_queries(metrics),
                recommendation: "Consider increasing retry limits or switching to more reliable system".to_string(),
            });
        }

        // 检查延迟
        if stats.p95_latency_ms > 5000.0 {
            return Some(BottleneckAnalysis {
                system_type,
                bottleneck_type: BottleneckType::Latency,
                severity: if stats.p95_latency_ms > 10000.0 {
                    Severity::Critical
                } else {
                    Severity::High
                },
                description: format!("P95 latency is {:.0}ms", stats.p95_latency_ms),
                affected_queries: self.get_affected_queries(metrics),
                recommendation: "Consider using faster system or optimizing context length".to_string(),
            });
        }

        // 检查上下文长度问题
        let long_context_count = metrics.iter().filter(|m| m.context_length > 8000).count();
        if long_context_count > metrics.len() / 3 {
            return Some(BottleneckAnalysis {
                system_type,
                bottleneck_type: BottleneckType::ContextLength,
                severity: Severity::Medium,
                description: format!("{}% of queries have long context", long_context_count * 100 / metrics.len()),
                affected_queries: self.get_affected_queries(metrics),
                recommendation: "Consider implementing context compression or summarization".to_string(),
            });
        }

        None
    }

    /// 分析趋势
    fn analyze_trend(
        &self,
        system_type: CognitiveSystemType,
        metrics: &[&ExecutionMetrics],
        window_hours: u32,
    ) -> Option<PerformanceTrend> {
        if metrics.len() < 10 {
            return None;
        }

        // 分割为两半比较
        let mid = metrics.len() / 2;
        let first_half: Vec<u64> = metrics[..mid].iter().map(|m| m.latency_ms).collect();
        let second_half: Vec<u64> = metrics[mid..].iter().map(|m| m.latency_ms).collect();

        let first_avg = first_half.iter().sum::<u64>() as f64 / first_half.len() as f64;
        let second_avg = second_half.iter().sum::<u64>() as f64 / second_half.len() as f64;

        if first_avg == 0.0 {
            return None;
        }

        let change_percent = ((second_avg - first_avg) / first_avg) * 100.0;

        let direction = if change_percent < -10.0 {
            TrendDirection::Improving
        } else if change_percent > 10.0 {
            TrendDirection::Degrading
        } else {
            TrendDirection::Stable
        };

        Some(PerformanceTrend {
            system_type,
            metric: "latency".to_string(),
            direction,
            change_percent,
            window_hours,
        })
    }

    /// 生成洞察
    fn generate_insights(
        &self,
        stats: &HashMap<CognitiveSystemType, PerformanceStats>,
        bottlenecks: &[BottleneckAnalysis],
        trends: &[PerformanceTrend],
    ) -> Vec<String> {
        let mut insights = Vec::new();

        // 比较系统性能
        if let (Some(s1), Some(s2)) = (stats.get(&CognitiveSystemType::System1), stats.get(&CognitiveSystemType::System2)) {
            if s1.avg_latency_ms < s2.avg_latency_ms / 10.0 {
                insights.push(format!(
                    "System1 is {:.1}x faster than System2 on average",
                    s2.avg_latency_ms / s1.avg_latency_ms
                ));
            }
        }

        // 瓶颈洞察
        for bottleneck in bottlenecks {
            insights.push(format!(
                "{:?} has {:?} bottleneck: {}",
                bottleneck.system_type, bottleneck.bottleneck_type, bottleneck.description
            ));
        }

        // 趋势洞察
        for trend in trends {
            match trend.direction {
                TrendDirection::Improving => {
                    insights.push(format!(
                        "{:?} performance is improving ({:.1}% better)",
                        trend.system_type, trend.change_percent.abs()
                    ));
                }
                TrendDirection::Degrading => {
                    insights.push(format!(
                        "{:?} performance is degrading ({:.1}% worse)",
                        trend.system_type, trend.change_percent.abs()
                    ));
                }
                _ => {}
            }
        }

        insights
    }

    fn get_affected_queries(&self, metrics: &[&ExecutionMetrics]) -> Vec<String> {
        metrics
            .iter()
            .take(5)
            .map(|m| m.query.chars().take(50).collect::<String>() + "...")
            .collect()
    }

    /// 获取历史数据（用于其他分析器）
    pub async fn get_history(&self, limit: usize) -> Vec<ExecutionMetrics> {
        let history = self.metrics_history.read().await;
        history.iter().rev().take(limit).cloned().collect()
    }

    /// 清除历史数据
    pub async fn clear_history(&self) {
        let mut history = self.metrics_history.write().await;
        history.clear();
        info!("Performance metrics history cleared");
    }
}

impl Default for PerformanceAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_and_report() {
        let analyzer = PerformanceAnalyzer::new();

        // 记录一些测试指标
        for i in 0..10 {
            analyzer.record_metrics(ExecutionMetrics {
                system_type: CognitiveSystemType::System2,
                query: format!("test query {}", i),
                latency_ms: 1000 + (i * 100) as u64,
                success: i < 8, // 2 failures
                token_count: 100,
                tool_calls: 2,
                agent_count: 1,
                timestamp: Utc::now(),
                context_length: 500,
                retry_count: 0,
            }).await;
        }

        let report = analyzer.generate_report(24).await;
        assert!(!report.overall_stats.is_empty());
        
        let s2_stats = report.overall_stats.get(&CognitiveSystemType::System2).unwrap();
        assert_eq!(s2_stats.total_requests, 10);
        assert_eq!(s2_stats.failed_requests, 2);
        assert!(s2_stats.error_rate > 0.19 && s2_stats.error_rate < 0.21);
    }

    #[test]
    fn test_percentile() {
        let analyzer = PerformanceAnalyzer::new();
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        
        assert_eq!(analyzer.percentile(&data, 0.5), 6.0);
        assert_eq!(analyzer.percentile(&data, 0.95), 10.0);
    }
}
