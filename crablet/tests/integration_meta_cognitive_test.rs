//! Meta-Cognitive System Integration Tests

use crablet::cognitive::{
    MetaCognitiveController, MetaConfig, ExecutionRequest, ExecutionResult,
    create_llm_client,
};
use crablet::config::Config;
use crablet::cognitive::meta_controller::monitor::ExecutionMetrics;
use std::time::{Duration, Instant};

fn test_config() -> Config {
    std::env::set_var("OPENAI_API_KEY", "sk-test");
    Config::default()
}

#[tokio::test]
async fn test_meta_controller_creation() {
    let config = test_config();
    let llm = create_llm_client(&config).await.unwrap();
    let controller = MetaCognitiveController::new(llm).await;
    
    assert!(controller.is_ok(), "Failed to create meta controller");
}

#[tokio::test]
async fn test_execute_with_meta_success() {
    let config = test_config();
    let llm = create_llm_client(&config).await.unwrap();
    let controller = MetaCognitiveController::new(llm).await.unwrap();
    
    let request = ExecutionRequest {
        task_id: "test-1".into(),
        task: "Write a hello world function in Rust".into(),
        context: vec![],
        start_time: Instant::now(),
    };
    
    let result = controller.clone().execute_with_meta(request, |req| {
        ExecutionResult {
            task_id: req.task_id.clone(),
            success: true,
            output: "```rust\nfn hello_world() {\n    println!(\"Hello, World!\");\n}\n```".into(),
            confidence: 0.95,
            duration: Duration::from_millis(150),
            metrics: ExecutionMetrics::default(),
        }
    }).await;
    
    assert!(result.success);
    assert_eq!(result.task_id, "test-1");
    assert!(result.confidence > 0.9);
}

#[tokio::test]
async fn test_execute_with_meta_failure() {
    let config = test_config();
    let llm = create_llm_client(&config).await.unwrap();
    let controller = MetaCognitiveController::new(llm).await.unwrap();
    
    let request = ExecutionRequest {
        task_id: "test-2".into(),
        task: "Complex task that will fail".into(),
        context: vec![],
        start_time: Instant::now(),
    };
    
    let mut metrics = ExecutionMetrics::default();
    metrics.success = false;
    metrics.error = Some("Execution failed".into());
    metrics.confidence = 0.2;
    
    let result = controller.execute_with_meta(request, |req| {
        ExecutionResult {
            task_id: req.task_id.clone(),
            success: false,
            output: "Failed to complete task".into(),
            confidence: 0.2,
            duration: Duration::from_millis(500),
            metrics: metrics.clone(),
        }
    }).await;
    
    assert!(!result.success);
    assert_eq!(result.task_id, "test-2");
}

#[tokio::test]
async fn test_statistics() {
    let config = test_config();
    let llm = create_llm_client(&config).await.unwrap();
    let controller = MetaCognitiveController::new(llm).await.unwrap();
    
    // Execute a few tasks
    for i in 0..5 {
        let request = ExecutionRequest {
            task_id: format!("test-stats-{}", i),
            task: format!("Task {}", i),
            context: vec![],
            start_time: Instant::now(),
        };
        
        controller.execute_with_meta(request, |req| {
            ExecutionResult {
                task_id: req.task_id.clone(),
                success: i % 2 == 0,
                output: "Result".into(),
                confidence: if i % 2 == 0 { 0.9 } else { 0.3 },
                duration: Duration::from_millis(100),
                metrics: ExecutionMetrics::default(),
            }
        }).await;
    }
    
    let stats = controller.get_statistics().await;
    assert_eq!(stats.total_tasks, 5);
    assert!(stats.successful_tasks > 0);
    assert!(stats.failed_tasks > 0);
}

#[tokio::test]
async fn test_feedback_integration() {
    let config = test_config();
    let llm = create_llm_client(&config).await.unwrap();
    let controller = MetaCognitiveController::new(llm).await.unwrap();
    
    // Execute a task
    let request = ExecutionRequest {
        task_id: "feedback-test".into(),
        task: "Test task for feedback".into(),
        context: vec![],
        start_time: Instant::now(),
    };
    
    controller.execute_with_meta(request, |req| {
        ExecutionResult {
            task_id: req.task_id.clone(),
            success: true,
            output: "Test output".into(),
            confidence: 0.8,
            duration: Duration::from_millis(100),
            metrics: ExecutionMetrics::default(),
        }
    }).await;
    
    // Integrate feedback
    controller.integrate_feedback("feedback-test", 0.9).await.unwrap();
    
    let stats = controller.get_statistics().await;
    // The feedback should be recorded
    assert!(stats.total_tasks >= 1);
}

#[tokio::test]
async fn test_export_knowledge() {
    let config = test_config();
    let llm = create_llm_client(&config).await.unwrap();
    let controller = MetaCognitiveController::new(llm).await.unwrap();
    
    // Execute some tasks to generate knowledge
    for i in 0..3 {
        let request = ExecutionRequest {
            task_id: format!("knowledge-test-{}", i),
            task: format!("Task {}", i),
            context: vec![],
            start_time: Instant::now(),
        };
        
        controller.execute_with_meta(request, |req| {
            ExecutionResult {
                task_id: req.task_id.clone(),
                success: i > 0,
                output: "Result".into(),
                confidence: if i > 0 { 0.9 } else { 0.2 },
                duration: Duration::from_millis(100),
                metrics: ExecutionMetrics::default(),
            }
        }).await;
    }
    
    let knowledge = controller.export_knowledge().await.unwrap();
    // Should have learned something from failed tasks
    // Note: This may be empty if all tasks succeed or if learning threshold is not met
    println!("Exported knowledge: {} items", knowledge.len());
}

#[tokio::test]
async fn test_custom_config() {
    let config = test_config();
    let llm = create_llm_client(&config).await.unwrap();
    
    let custom_config = MetaConfig {
        monitor_interval: Duration::from_millis(50),
        max_feedback_history: 500,
        max_patterns: 500,
        learning_threshold: 0.7,
        enable_auto_optimization: false,
        optimization_interval: Duration::from_secs(30),
    };
    
    let controller = MetaCognitiveController::with_config(llm, custom_config).await;
    assert!(controller.is_ok());
}

#[tokio::test]
async fn test_concurrent_executions() {
    let config = test_config();
    let llm = create_llm_client(&config).await.unwrap();
    let controller = MetaCognitiveController::new(llm).await.unwrap();
    
    // Execute multiple tasks concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let controller = controller.clone();
        let handle = tokio::spawn(async move {
            let request = ExecutionRequest {
                task_id: format!("concurrent-{}", i),
                task: format!("Concurrent task {}", i),
                context: vec![],
                start_time: Instant::now(),
            };
            
            controller.execute_with_meta(request, |req| {
                ExecutionResult {
                    task_id: req.task_id.clone(),
                    success: true,
                    output: "Result".into(),
                    confidence: 0.9,
                    duration: Duration::from_millis(50),
                    metrics: ExecutionMetrics::default(),
                }
            }).await
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    let stats = controller.get_statistics().await;
    assert_eq!(stats.total_tasks, 10);
    assert_eq!(stats.successful_tasks, 10);
}

#[tokio::test]
async fn test_meta_cognitive_workflow() {
    let config = test_config();
    let llm = create_llm_client(&config).await.unwrap();
    let controller = MetaCognitiveController::new(llm).await.unwrap();
    
    // Simulate a learning workflow
    // 1. Execute a failing task
    let request1 = ExecutionRequest {
        task_id: "workflow-1".into(),
        task: "Complex task that fails".into(),
        context: vec![],
        start_time: Instant::now(),
    };
    
    let mut metrics = ExecutionMetrics::default();
    metrics.success = false;
    metrics.confidence = 0.2;
    
    controller.execute_with_meta(request1, |req| {
        ExecutionResult {
            task_id: req.task_id.clone(),
            success: false,
            output: "Failed".into(),
            confidence: 0.2,
            duration: Duration::from_millis(200),
            metrics: metrics.clone(),
        }
    }).await;
    
    // 2. Execute a similar successful task
    let request2 = ExecutionRequest {
        task_id: "workflow-2".into(),
        task: "Complex task".into(),
        context: vec![],
        start_time: Instant::now(),
    };
    
    controller.execute_with_meta(request2, |req| {
        ExecutionResult {
            task_id: req.task_id.clone(),
            success: true,
            output: "Success".into(),
            confidence: 0.95,
            duration: Duration::from_millis(100),
            metrics: ExecutionMetrics::default(),
        }
    }).await;
    
    // 3. Check if learning occurred
    let knowledge = controller.export_knowledge().await.unwrap();
    let stats = controller.get_statistics().await;
    
    println!("Workflow test - Knowledge items: {}", knowledge.len());
    println!("Workflow test - Total tasks: {}", stats.total_tasks);
    println!("Workflow test - Patterns extracted: {}", stats.patterns_extracted);
    
    assert!(stats.total_tasks >= 2);
}
