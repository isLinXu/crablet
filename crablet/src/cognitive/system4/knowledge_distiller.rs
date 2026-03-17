//! Knowledge Distiller - 知识蒸馏引擎
//!
//! 将 System3 的复杂多 Agent 执行结果蒸馏为 System2 可直接使用的知识和技能

use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::cognitive::llm::LlmClient;
use crate::types::Message;

/// 蒸馏任务
#[derive(Debug, Clone)]
pub struct DistillationTask {
    pub id: String,
    pub source_system: DistillationSourceSystem,
    pub input: String,
    pub output: String,
    pub execution_trace: ExecutionTrace,
    pub priority: DistillationPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistillationSourceSystem {
    System3,
    System2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistillationPriority {
    Low,
    Medium,
    High,
}

/// 执行轨迹
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    pub steps: Vec<TraceStep>,
    pub agent_interactions: Vec<AgentInteraction>,
    pub tool_calls: Vec<ToolCallRecord>,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    pub step_number: usize,
    pub action: String,
    pub input: String,
    pub output: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInteraction {
    pub from_agent: String,
    pub to_agent: String,
    pub message_type: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool_name: String,
    pub parameters: serde_json::Value,
    pub result: serde_json::Value,
    pub duration_ms: u64,
}

/// 蒸馏结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillationResult {
    pub task_id: String,
    pub success: bool,
    pub distilled_knowledge: Option<DistilledKnowledge>,
    pub extracted_skill: Option<ExtractedSkill>,
    pub confidence: f64,
    pub processing_time_ms: u64,
    pub error_message: Option<String>,
}

/// 蒸馏后的知识
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistilledKnowledge {
    pub id: String,
    pub topic: String,
    pub summary: String,
    pub key_facts: Vec<String>,
    pub relationships: Vec<Relationship>,
    pub source_trace_id: String,
    pub created_at: DateTime<Utc>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
}

/// 提取的技能
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSkill {
    pub name: String,
    pub description: String,
    pub input_pattern: String,
    pub execution_template: String,
    pub required_tools: Vec<String>,
    pub example_inputs: Vec<String>,
    pub example_outputs: Vec<String>,
    pub confidence: f64,
}

/// 知识蒸馏引擎
pub struct KnowledgeDistiller {
    llm: Arc<Box<dyn LlmClient>>,
    knowledge_base: Arc<RwLock<Vec<DistilledKnowledge>>>,
    extracted_skills: Arc<RwLock<Vec<ExtractedSkill>>>,
    max_knowledge_entries: usize,
}

impl KnowledgeDistiller {
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self {
            llm,
            knowledge_base: Arc::new(RwLock::new(Vec::new())),
            extracted_skills: Arc::new(RwLock::new(Vec::new())),
            max_knowledge_entries: 10000,
        }
    }

    pub fn with_capacity(llm: Arc<Box<dyn LlmClient>>, max_entries: usize) -> Self {
        Self {
            llm,
            knowledge_base: Arc::new(RwLock::new(Vec::new())),
            extracted_skills: Arc::new(RwLock::new(Vec::new())),
            max_knowledge_entries: max_entries,
        }
    }

    /// 执行蒸馏任务
    pub async fn distill(&self, task: DistillationTask) -> DistillationResult {
        let start_time = std::time::Instant::now();
        
        info!("Starting distillation for task {}", task.id);

        // 1. 分析执行轨迹
        let trace_analysis = self.analyze_trace(&task.execution_trace).await;
        
        // 2. 提取知识
        let knowledge_result = self.extract_knowledge(&task, &trace_analysis).await;
        
        // 3. 提取可复用技能
        let skill_result = self.extract_skill(&task, &trace_analysis).await;

        let processing_time = start_time.elapsed().as_millis() as u64;

        let result = DistillationResult {
            task_id: task.id.clone(),
            success: knowledge_result.is_some() || skill_result.is_some(),
            distilled_knowledge: knowledge_result.clone(),
            extracted_skill: skill_result.clone(),
            confidence: self.calculate_overall_confidence(&knowledge_result, &skill_result),
            processing_time_ms: processing_time,
            error_message: None,
        };

        // 4. 存储结果
        if let Some(knowledge) = knowledge_result {
            self.store_knowledge(knowledge).await;
        }
        
        if let Some(skill) = skill_result {
            self.store_skill(skill).await;
        }

        info!("Distillation completed for task {} in {}ms", task.id, processing_time);
        result
    }

    /// 分析执行轨迹
    async fn analyze_trace(&self, trace: &ExecutionTrace) -> TraceAnalysis {
        let mut tool_frequency: HashMap<String, usize> = HashMap::new();
        let mut agent_communication_patterns: Vec<String> = Vec::new();
        let mut key_decision_points: Vec<usize> = Vec::new();

        // 统计工具使用频率
        for tool_call in &trace.tool_calls {
            *tool_frequency.entry(tool_call.tool_name.clone()).or_insert(0) += 1;
        }

        // 分析 Agent 通信模式
        for interaction in &trace.agent_interactions {
            let pattern = format!("{} -> {}: {}", 
                interaction.from_agent, 
                interaction.to_agent,
                interaction.message_type
            );
            if !agent_communication_patterns.contains(&pattern) {
                agent_communication_patterns.push(pattern);
            }
        }

        // 识别关键决策点（工具调用较多的步骤）
        for (i, step) in trace.steps.iter().enumerate() {
            if step.action.contains("decide") || step.action.contains("plan") {
                key_decision_points.push(i);
            }
        }

        TraceAnalysis {
            tool_frequency,
            agent_communication_patterns,
            key_decision_points,
            total_steps: trace.steps.len(),
            total_tool_calls: trace.tool_calls.len(),
            total_agent_interactions: trace.agent_interactions.len(),
        }
    }

    /// 提取知识
    async fn extract_knowledge(
        &self,
        task: &DistillationTask,
        analysis: &TraceAnalysis,
    ) -> Option<DistilledKnowledge> {
        // 构建 LLM 提示
        let prompt = self.build_knowledge_extraction_prompt(task, analysis);
        
        let messages = vec![Message::user(&prompt)];
        
        match self.llm.chat_complete(&messages).await {
            Ok(response) => {
                match self.parse_knowledge_extraction(&response, &task.id) {
                    Ok(knowledge) => {
                        debug!("Successfully extracted knowledge for topic: {}", knowledge.topic);
                        Some(knowledge)
                    }
                    Err(e) => {
                        warn!("Failed to parse knowledge extraction: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                warn!("LLM call failed during knowledge extraction: {}", e);
                None
            }
        }
    }

    fn build_knowledge_extraction_prompt(&self, task: &DistillationTask, analysis: &TraceAnalysis) -> String {
        let tool_summary: String = analysis.tool_frequency
            .iter()
            .map(|(tool, count)| format!("- {}: {} times", tool, count))
            .collect::<Vec<_>>()
            .join("\n");

        let steps_summary: String = task.execution_trace.steps
            .iter()
            .take(10)
            .map(|s| format!("{}. {}: {}", s.step_number, s.action, s.output.chars().take(100).collect::<String>()))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "Analyze the following multi-agent execution and extract structured knowledge:\n\n\
            Input: {}\n\
            Output: {}\n\n\
            Execution Summary:\n\
            - Total steps: {}\n\
            - Tool calls: {}\n\
            - Agent interactions: {}\n\n\
            Tool Usage:\n{}\n\n\
            Key Steps:\n{}\n\n\
            Extract the knowledge in the following JSON format:\n\
            {{\n\
              \"topic\": \"main topic\",\n\
              \"summary\": \"concise summary of what was learned\",\n\
              \"key_facts\": [\"fact 1\", \"fact 2\", ...],\n\
              \"relationships\": [\n\
                {{\"subject\": \"A\", \"predicate\": \"relates to\", \"object\": \"B\", \"confidence\": 0.9}}\n\
              ]\n\
            }}",
            task.input,
            task.output.chars().take(500).collect::<String>(),
            analysis.total_steps,
            analysis.total_tool_calls,
            analysis.total_agent_interactions,
            tool_summary,
            steps_summary
        )
    }

    fn parse_knowledge_extraction(&self, response: &str, task_id: &str) -> anyhow::Result<DistilledKnowledge> {
        // 提取 JSON 部分
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        #[derive(Deserialize)]
        struct KnowledgeExtraction {
            topic: String,
            summary: String,
            key_facts: Vec<String>,
            relationships: Vec<Relationship>,
        }

        let extraction: KnowledgeExtraction = serde_json::from_str(json_str)?;

        Ok(DistilledKnowledge {
            id: format!("knowledge_{}", &uuid::Uuid::new_v4().to_string()[..8]),
            topic: extraction.topic,
            summary: extraction.summary,
            key_facts: extraction.key_facts,
            relationships: extraction.relationships.clone(),
            source_trace_id: task_id.to_string(),
            created_at: Utc::now(),
            confidence: self.calculate_knowledge_confidence(&extraction.relationships),
        })
    }

    fn calculate_knowledge_confidence(&self, relationships: &[Relationship]) -> f64 {
        if relationships.is_empty() {
            return 0.5;
        }
        
        let avg_confidence: f64 = relationships.iter().map(|r| r.confidence).sum::<f64>() 
            / relationships.len() as f64;
        avg_confidence
    }

    /// 提取技能
    async fn extract_skill(
        &self,
        task: &DistillationTask,
        analysis: &TraceAnalysis,
    ) -> Option<ExtractedSkill> {
        // 只有当执行足够复杂且成功时才提取技能
        if analysis.total_steps < 3 || analysis.total_tool_calls < 2 {
            return None;
        }

        let prompt = self.build_skill_extraction_prompt(task, analysis);
        let messages = vec![Message::user(&prompt)];

        match self.llm.chat_complete(&messages).await {
            Ok(response) => {
                match self.parse_skill_extraction(&response) {
                    Ok(skill) => {
                        debug!("Successfully extracted skill: {}", skill.name);
                        Some(skill)
                    }
                    Err(e) => {
                        warn!("Failed to parse skill extraction: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                warn!("LLM call failed during skill extraction: {}", e);
                None
            }
        }
    }

    fn build_skill_extraction_prompt(&self, task: &DistillationTask, analysis: &TraceAnalysis) -> String {
        format!(
            "Analyze this successful multi-agent execution and extract a reusable skill:\n\n\
            Input: {}\n\
            Output: {}\n\n\
            Tools used: {:?}\n\
            Steps: {}\n\n\
            Extract a skill definition in JSON format:\n\
            {{\n\
              \"name\": \"skill_name\",\n\
              \"description\": \"what this skill does\",\n\
              \"input_pattern\": \"regex or pattern to match inputs\",\n\
              \"execution_template\": \"template for execution\",\n\
              \"required_tools\": [\"tool1\", \"tool2\"],\n\
              \"example_inputs\": [\"example 1\", \"example 2\"],\n\
              \"example_outputs\": [\"output 1\", \"output 2\"]\n\
            }}",
            task.input,
            task.output.chars().take(300).collect::<String>(),
            analysis.tool_frequency.keys().collect::<Vec<_>>(),
            analysis.total_steps
        )
    }

    fn parse_skill_extraction(&self, response: &str) -> anyhow::Result<ExtractedSkill> {
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        #[derive(Deserialize)]
        struct SkillExtraction {
            name: String,
            description: String,
            input_pattern: String,
            execution_template: String,
            required_tools: Vec<String>,
            example_inputs: Vec<String>,
            example_outputs: Vec<String>,
        }

        let extraction: SkillExtraction = serde_json::from_str(json_str)?;

        Ok(ExtractedSkill {
            name: extraction.name,
            description: extraction.description,
            input_pattern: extraction.input_pattern,
            execution_template: extraction.execution_template,
            required_tools: extraction.required_tools,
            example_inputs: extraction.example_inputs,
            example_outputs: extraction.example_outputs,
            confidence: 0.7, // 默认置信度，可根据更多因素调整
        })
    }

    fn calculate_overall_confidence(
        &self,
        knowledge: &Option<DistilledKnowledge>,
        skill: &Option<ExtractedSkill>,
    ) -> f64 {
        match (knowledge, skill) {
            (Some(k), Some(s)) => (k.confidence + s.confidence) / 2.0,
            (Some(k), None) => k.confidence,
            (None, Some(s)) => s.confidence,
            (None, None) => 0.0,
        }
    }

    /// 存储知识
    async fn store_knowledge(&self, knowledge: DistilledKnowledge) {
        let mut kb = self.knowledge_base.write().await;
        
        if kb.len() >= self.max_knowledge_entries {
            // 移除最旧的知识
            kb.remove(0);
        }
        
        kb.push(knowledge);
        debug!("Stored knowledge, total entries: {}", kb.len());
    }

    /// 存储技能
    async fn store_skill(&self, skill: ExtractedSkill) {
        let mut skills = self.extracted_skills.write().await;
        skills.push(skill);
        debug!("Stored extracted skill, total: {}", skills.len());
    }

    /// 查询知识
    pub async fn query_knowledge(&self, topic: &str) -> Vec<DistilledKnowledge> {
        let kb = self.knowledge_base.read().await;
        
        kb.iter()
            .filter(|k| {
                k.topic.to_lowercase().contains(&topic.to_lowercase()) ||
                k.summary.to_lowercase().contains(&topic.to_lowercase()) ||
                k.key_facts.iter().any(|f| f.to_lowercase().contains(&topic.to_lowercase()))
            })
            .cloned()
            .collect()
    }

    /// 获取所有提取的技能
    pub async fn get_extracted_skills(&self) -> Vec<ExtractedSkill> {
        self.extracted_skills.read().await.clone()
    }

    /// 批量蒸馏
    pub async fn distill_batch(&self, tasks: Vec<DistillationTask>) -> Vec<DistillationResult> {
        let mut results = Vec::new();
        
        for task in tasks {
            let result = self.distill(task).await;
            results.push(result);
        }
        
        results
    }
}

/// 轨迹分析结果
struct TraceAnalysis {
    tool_frequency: HashMap<String, usize>,
    agent_communication_patterns: Vec<String>,
    key_decision_points: Vec<usize>,
    total_steps: usize,
    total_tool_calls: usize,
    total_agent_interactions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mocks::MockLlmClient;

    // 注意：这些测试需要 MockLlmClient 才能运行
    // 这里仅作为结构示例

    #[test]
    fn test_knowledge_confidence_calculation() {
        let distiller = KnowledgeDistiller::new(Arc::new(Box::new(MockLlmClient::new())));
        
        let relationships = vec![
            Relationship {
                subject: "A".to_string(),
                predicate: "relates to".to_string(),
                object: "B".to_string(),
                confidence: 0.9,
            },
            Relationship {
                subject: "B".to_string(),
                predicate: "relates to".to_string(),
                object: "C".to_string(),
                confidence: 0.7,
            },
        ];

        let confidence = distiller.calculate_knowledge_confidence(&relationships);
        assert!((confidence - 0.8).abs() < 0.01);
    }
}
