//! Execution Metrics
//!
//! Performance tracking and cost analysis for Agent executions.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Execution metrics collector
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub execution_id: String,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub step_metrics: Vec<StepMetrics>,
    pub token_usage: TokenUsage,
    pub cost: CostBreakdown,
    pub performance: PerformanceStats,
    // Aliases for API compatibility
    pub total_steps: usize,
    pub total_tokens: usize,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub estimated_cost: f64,
    pub duration_ms: u64,
}

impl ExecutionMetrics {
    pub fn new(execution_id: String) -> Self {
        Self {
            execution_id: execution_id.clone(),
            start_time: current_timestamp(),
            end_time: None,
            step_metrics: Vec::new(),
            token_usage: TokenUsage::default(),
            cost: CostBreakdown::default(),
            performance: PerformanceStats::default(),
            total_steps: 0,
            total_tokens: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            estimated_cost: 0.0,
            duration_ms: 0,
        }
    }

    pub fn record_step(&mut self, step: StepMetrics) {
        self.step_metrics.push(step);
    }

    pub fn add_tokens(&mut self, input: usize, output: usize, model: &str) {
        self.token_usage.input += input;
        self.token_usage.output += output;
        self.token_usage.total += input + output;
        
        *self.token_usage.by_model.entry(model.to_string()).or_insert(0) += input + output;
        
        // Update cost
        let input_cost = self.calculate_input_cost(input, model);
        let output_cost = self.calculate_output_cost(output, model);
        self.cost.total += input_cost + output_cost;
        self.cost.breakdown.insert(format!("{}_input", model), input_cost);
        self.cost.breakdown.insert(format!("{}_output", model), output_cost);
    }

    pub fn finish(&mut self) {
        self.end_time = Some(current_timestamp());
        self.calculate_performance_stats();
    }

    fn calculate_input_cost(&self, tokens: usize, model: &str) -> f64 {
        let rate = match model {
            "gpt-4" => 0.03,
            "gpt-4-turbo" => 0.01,
            "gpt-3.5-turbo" => 0.0015,
            "claude-3-opus" => 0.015,
            "claude-3-sonnet" => 0.003,
            _ => 0.01,
        };
        (tokens as f64 / 1000.0) * rate
    }

    fn calculate_output_cost(&self, tokens: usize, model: &str) -> f64 {
        let rate = match model {
            "gpt-4" => 0.06,
            "gpt-4-turbo" => 0.03,
            "gpt-3.5-turbo" => 0.002,
            "claude-3-opus" => 0.075,
            "claude-3-sonnet" => 0.015,
            _ => 0.03,
        };
        (tokens as f64 / 1000.0) * rate
    }

    fn calculate_performance_stats(&mut self) {
        if self.step_metrics.is_empty() {
            return;
        }

        let durations: Vec<u64> = self.step_metrics.iter()
            .map(|s| s.duration_ms)
            .collect();

        self.performance.avg_step_duration_ms = durations.iter().sum::<u64>() / durations.len() as u64;
        self.performance.max_step_duration_ms = *durations.iter().max().unwrap_or(&0);
        self.performance.min_step_duration_ms = *durations.iter().min().unwrap_or(&0);
        self.performance.total_steps = self.step_metrics.len();
        
        // Calculate total duration
        if let Some(end) = self.end_time {
            self.performance.total_duration_ms = end - self.start_time;
        }
    }
}

/// Metrics for a single step
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepMetrics {
    pub step_number: usize,
    pub step_type: String,
    pub duration_ms: u64,
    pub token_usage: TokenUsage,
    pub tool_calls: usize,
    pub llm_calls: usize,
    pub success: bool,
    pub error: Option<String>,
}

/// Token usage tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: usize,
    pub output: usize,
    pub total: usize,
    pub by_model: HashMap<String, usize>,
}

/// Cost breakdown
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub total: f64,
    pub breakdown: HashMap<String, f64>,
}

/// Performance statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceStats {
    pub total_duration_ms: u64,
    pub total_steps: usize,
    pub avg_step_duration_ms: u64,
    pub max_step_duration_ms: u64,
    pub min_step_duration_ms: u64,
    pub tool_call_count: usize,
    pub error_count: usize,
    pub retry_count: usize,
}

/// Cost tracker for budget management
pub struct CostTracker {
    budget: Option<f64>,
    current_cost: f64,
    alert_threshold: f64,
}

impl CostTracker {
    pub fn new(budget: Option<f64>) -> Self {
        Self {
            budget,
            current_cost: 0.0,
            alert_threshold: 0.8, // 80%
        }
    }

    pub fn add_cost(&mut self, cost: f64) -> CostStatus {
        self.current_cost += cost;

        if let Some(budget) = self.budget {
            if self.current_cost >= budget {
                return CostStatus::BudgetExceeded;
            }
            
            if self.current_cost >= budget * self.alert_threshold {
                return CostStatus::NearBudget;
            }
        }

        CostStatus::Ok
    }

    pub fn current_cost(&self) -> f64 {
        self.current_cost
    }

    pub fn remaining_budget(&self) -> Option<f64> {
        self.budget.map(|b| b - self.current_cost)
    }
}

#[derive(Debug, Clone)]
pub enum CostStatus {
    Ok,
    NearBudget,
    BudgetExceeded,
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
