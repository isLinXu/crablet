//! Self-Improvement Loop - Adaptive optimization system
//!
//! This module implements continuous self-optimization:
//! - Performance monitoring and analysis
//! - Strategy adaptation based on outcomes
//! - Parameter auto-tuning
//! - Feedback loop integration
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                 Self-Improvement Loop                               │
//! │                                                                      │
//! │   Metrics ──→  Analyze  ──→  Identify Issues  ──→  Adapt           │
//! │                   │                                              │
//! │                   ▼                                              │
//! │   ┌────────────────────────────────────────────────────────────┐   │
//! │   │                    Adaptation Strategies                   │   │
//! │   │  • Parameter Tuning (adjust thresholds)                    │   │
//! │   │  • Strategy Selection (choose best approach)               │   │
//! │   │  • Resource Allocation (optimize usage)                    │   │
//! │   │  • Model Selection (choose best models)                    │   │
//! │   │  • Feedback Integration (learn from outcomes)              │   │
//! │   └────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘

use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::events::{AgentEvent, EventBus};
use crate::error::Result;

/// Configuration for Self-Improvement Loop
#[derive(Debug, Clone)]
pub struct SelfImprovementConfig {
    /// How often to run improvement analysis (default: 1 hour)
    pub analysis_interval: Duration,
    /// Minimum samples before adaptation
    pub min_samples_for_adaptation: usize,
    /// Learning rate for parameter updates
    pub learning_rate: f32,
    /// Enable parameter auto-tuning
    pub enable_auto_tuning: bool,
    /// Enable strategy adaptation
    pub enable_strategy_adaptation: bool,
    /// Performance history window size
    pub performance_window_size: usize,
    /// Adaptation threshold (minimum improvement required)
    pub adaptation_threshold: f32,
}

impl Default for SelfImprovementConfig {
    fn default() -> Self {
        Self {
            analysis_interval: Duration::from_secs(3600), // 1 hour
            min_samples_for_adaptation: 10,
            learning_rate: 0.1,
            enable_auto_tuning: true,
            enable_strategy_adaptation: true,
            performance_window_size: 100,
            adaptation_threshold: 0.05, // 5% improvement
        }
    }
}

/// Performance metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetric {
    pub timestamp: DateTime<Utc>,
    pub metric_name: String,
    pub value: f64,
    pub context: HashMap<String, String>,
}

/// Performance analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnalysis {
    pub timestamp: DateTime<Utc>,
    pub metric_name: String,
    pub current_value: f64,
    pub trend: Trend,
    pub average: f64,
    pub min: f64,
    pub max: f64,
    pub std_dev: f64,
    pub recommendations: Vec<Recommendation>,
}

/// Trend direction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Trend {
    Improving,
    Stable,
    Degrading,
    Volatile,
}

/// Recommendation for improvement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub category: String,
    pub description: String,
    pub expected_impact: f32,
    pub confidence: f32,
    pub action: RecommendedAction,
}

/// Recommended action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendedAction {
    AdjustParameter { parameter: String, new_value: f64 },
    ChangeStrategy { from: String, to: String },
    IncreaseResource { resource: String, amount: f64 },
    DecreaseResource { resource: String, amount: f64 },
    EnableFeature { feature: String },
    DisableFeature { feature: String },
}

/// Tunable parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunableParameter {
    pub name: String,
    pub current_value: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub optimal_value: Option<f64>,
    pub description: String,
}

/// Adaptation strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptationStrategy {
    pub name: String,
    pub description: String,
    pub parameters: HashMap<String, f64>,
    pub success_rate: f32,
    pub usage_count: u32,
}

/// System component being optimized
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizableComponent {
    pub name: String,
    pub parameters: Vec<TunableParameter>,
    pub strategies: Vec<AdaptationStrategy>,
    pub active_strategy: String,
    pub performance_history: VecDeque<PerformanceMetric>,
}

/// Statistics for Self-Improvement
#[derive(Debug, Clone, Default)]
pub struct SelfImprovementStats {
    pub total_analysis_runs: u64,
    pub adaptations_made: u64,
    pub parameters_tuned: u64,
    pub strategies_changed: u64,
    pub improvements_achieved: f64,
    pub last_analysis: Option<DateTime<Utc>>,
    pub avg_analysis_duration_ms: u64,
}

/// Self-Improvement Loop system
pub struct SelfImprovementLoop {
    config: SelfImprovementConfig,
    event_bus: Arc<EventBus>,
    /// Metrics storage
    metrics: Arc<RwLock<Vec<PerformanceMetric>>>,
    /// Optimizable components
    components: Arc<RwLock<HashMap<String, OptimizableComponent>>>,
    /// Statistics
    stats: Arc<RwLock<SelfImprovementStats>>,
    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
}

impl SelfImprovementLoop {
    pub fn new(
        config: SelfImprovementConfig,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            config,
            event_bus,
            metrics: Arc::new(RwLock::new(Vec::new())),
            components: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(SelfImprovementStats::default())),
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Register an optimizable component
    pub async fn register_component(&self, component: OptimizableComponent) {
        self.components.write().await.insert(component.name.clone(), component);
        info!("Registered optimizable component: {}", component.name);
    }

    /// Record a performance metric
    pub async fn record_metric(&self, metric: PerformanceMetric) {
        self.metrics.write().await.push(metric);
        
        // Trim old metrics
        let max_size = self.config.performance_window_size * 10;
        let mut metrics = self.metrics.write().await;
        while metrics.len() > max_size {
            metrics.remove(0);
        }
    }

    /// Start the improvement loop
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            info!(
                "Self-Improvement Loop started (interval: {:?})",
                self.config.analysis_interval
            );

            let mut interval = tokio::time::interval(self.config.analysis_interval);

            loop {
                interval.tick().await;

                if *self.shutdown.read().await {
                    info!("Self-Improvement Loop shutting down");
                    break;
                }

                if let Err(e) = self.run_analysis().await {
                    warn!("Self-improvement analysis failed: {}", e);
                }
            }
        });
    }

    /// Stop the improvement loop
    pub async fn stop(&self) {
        *self.shutdown.write().await = true;
    }

    /// Run analysis and adaptation cycle
    pub async fn run_analysis(&self) -> Result<()> {
        let start_time = std::time::Instant::now();
        info!("Starting self-improvement analysis");

        let components = self.components.read().await;
        
        for (name, component) in components.iter() {
            // Analyze component performance
            let analysis = self.analyze_component(component).await?;
            
            // Generate recommendations
            let recommendations = self.generate_recommendations(component, &analysis).await?;
            
            // Apply adaptations if enabled
            if self.config.enable_auto_tuning {
                for rec in recommendations {
                    if rec.confidence >= 0.7 && rec.expected_impact >= self.config.adaptation_threshold {
                        self.apply_recommendation(name, &rec).await?;
                        
                        self.stats.write().await.adaptations_made += 1;
                    }
                }
            }

            // Publish analysis event
            self.event_bus.publish(AgentEvent::SystemLog(format!(
                "Component {} analysis: trend={:?}, avg={:.2}",
                name, analysis.trend, analysis.average
            )));
        }
        drop(components);

        // Update stats
        let duration_ms = start_time.elapsed().as_millis() as u64;
        {
            let mut stats = self.stats.write().await;
            stats.total_analysis_runs += 1;
            stats.last_analysis = Some(Utc::now());

            if stats.total_analysis_runs == 1 {
                stats.avg_analysis_duration_ms = duration_ms;
            } else {
                stats.avg_analysis_duration_ms =
                    (stats.avg_analysis_duration_ms * (stats.total_analysis_runs - 1) + duration_ms)
                    / stats.total_analysis_runs;
            }
        }

        info!("Self-improvement analysis completed in {}ms", duration_ms);

        Ok(())
    }

    /// Analyze a component's performance
    async fn analyze_component(&self, component: &OptimizableComponent) -> Result<PerformanceAnalysis> {
        let history: Vec<_> = component.performance_history.iter().cloned().collect();
        
        if history.len() < self.config.min_samples_for_adaptation {
            return Ok(PerformanceAnalysis {
                timestamp: Utc::now(),
                metric_name: component.name.clone(),
                current_value: 0.0,
                trend: Trend::Stable,
                average: 0.0,
                min: 0.0,
                max: 0.0,
                std_dev: 0.0,
                recommendations: vec![],
            });
        }

        let values: Vec<f64> = history.iter().map(|m| m.value).collect();
        
        // Calculate statistics
        let current = *values.last().unwrap_or(&0.0);
        let average = values.iter().sum::<f64>() / values.len() as f64;
        let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        // Calculate standard deviation
        let variance = values.iter()
            .map(|v| (v - average).powi(2))
            .sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        // Determine trend
        let trend = if values.len() >= 10 {
            let recent: f64 = values.iter().rev().take(5).sum::<f64>() / 5.0;
            let older: f64 = values.iter().rev().skip(5).take(5).sum::<f64>() / 5.0;
            
            let change = (recent - older) / older.abs().max(1.0);
            
            if change > 0.1 {
                Trend::Improving
            } else if change < -0.1 {
                Trend::Degrading
            } else if std_dev / average > 0.2 {
                Trend::Volatile
            } else {
                Trend::Stable
            }
        } else {
            Trend::Stable
        };

        Ok(PerformanceAnalysis {
            timestamp: Utc::now(),
            metric_name: component.name.clone(),
            current_value: current,
            trend,
            average,
            min,
            max,
            std_dev,
            recommendations: vec![],
        })
    }

    /// Generate recommendations based on analysis
    async fn generate_recommendations(
        &self,
        component: &OptimizableComponent,
        analysis: &PerformanceAnalysis,
    ) -> Result<Vec<Recommendation>> {
        let mut recommendations = Vec::new();

        // Analyze each parameter
        for param in &component.parameters {
            let current = param.current_value;
            let optimal = param.optimal_value.unwrap_or(current);
            
            // If performance is degrading, suggest parameter adjustment
            if analysis.trend == Trend::Degrading {
                let adjustment = (optimal - current) * self.config.learning_rate as f64;
                let new_value = (current + adjustment)
                    .clamp(param.min_value, param.max_value);
                
                if (new_value - current).abs() > 0.01 {
                    recommendations.push(Recommendation {
                        category: "parameter_tuning".to_string(),
                        description: format!(
                            "Adjust {} from {:.2} to {:.2}",
                            param.name, current, new_value
                        ),
                        expected_impact: 0.1,
                        confidence: 0.7,
                        action: RecommendedAction::AdjustParameter {
                            parameter: param.name.clone(),
                            new_value,
                        },
                    });
                }
            }
        }

        // Strategy recommendations
        if self.config.enable_strategy_adaptation {
            // Find best performing strategy
            let best_strategy = component.strategies.iter()
                .max_by(|a, b| a.success_rate.partial_cmp(&b.success_rate).unwrap());
            
            if let Some(best) = best_strategy {
                if best.name != component.active_strategy && best.success_rate > 0.7 {
                    recommendations.push(Recommendation {
                        category: "strategy_change".to_string(),
                        description: format!(
                            "Switch from {} to {} strategy (success rate: {:.1}%)",
                            component.active_strategy, best.name, best.success_rate * 100.0
                        ),
                        expected_impact: best.success_rate - 0.5,
                        confidence: best.success_rate,
                        action: RecommendedAction::ChangeStrategy {
                            from: component.active_strategy.clone(),
                            to: best.name.clone(),
                        },
                    });
                }
            }
        }

        // Resource recommendations based on volatility
        if analysis.trend == Trend::Volatile {
            recommendations.push(Recommendation {
                category: "resource_allocation".to_string(),
                description: "Increase resource allocation to stabilize performance".to_string(),
                expected_impact: 0.15,
                confidence: 0.6,
                action: RecommendedAction::IncreaseResource {
                    resource: "compute".to_string(),
                    amount: 0.2,
                },
            });
        }

        Ok(recommendations)
    }

    /// Apply a recommendation
    async fn apply_recommendation(&self, component_name: &str, recommendation: &Recommendation) -> Result<()> {
        info!(
            "Applying recommendation for {}: {:?}",
            component_name, recommendation.action
        );

        let mut components = self.components.write().await;
        
        if let Some(component) = components.get_mut(component_name) {
            match &recommendation.action {
                RecommendedAction::AdjustParameter { parameter, new_value } => {
                    if let Some(param) = component.parameters.iter_mut().find(|p| p.name == *parameter) {
                        param.current_value = *new_value;
                        self.stats.write().await.parameters_tuned += 1;
                        
                        info!(
                            "Tuned parameter {} to {:.2} for component {}",
                            parameter, new_value, component_name
                        );
                    }
                }
                RecommendedAction::ChangeStrategy { from, to } => {
                    component.active_strategy = to.clone();
                    self.stats.write().await.strategies_changed += 1;
                    
                    info!(
                        "Changed strategy from {} to {} for component {}",
                        from, to, component_name
                    );
                }
                RecommendedAction::IncreaseResource { resource, amount } => {
                    info!(
                        "Increasing resource {} by {:.0}% for component {}",
                        resource, amount * 100.0, component_name
                    );
                }
                RecommendedAction::DecreaseResource { resource, amount } => {
                    info!(
                        "Decreasing resource {} by {:.0}% for component {}",
                        resource, amount * 100.0, component_name
                    );
                }
                RecommendedAction::EnableFeature { feature } => {
                    info!("Enabling feature {} for component {}", feature, component_name);
                }
                RecommendedAction::DisableFeature { feature } => {
                    info!("Disabling feature {} for component {}", feature, component_name);
                }
            }

            // Publish event
            self.event_bus.publish(AgentEvent::SystemLog(format!(
                "Applied adaptation to {}: {}",
                component_name, recommendation.description
            )));
        }

        Ok(())
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> SelfImprovementStats {
        self.stats.read().await.clone()
    }

    /// Get component status
    pub async fn get_component_status(&self, name: &str) -> Option<OptimizableComponent> {
        self.components.read().await.get(name).cloned()
    }

    /// Get all component names
    pub async fn get_component_names(&self) -> Vec<String> {
        self.components.read().await.keys().cloned().collect()
    }

    /// Force analysis run
    pub async fn force_analysis(&self) -> Result<()> {
        self.run_analysis().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_metric_creation() {
        let metric = PerformanceMetric {
            timestamp: Utc::now(),
            metric_name: "response_time".to_string(),
            value: 150.0,
            context: HashMap::new(),
        };

        assert_eq!(metric.metric_name, "response_time");
        assert_eq!(metric.value, 150.0);
    }

    #[test]
    fn test_tunable_parameter_creation() {
        let param = TunableParameter {
            name: "threshold".to_string(),
            current_value: 0.5,
            min_value: 0.0,
            max_value: 1.0,
            optimal_value: Some(0.7),
            description: "Example threshold".to_string(),
        };

        assert_eq!(param.current_value, 0.5);
        assert_eq!(param.min_value, 0.0);
        assert_eq!(param.max_value, 1.0);
    }

    #[test]
    fn test_recommendation_creation() {
        let rec = Recommendation {
            category: "parameter_tuning".to_string(),
            description: "Adjust threshold".to_string(),
            expected_impact: 0.15,
            confidence: 0.8,
            action: RecommendedAction::AdjustParameter {
                parameter: "threshold".to_string(),
                new_value: 0.7,
            },
        };

        assert_eq!(rec.expected_impact, 0.15);
        assert_eq!(rec.confidence, 0.8);
    }

    #[test]
    fn test_self_improvement_config_default() {
        let config = SelfImprovementConfig::default();
        assert_eq!(config.analysis_interval, Duration::from_secs(3600));
        assert_eq!(config.min_samples_for_adaptation, 10);
        assert!(config.enable_auto_tuning);
        assert!(config.enable_strategy_adaptation);
    }

    #[test]
    fn test_trend_ordering() {
        assert_ne!(Trend::Improving, Trend::Degrading);
        assert_ne!(Trend::Stable, Trend::Volatile);
    }
}
