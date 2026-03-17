//! Reflector - 反思问题并生成改进建议

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::error::Result;
use crate::cognitive::llm::LlmClient;
use crate::cognitive::meta_controller::monitor::ExecutionMetrics;

/// 问题类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProblemType {
    /// 执行失败
    ExecutionFailed,
    /// 低置信度
    LowConfidence,
    /// 资源耗尽
    ResourceExhaustion,
    /// 性能问题
    PerformanceIssue,
    /// 质量问题
    QualityIssue,
    /// 其他
    Other(String),
}

/// 问题诊断
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemDiagnosis {
    /// 问题类型
    pub problem_type: ProblemType,
    /// 问题描述
    pub description: String,
    /// 严重程度 (0-1)
    pub severity: f32,
    /// 根本原因
    pub root_cause: Option<String>,
    /// 改进建议
    pub suggested_actions: Vec<ImprovementAction>,
}

/// 改进动作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementAction {
    /// 动作类型
    pub action_type: ActionType,
    /// 动作描述
    pub description: String,
    /// 优先级 (0-1)
    pub priority: f32,
    /// 预期效果
    pub expected_impact: String,
}

/// 动作类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    /// 切换策略
    SwitchStrategy { new_strategy: String },
    /// 更新知识
    UpdateKnowledge { knowledge_id: String, content: String },
    /// 调整参数
    AdjustParameters { parameters: std::collections::HashMap<String, serde_json::Value> },
    /// 优化提示
    OptimizePrompt { new_prompt: String },
    /// 增加上下文
    AddContext { context: String },
    /// 其他
    Other(String),
}

/// 反思器
pub struct Reflector {
    llm: Arc<Box<dyn LlmClient>>,
}

impl Reflector {
    /// 创建新的反思器
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self { llm }
    }

    /// 诊断问题
    pub async fn diagnose(&self, task: &str, metrics: &ExecutionMetrics) -> Result<ProblemDiagnosis> {
        debug!("Diagnosing problem for task: {}", task);

        // 分析问题类型
        let problem_type = self.classify_problem(task, metrics);

        // 生成描述
        let description = self.generate_description(&problem_type, task, metrics);

        // 评估严重程度
        let severity = self.assess_severity(&problem_type, metrics);

        // 分析根本原因
        let root_cause = self.analyze_root_cause(&problem_type, task, metrics).await?;

        // 生成改进建议
        let suggested_actions = self.generate_improvements(&problem_type, &root_cause, task).await?;

        Ok(ProblemDiagnosis {
            problem_type,
            description,
            severity,
            root_cause,
            suggested_actions,
        })
    }

    /// 分类问题类型
    fn classify_problem(&self, _task: &str, metrics: &ExecutionMetrics) -> ProblemType {
        if !metrics.success {
            ProblemType::ExecutionFailed
        } else if metrics.confidence < 0.5 {
            ProblemType::LowConfidence
        } else if metrics.resources.duration_ms > 10000 {
            ProblemType::PerformanceIssue
        } else if metrics.resources.memory_bytes > 512 * 1024 * 1024 {
            ProblemType::ResourceExhaustion
        } else {
            ProblemType::Other("General issue".into())
        }
    }

    /// 生成问题描述
    fn generate_description(&self, problem_type: &ProblemType, task: &str, metrics: &ExecutionMetrics) -> String {
        match problem_type {
            ProblemType::ExecutionFailed => {
                format!(
                    "Task '{}' failed to execute successfully. Error: {}",
                    task,
                    metrics.error.as_deref().unwrap_or("Unknown error")
                )
            }
            ProblemType::LowConfidence => {
                format!(
                    "Task '{}' executed with low confidence ({:.2}). Output may be unreliable.",
                    task, metrics.confidence
                )
            }
            ProblemType::PerformanceIssue => {
                format!(
                    "Task '{}' took {}ms to execute, exceeding performance threshold.",
                    task, metrics.resources.duration_ms
                )
            }
            ProblemType::ResourceExhaustion => {
                format!(
                    "Task '{}' consumed excessive resources ({}MB memory).",
                    task,
                    metrics.resources.memory_bytes / (1024 * 1024)
                )
            }
            ProblemType::QualityIssue => {
                format!(
                    "Task '{}' output quality is below acceptable threshold.",
                    task
                )
            }
            ProblemType::Other(desc) => {
                format!("Task '{}' encountered an issue: {}", task, desc)
            }
        }
    }

    /// 评估严重程度
    fn assess_severity(&self, problem_type: &ProblemType, metrics: &ExecutionMetrics) -> f32 {
        match problem_type {
            ProblemType::ExecutionFailed => 1.0,
            ProblemType::ResourceExhaustion => 0.8,
            ProblemType::PerformanceIssue => {
                if metrics.resources.duration_ms > 30000 {
                    0.8
                } else if metrics.resources.duration_ms > 10000 {
                    0.5
                } else {
                    0.3
                }
            }
            ProblemType::LowConfidence => {
                if metrics.confidence < 0.3 {
                    0.8
                } else if metrics.confidence < 0.5 {
                    0.5
                } else {
                    0.3
                }
            }
            ProblemType::QualityIssue => 0.6,
            ProblemType::Other(_) => 0.5,
        }
    }

    /// 分析根本原因
    async fn analyze_root_cause(
        &self,
        problem_type: &ProblemType,
        task: &str,
        metrics: &ExecutionMetrics,
    ) -> Result<Option<String>> {
        // 使用 LLM 分析根本原因
        let prompt = format!(
            "Analyze the root cause of the following problem:\n\n\
            Problem Type: {:?}\n\
            Task: {}\n\
            Error: {:?}\n\
            Confidence: {:.2}\n\
            Duration: {}ms\n\n\
            What is the most likely root cause?",
            problem_type, task, metrics.error, metrics.confidence, metrics.resources.duration_ms
        );

        use crate::types::Message;
        
        match self.llm.chat_complete(&[Message::user(&prompt)]).await {
            Ok(analysis) => Ok(Some(analysis)),
            Err(e) => {
                warn!("Failed to analyze root cause with LLM: {}", e);
                Ok(None)
            }
        }
    }

    /// 生成改进建议
    async fn generate_improvements(
        &self,
        problem_type: &ProblemType,
        root_cause: &Option<String>,
        task: &str,
    ) -> Result<Vec<ImprovementAction>> {
        let mut actions = Vec::new();

        // 基于问题类型生成默认建议
        match problem_type {
            ProblemType::ExecutionFailed => {
                actions.push(ImprovementAction {
                    action_type: ActionType::SwitchStrategy {
                        new_strategy: "enhanced".to_string(),
                    },
                    description: "Switch to enhanced processing strategy with better error handling".into(),
                    priority: 0.9,
                    expected_impact: "Reduce execution failures by improving error handling".into(),
                });
                actions.push(ImprovementAction {
                    action_type: ActionType::UpdateKnowledge {
                        knowledge_id: "error_patterns".into(),
                        content: format!("Error pattern for task '{}': {:?}", task, root_cause),
                    },
                    description: "Learn from this error pattern".into(),
                    priority: 0.8,
                    expected_impact: "Improve error recognition and prevention".into(),
                });
            }
            ProblemType::LowConfidence => {
                actions.push(ImprovementAction {
                    action_type: ActionType::AddContext {
                        context: "Request additional context and clarification".into(),
                    },
                    description: "Ask for more information to improve confidence".into(),
                    priority: 0.8,
                    expected_impact: "Increase output quality and confidence".into(),
                });
                actions.push(ImprovementAction {
                    action_type: ActionType::OptimizePrompt {
                        new_prompt: "Enhance prompt with better instructions".into(),
                    },
                    description: "Optimize the prompt for better clarity".into(),
                    priority: 0.7,
                    expected_impact: "Improve LLM understanding and response quality".into(),
                });
            }
            ProblemType::PerformanceIssue => {
                actions.push(ImprovementAction {
                    action_type: ActionType::SwitchStrategy {
                        new_strategy: "fast".to_string(),
                    },
                    description: "Switch to faster processing strategy".into(),
                    priority: 0.8,
                    expected_impact: "Reduce execution time".into(),
                });
            }
            ProblemType::ResourceExhaustion => {
                actions.push(ImprovementAction {
                    action_type: ActionType::AdjustParameters {
                        parameters: {
                            let mut params = std::collections::HashMap::new();
                            params.insert("max_tokens".to_string(), serde_json::json!(1024));
                            params
                        },
                    },
                    description: "Reduce token limit to lower memory usage".into(),
                    priority: 0.9,
                    expected_impact: "Decrease memory consumption".into(),
                });
            }
            _ => {}
        }

        // 如果有根本原因分析，生成更多针对性的建议
        if let Some(cause) = root_cause {
            let prompt = format!(
                "Based on the root cause analysis: \"{}\"\n\n\
                Generate 1-2 specific improvement actions to address this issue.\n\
                Focus on actionable and high-impact solutions.",
                cause
            );

            use crate::types::Message;
            
            match self.llm.chat_complete(&[Message::user(&prompt)]).await {
                Ok(suggestions) => {
                    // 简单解析 LLM 返回的建议
                    for line in suggestions.lines().take(2) {
                        if !line.trim().is_empty() {
                            actions.push(ImprovementAction {
                                action_type: ActionType::Other(line.trim().to_string()),
                                description: line.trim().to_string(),
                                priority: 0.7,
                                expected_impact: "Address specific root cause".into(),
                            });
                        }
                    }
                }
                Err(e) => {
                    debug!("Failed to generate LLM suggestions: {}", e);
                }
            }
        }

        Ok(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognitive::llm::MockClient;

    #[tokio::test]
    async fn test_reflector_creation() {
        let llm = Arc::new(Box::new(MockClient) as Box<dyn LlmClient>);
        let _reflector = Reflector::new(llm);
        // Test passes if reflector is created successfully
    }

    #[tokio::test]
    async fn test_diagnose_execution_failed() {
        let llm = Arc::new(Box::new(MockClient) as Box<dyn LlmClient>);
        let reflector = Reflector::new(llm);

        let metrics = ExecutionMetrics {
            success: false,
            error: Some("Test error".into()),
            ..Default::default()
        };

        let diagnosis = reflector.diagnose("test task", &metrics).await.unwrap();
        assert_eq!(diagnosis.problem_type, ProblemType::ExecutionFailed);
        assert_eq!(diagnosis.severity, 1.0);
    }

    #[tokio::test]
    async fn test_diagnose_low_confidence() {
        let llm = Arc::new(Box::new(MockClient) as Box<dyn LlmClient>);
        let reflector = Reflector::new(llm);

        let metrics = ExecutionMetrics {
            success: true,
            confidence: 0.3,
            ..Default::default()
        };

        let diagnosis = reflector.diagnose("test task", &metrics).await.unwrap();
        assert_eq!(diagnosis.problem_type, ProblemType::LowConfidence);
        assert!(diagnosis.severity >= 0.5); // 0.3 confidence 对应 0.5 severity
    }
}
