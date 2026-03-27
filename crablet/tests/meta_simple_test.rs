//! Simple Meta-Cognitive System Tests

use crablet::cognitive::{
    MetaCognitiveController, MetaConfig, ExecutionRequest, ExecutionResult,
    create_llm_client,
};
use crablet::config::Config;
use crablet::cognitive::meta_controller::{
    monitor::ExecutionMetrics,
};
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_meta_controller_creation() {
    let config = Config::default();
    let llm = create_llm_client(&config).await.unwrap();
    let controller = MetaCognitiveController::new(llm).await;
    
    assert!(controller.is_ok(), "Failed to create meta controller");
}

#[tokio::test]
async fn test_execute_simple_task() {
    let config = Config::default();
    let llm = create_llm_client(&config).await.unwrap();
    let controller = MetaCognitiveController::new(llm).await.unwrap();
    
    let request = ExecutionRequest {
        task_id: "test-1".into(),
        task: "Simple test task".into(),
        context: vec![],
        start_time: Instant::now(),
    };
    
    let result = controller.execute_with_meta(request, |req| {
        ExecutionResult {
            task_id: req.task_id.clone(),
            success: true,
            output: "Test output".into(),
            confidence: 0.9,
            duration: Duration::from_millis(100),
            metrics: ExecutionMetrics::default(),
        }
    }).await;
    
    assert!(result.success);
    assert_eq!(result.task_id, "test-1");
}

#[tokio::test]
async fn test_get_statistics() {
    let config = Config::default();
    let llm = create_llm_client(&config).await.unwrap();
    let controller = MetaCognitiveController::new(llm).await.unwrap();
    
    let stats = controller.get_statistics().await;
    assert_eq!(stats.total_tasks, 0);
    assert_eq!(stats.successful_tasks, 0);
}

#[tokio::test]
async fn test_custom_config() {
    let config = Config::default();
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
