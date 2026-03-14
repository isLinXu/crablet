//! 可观测性系统完整演示
//!
//! 此示例展示如何使用 ObservableReActEngine 进行完整的 Agent 执行追踪和调试
//!
//! 运行方式: cargo run --example observability_demo --features web

use crablet::cognitive::react_observable::ObservableReActEngine;
use crablet::cognitive::llm::{LlmClient, MockClient};
use crablet::skills::SkillRegistry;
use crablet::events::EventBus;
use crablet::observability::{ObservabilityManager, InMemoryStorage, Breakpoint, BreakpointCondition, BreakpointAction};
use crablet::types::Message;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    println!("=== Crablet 可观测性系统演示 ===\n");

    // 1. 初始化组件
    println!("1. 初始化系统组件...");
    
    let event_bus = Arc::new(EventBus::new());
    let llm: Arc<Box<dyn LlmClient>> = Arc::new(Box::new(MockClient));
    let skills = Arc::new(RwLock::new(SkillRegistry::new()));
    
    // 创建可观测性管理器
    let storage = Arc::new(InMemoryStorage::new());
    let observability = ObservabilityManager::new(storage);
    let tracer = observability.tracer();
    let breakpoint_manager = observability.breakpoint_manager();
    
    println!("   ✓ 组件初始化完成");

    // 2. 创建 ObservableReActEngine
    println!("\n2. 创建 ObservableReActEngine...");
    
    let engine = ObservableReActEngine::new(
        llm,
        skills,
        event_bus,
        tracer,
        breakpoint_manager.clone(),
    );
    
    println!("   ✓ ObservableReActEngine 创建完成");

    // 3. 设置断点
    println!("\n3. 设置调试断点...");
    
    // 在第二步暂停执行
    let step_breakpoint = Breakpoint::new(BreakpointCondition::AfterIteration { count: 2 })
        .with_name("Pause at step 2")
        .with_action(BreakpointAction::Pause)
        .with_timeout(60);
    
    let bp_id = breakpoint_manager.read().await.set_breakpoint(step_breakpoint).await;
    println!("   ✓ 设置步骤断点 (ID: {})", bp_id);

    // 4. 启动执行追踪
    println!("\n4. 启动执行追踪...");
    
    let execution_id = "demo-execution-001".to_string();
    let workflow_id = "test-workflow".to_string();
    
    observability.start_session(execution_id.clone(), workflow_id).await;
    println!("   ✓ 追踪会话已启动 (Execution ID: {})", execution_id);

    // 5. 模拟执行
    println!("\n5. 模拟 Agent 执行...");
    
    let context = vec![
        Message::new("system", "你是一个 helpful assistant"),
        Message::new("user", "请帮我搜索关于 Rust 编程语言的信息"),
    ];

    // 模拟执行过程
    match engine.execute(&execution_id, &context, 5).await {
        Ok((response, traces)) => {
            println!("   ✓ 执行完成!");
            println!("   响应: {}", response);
            println!("   追踪步骤数: {}", traces.len());
        }
        Err(e) => {
            println!("   ✗ 执行失败: {}", e);
        }
    }

    // 6. 查看追踪数据
    println!("\n6. 查看追踪数据...");
    
    let tracer_read = tracer.read().await;
    if let Some(spans) = tracer_read.get_spans(&execution_id).await {
        println!("   追踪跨度数量: {}", spans.len());
        
        for (i, span) in spans.iter().enumerate() {
            match span {
                crablet::observability::AgentSpan::Thought { content, .. } => {
                    println!("   [{}] 💭 Thought: {}", i, content.chars().take(50).collect::<String>());
                }
                crablet::observability::AgentSpan::Action { tool, .. } => {
                    println!("   [{}] ⚡ Action: {}", i, tool);
                }
                crablet::observability::AgentSpan::Observation { result, .. } => {
                    println!("   [{}] 👁️  Observation: {}", i, result.chars().take(50).collect::<String>());
                }
                crablet::observability::AgentSpan::Error { error, .. } => {
                    println!("   [{}] ❌ Error: {}", i, error);
                }
                _ => {}
            }
        }
    }

    // 7. 演示断点管理
    println!("\n7. 断点管理...");
    
    let breakpoints = breakpoint_manager.read().await.list_breakpoints().await;
    println!("   当前断点数量: {}", breakpoints.len());
    
    for (id, bp) in breakpoints {
        println!("   - {}: {:?}", id, bp.name);
    }

    // 8. 清理
    println!("\n8. 清理...");
    breakpoint_manager.read().await.remove_breakpoint(&bp_id).await;
    println!("   ✓ 断点已移除");

    println!("\n=== 演示完成 ===");
    println!("\n提示:");
    println!("  - 访问 http://localhost:8080/observability 查看可视化追踪界面");
    println!("  - 使用 WebSocket 连接 ws://localhost:8080/ws/observability 接收实时事件");
}
