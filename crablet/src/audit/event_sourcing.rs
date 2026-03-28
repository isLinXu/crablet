//! Event Sourcing Audit System for Crablet
//!
//! 完整记录所有 Agent 决策和执行过程，支持回溯和重放

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    // Agent Events
    AgentCreated { agent_id: String, role: String },
    AgentExecuted { agent_id: String, task: String, result: String },
    AgentFailed { agent_id: String, error: String },
    ToolCalled { agent_id: String, tool: String, args: serde_json::Value },
    ToolResult { tool: String, result: String },
    
    // Cognitive Events
    SystemSelected { system: String, reason: String },
    ComplexityAnalyzed { complexity: f32, features: Vec<String> },
    
    // Memory Events
    MemoryStored { memory_type: String, key: String },
    MemoryRetrieved { memory_type: String, key: String },
    MemoryPruned { count: usize },
    
    // Skill Events
    SkillActivated { skill: String, trigger: String },
    SkillExecuted { skill: String, duration_ms: u64 },
    
    // Gateway Events
    RequestReceived { method: String, path: String },
    ResponseSent { status: u16, duration_ms: u64 },
    WebSocketConnected { session_id: String },
    WebSocketMessage { session_id: String, direction: String },
}

/// 领域事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent {
    pub id: Uuid,
    pub event_type: EventType,
    pub aggregate_id: String,      // 聚合根 ID
    pub aggregate_type: String,    // 聚合类型 (Agent/Session/Skill)
    pub timestamp: DateTime<Utc>,
    pub metadata: serde_json::Value,
    pub previous_state: Option<serde_json::Value>,
    pub new_state: serde_json::Value,
}

/// 事件存储
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStore {
    pub events: Vec<DomainEvent>,
    pub snapshot_interval: usize,  // 快照间隔
}

/// 审计事件溯源引擎
pub struct AuditEventSourcing {
    event_store: Vec<DomainEvent>,
    snapshots: std::collections::HashMap<String, serde_json::Value>,
}

impl AuditEventSourcing {
    pub fn new() -> Self {
        Self {
            event_store: Vec::new(),
            snapshots: std::collections::HashMap::new(),
        }
    }
    
    /// 记录事件
    pub fn record(&mut self, event: DomainEvent) {
        // 存储事件
        self.event_store.push(event.clone());
        
        // 更新聚合状态快照
        self.snapshots.insert(
            format!("{}:{}", event.aggregate_type, event.aggregate_id),
            event.new_state,
        );
    }
    
    /// 获取聚合的所有事件 (用于重放)
    pub fn get_events(&self, aggregate_id: &str, aggregate_type: &str) -> Vec<&DomainEvent> {
        self.event_store.iter()
            .filter(|e| e.aggregate_id == aggregate_id && e.aggregate_type == aggregate_type)
            .collect()
    }
    
    /// 获取当前状态 (从快照或重放)
    pub fn get_state(&self, aggregate_id: &str, aggregate_type: &str) -> Option<serde_json::Value> {
        let key = format!("{}:{}", aggregate_type, aggregate_id);
        self.snapshots.get(&key).cloned()
    }
    
    /// 回放到指定时间点
    pub fn replay_to(&self, aggregate_id: &str, aggregate_type: &str, timestamp: DateTime<Utc>) -> Vec<&DomainEvent> {
        self.event_store.iter()
            .filter(|e| {
                e.aggregate_id == aggregate_id 
                && e.aggregate_type == aggregate_type
                && e.timestamp <= timestamp
            })
            .collect()
    }
}

impl Default for AuditEventSourcing {
    fn default() -> Self {
        Self::new()
    }
}

/// 审计查询 API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditQuery {
    pub aggregate_id: Option<String>,
    pub aggregate_type: Option<String>,
    pub event_types: Vec<EventType>,
    pub from_time: Option<DateTime<Utc>>,
    pub to_time: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

impl AuditEventSourcing {
    pub fn query(&self, query: AuditQuery) -> Vec<&DomainEvent> {
        self.event_store.iter()
            .filter(|e| {
                let type_match = query.aggregate_type.as_ref()
                    .map(|t| &e.aggregate_type == t)
                    .unwrap_or(true);
                
                let id_match = query.aggregate_id.as_ref()
                    .map(|id| &e.aggregate_id == id)
                    .unwrap_or(true);
                
                let event_match = query.event_types.is_empty() 
                    || query.event_types.iter().any(|et| std::mem::discriminant(et) == std::mem::discriminant(&e.event_type));
                
                let time_match = query.from_time.map_or(true, |f| e.timestamp >= f)
                    && query.to_time.map_or(true, |t| e.timestamp <= t);
                
                type_match && id_match && event_match && time_match
            })
            .take(query.limit.unwrap_or(1000))
            .collect()
    }
}

/// 审计报告生成
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub total_events: usize,
    pub events_by_type: std::collections::HashMap<String, usize>,
    pub events_by_aggregate: std::collections::HashMap<String, usize>,
    pub timeline: Vec<serde_json::Value>,
    pub errors: Vec<String>,
}

impl AuditEventSourcing {
    pub fn generate_report(&self, query: AuditQuery) -> AuditReport {
        let events = self.query(query);
        
        let mut events_by_type = std::collections::HashMap::new();
        let mut events_by_aggregate = std::collections::HashMap::new();
        let mut errors = Vec::new();
        
        for e in &events {
            let type_name = format!("{:?}", e.event_type);
            *events_by_type.entry(type_name).or_insert(0) += 1;
            *events_by_aggregate.entry(e.aggregate_id.clone()).or_insert(0) += 1;
            
            if let EventType::AgentFailed { error, .. } = &e.event_type {
                errors.push(error.clone());
            }
        }
        
        let timeline: Vec<serde_json::Value> = events.iter().map(|e| {
            serde_json::json!({
                "timestamp": e.timestamp,
                "type": format!("{:?}", e.event_type),
                "aggregate_id": e.aggregate_id,
                "metadata": e.metadata
            })
        }).collect();
        
        AuditReport {
            total_events: events.len(),
            events_by_type,
            events_by_aggregate,
            timeline,
            errors,
        }
    }
}