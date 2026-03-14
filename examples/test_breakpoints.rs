//! 断点系统测试示例
//!
//! 运行方式: cargo run --example test_breakpoints

use crablet::observability::{
    BreakpointManager, Breakpoint, BreakpointCondition, BreakpointAction,
    ExecutionContext, ObservabilityManager, InMemoryStorage,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    println!("=== Crablet 断点系统测试 ===\n");

    // 创建可观测性管理器
    let storage = Arc::new(InMemoryStorage::new());
    let observability = ObservabilityManager::new(storage);
    let breakpoint_manager = observability.breakpoint_manager();

    // 测试 1: 设置断点
    println!("测试 1: 设置不同类型的断点");
    
    // 断点 1: 在特定迭代次数后触发
    let bp1 = Breakpoint::new(BreakpointCondition::AfterIteration { count: 3 })
        .with_name("After 3 iterations")
        .with_action(BreakpointAction::Pause);
    let bp1_id = breakpoint_manager.read().await.set_breakpoint(bp1).await;
    println!("  ✓ 设置迭代断点 (ID: {})", bp1_id);

    // 断点 2: 当思考包含特定文本时触发
    let bp2 = Breakpoint::new(BreakpointCondition::ThoughtContains { 
        text: "error".to_string() 
    })
    .with_name("Error detected in thought")
    .with_action(BreakpointAction::InjectHint { 
        hint: "请检查错误并修正".to_string() 
    });
    let bp2_id = breakpoint_manager.read().await.set_breakpoint(bp2).await;
    println!("  ✓ 设置文本匹配断点 (ID: {})", bp2_id);

    // 断点 3: 在工具调用前触发
    let bp3 = Breakpoint::new(BreakpointCondition::BeforeToolCall { 
        tool_pattern: Some("search.*".to_string()) 
    })
    .with_name("Before search tool")
    .with_action(BreakpointAction::Continue); // 自动继续
    let bp3_id = breakpoint_manager.read().await.set_breakpoint(bp3).await;
    println!("  ✓ 设置工具调用断点 (ID: {})", bp3_id);

    // 测试 2: 模拟执行并检查断点
    println!("\n测试 2: 模拟执行流程");
    
    for step in 1..=5 {
        println!("\n--- 步骤 {} ---", step);
        
        let context = ExecutionContext {
            execution_id: "test-execution-001".to_string(),
            step_number: step,
            current_thought: if step == 4 { 
                Some("I found an error in the code".to_string()) 
            } else { 
                Some(format!("Thinking at step {}", step)) 
            },
            current_action: if step == 2 {
                Some("search_web".to_string())
            } else {
                None
            },
            variables: HashMap::new(),
        };

        // 检查断点
        let bp_manager = breakpoint_manager.read().await;
        match bp_manager.check_breakpoint(&context).await {
            Some(BreakpointAction::Pause) => {
                println!("  ⏸️  执行暂停! 等待人工介入...");
                // 模拟人工介入
                sleep(Duration::from_millis(500)).await;
                println!("  ▶️  继续执行");
            }
            Some(BreakpointAction::InjectHint { hint }) => {
                println!("  💡 注入提示: {}", hint);
            }
            Some(BreakpointAction::Continue) => {
                println!("  ⏩ 自动继续");
            }
            Some(BreakpointAction::Abort { reason }) => {
                println!("  🛑 执行中止: {}", reason);
                break;
            }
            None => {
                println!("  ✓ 无断点触发");
            }
            _ => {}
        }
    }

    // 测试 3: 列出所有断点
    println!("\n测试 3: 列出所有断点");
    let breakpoints = breakpoint_manager.read().await.list_breakpoints().await;
    for (id, bp) in breakpoints {
        println!("  - {}: {:?}", id, bp.name);
    }

    // 测试 4: 删除断点
    println!("\n测试 4: 删除断点");
    let removed = breakpoint_manager.read().await.remove_breakpoint(&bp1_id).await;
    println!("  ✓ 删除断点 {}: {}", bp1_id, removed);

    // 测试 5: 复合条件断点
    println!("\n测试 5: 复合条件断点");
    let compound_bp = Breakpoint::new(BreakpointCondition::All(vec![
        BreakpointCondition::AfterIteration { count: 2 },
        BreakpointCondition::ThoughtContains { text: "important".to_string() },
    ]))
    .with_name("Compound condition")
    .with_action(BreakpointAction::ModifyContext {
        variable_updates: {
            let mut map = HashMap::new();
            map.insert("priority".to_string(), serde_json::json!("high"));
            map
        }
    });
    let compound_id = breakpoint_manager.read().await.set_breakpoint(compound_bp).await;
    println!("  ✓ 设置复合条件断点 (ID: {})", compound_id);

    // 验证复合条件
    let test_context = ExecutionContext {
        execution_id: "test-002".to_string(),
        step_number: 3,
        current_thought: Some("This is important".to_string()),
        current_action: None,
        variables: HashMap::new(),
    };

    let bp_manager = breakpoint_manager.read().await;
    match bp_manager.check_breakpoint(&test_context).await {
        Some(BreakpointAction::ModifyContext { variable_updates }) => {
            println!("  ✓ 复合条件触发，变量更新: {:?}", variable_updates);
        }
        _ => {
            println!("  ✗ 复合条件未触发");
        }
    }

    println!("\n=== 测试完成 ===");
}
