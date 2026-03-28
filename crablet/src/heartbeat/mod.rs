use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, debug, warn, error};
use crate::memory::manager::MemoryManager;
use crate::cognitive::llm::LlmClient;
use crate::skills::SkillRegistry;
use crate::error::Result;
use crate::types::{Message, ContentPart};
use crate::memory::core::CoreMemoryBlock;

use crate::agent::swarm::{SwarmOrchestrator, GraphStatus, TaskNode, TaskStatus, TaskGraph};
use uuid::Uuid;

/// HeartbeatEngine handles periodic background tasks like proactive agent activities,
/// memory consolidation, and predictive maintenance.
pub struct HeartbeatEngine {
    memory_mgr: Arc<MemoryManager>,
    llm: Arc<Box<dyn LlmClient>>,
    skills: Arc<RwLock<SkillRegistry>>,
    swarm_orch: Option<Arc<SwarmOrchestrator>>,
    idle_threshold: Duration,
    check_interval: Duration,
}

impl HeartbeatEngine {
    pub fn new(
        memory_mgr: Arc<MemoryManager>,
        llm: Arc<Box<dyn LlmClient>>,
        skills: Arc<RwLock<SkillRegistry>>,
    ) -> Self {
        Self {
            memory_mgr,
            llm,
            skills,
            swarm_orch: None,
            idle_threshold: Duration::from_secs(300), // 5 minutes
            check_interval: Duration::from_secs(60),  // 1 minute
        }
    }

    pub fn with_swarm(mut self, swarm_orch: Arc<SwarmOrchestrator>) -> Self {
        self.swarm_orch = Some(swarm_orch);
        self
    }

    pub fn with_thresholds(mut self, idle: Duration, interval: Duration) -> Self {
        self.idle_threshold = idle;
        self.check_interval = interval;
        self
    }

    pub async fn start(self: Arc<Self>, cancel_token: tokio_util::sync::CancellationToken) {
        info!("Starting Heartbeat Engine (Interval: {:?}, Idle Threshold: {:?})", 
            self.check_interval, self.idle_threshold);
            
        let engine = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(engine.check_interval);
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        debug!("Heartbeat tick");
                        
                        // 1. Check for User Idleness and Run Proactive Tasks
                        if engine.memory_mgr.is_idle(engine.idle_threshold).await {
                            debug!("User is idle, running enhanced background maintenance...");
                            if let Err(e) = engine.enhanced_background_think().await {
                                warn!("Failed to run enhanced background think: {}", e);
                            }
                            
                            // Run Draft Mode proactive refinement
                            if let Err(e) = engine.proactive_draft_refinement().await {
                                warn!("Draft refinement failed: {}", e);
                            }
                        }
                        
                        // 2. Periodic Memory Consolidation (always check)
                        if let Err(e) = engine.check_consolidation().await {
                            warn!("Failed to check memory consolidation: {}", e);
                        }
                    }
                    _ = cancel_token.cancelled() => {
                        info!("Heartbeat Engine shutting down...");
                        break;
                    }
                }
            }
        });
    }

    /// 增强的后台思考逻辑
    async fn enhanced_background_think(&self) -> Result<()> {
        // 1. 检查 Core Memory 使用率与压缩
        let core = self.memory_mgr.get_core_memory().await;
        if core.is_near_capacity() {
            info!("Core Memory is near capacity, triggering compression...");
            if let Err(e) = self.compress_core_memory().await {
                error!("Failed to compress Core Memory: {}", e);
            }
        }
        
        // 2. 预测性预加载 (Warmup)
        // 示例：预加载最近 5 个活跃会话
        let active_sessions = self.get_predicted_active_sessions().await;
        if !active_sessions.is_empty() {
            if let Err(e) = self.memory_mgr.warmup(&active_sessions).await {
                warn!("Failed to warmup predicted active sessions: {}", e);
            }
        }
        
        // 3. 记忆优先级重排与主动任务
        if let Err(e) = self.run_proactive_tasks().await {
             warn!("Proactive tasks failed: {}", e);
        }
        
        Ok(())
    }

    async fn compress_core_memory(&self) -> Result<()> {
        // 使用 LLM 压缩 Core Memory
        let core_prompt = self.memory_mgr.get_core_memory_prompt().await;
        let prompt = format!(
            "Summarize and compress this Core Memory, keeping only essential information. \
            Be concise and preserve key facts, user preferences, and persona guidelines:\n\n{}",
            core_prompt
        );
        
        let system_msg = Message {
            role: "system".to_string(),
            content: Some(vec![ContentPart::Text { 
                text: "You are a memory compression assistant. Your goal is to shrink memory content while preserving its essence.".to_string() 
            }]),
            ..Default::default()
        };
        
        let user_msg = Message {
            role: "user".to_string(),
            content: Some(vec![ContentPart::Text { text: prompt }]),
            ..Default::default()
        };
        
        let response = self.llm.chat_complete(&[system_msg, user_msg]).await?;
        let compressed = response;
        
        // 更新 Core Memory (这里简单地替换 Memory 块，实际可能需要更精细的处理)
        self.memory_mgr.core_memory_replace(
            CoreMemoryBlock::Memory,
            "",
            &compressed,
        ).await?;
        
        info!("Core Memory compressed successfully.");
        Ok(())
    }

    async fn get_predicted_active_sessions(&self) -> Vec<String> {
        // 简单实现：返回当前活跃会话中最近使用的前几个
        // 在更复杂的实现中，这里可以使用历史模式预测
        let mut sessions = Vec::new();
        for entry in self.memory_mgr.working_store.iter() {
            sessions.push(entry.key().clone());
        }
        
        // 限制预热数量
        sessions.truncate(5);
        sessions
    }

    async fn run_proactive_tasks(&self) -> Result<()> {
        let skills = self.skills.read().await;
        if skills.get_skill("proactive-agent").is_some() {
            debug!("Running proactive-agent skill...");
            // Execute the skill logic here
        }
        
        Ok(())
    }

    /// 主动草稿精炼逻辑
    async fn proactive_draft_refinement(&self) -> Result<()> {
        if let Some(orch) = &self.swarm_orch {
            let active_graphs = orch.coordinator.active_graphs.read().await;
            for (id, graph) in active_graphs.iter() {
                // 只针对草稿模式且已完成的任务进行主动优化
                if graph.goal.starts_with("Draft:") && graph.status == GraphStatus::Completed {
                    debug!("Checking draft swarm {} for proactive refinement...", id);
                    
                    // 随机触发深层次 polish (比如 10% 概率)
                    if rand::random::<f32>() < 0.1 {
                        info!("Triggering autonomous deep polish for draft: {}", graph.goal);
                        
                        // 创建新的 polish 任务
                        let polish_id = format!("proactive_polish_{}", Uuid::new_v4().to_string().chars().take(8).collect::<String>());
                        
                        let new_task = TaskNode {
                            id: polish_id.clone(),
                            agent_role: "drafter".to_string(),
                            prompt: format!("Perform an autonomous deep polish of the previous work for '{}'. Focus on enhancing the logical flow and professional tone.", graph.goal),
                            dependencies: vec![], // No dependencies as previous tasks are all done
                            status: TaskStatus::Pending,
                            result: None,
                            logs: Vec::new(),
                            priority: 64, // Lower priority for background tasks
                            timeout_ms: 120000,
                            max_retries: 2,
                            retry_count: 0,
                        };
                        
                        // 由于我们目前持有 read lock，无法直接修改 active_graphs
                        // 这里可以发送一个事件或者使用内部消息队列
                        // 为了简单起见，我们直接调用 orch.add_task_to_graph (它会获取自己的 write lock)
                        let id_clone = id.clone();
                        let orch_clone = orch.clone();
                        let goal_clone = graph.goal.clone();
                        
                        tokio::spawn(async move {
                            if let Err(e) = orch_clone.add_task_to_graph(&id_clone, new_task).await {
                                warn!("Failed to add proactive task to graph {}: {}", id_clone, e);
                            } else {
                                // 重新启动执行
                                let _ = orch_clone.execute_graph(TaskGraph::new(), &id_clone, &goal_clone).await;
                            }
                        });
                    }
                }
            }
        }
        Ok(())
    }

    async fn check_consolidation(&self) -> Result<()> {
        #[cfg(feature = "knowledge")]
        if let Some(_consolidator) = &self.memory_mgr.consolidator {
            debug!("Triggering periodic memory consolidation check...");
        }
        
        Ok(())
    }
}
