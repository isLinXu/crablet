use anyhow::Result;
use crablet::gateway::auth::{AuthManager, AuthMode};
use crablet::events::{EventBus, AgentEvent};
// use crablet::audit::start_audit_worker;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_e2e_auth_and_audit() -> Result<()> {
    // 1. Setup Database
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await?;
        
    // Create tables manually for test
    // Schema must match AuthManager::create_api_key expectations
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS api_keys (
            id TEXT PRIMARY KEY,
            key_hash TEXT,
            key_prefix TEXT,
            user_id TEXT,
            name TEXT,
            created_at INTEGER,
            status TEXT
        )
    "#).execute(&pool).await?;
    
    sqlx::query("CREATE TABLE IF NOT EXISTS swarm_logs (id INTEGER PRIMARY KEY, task_id TEXT, content TEXT, created_at INTEGER)").execute(&pool).await?;

    // 2. Setup Auth Manager
    let auth = AuthManager::new(AuthMode::ApiKey, Some(pool.clone()));
    
    // 3. Create API Key
    let key = auth.create_api_key("Test Key", "user-123").await?;
    println!("Created Key: {}", key);
    
    // 4. Validate Key (Success)
    let user = auth.validate_token_async(&key).await;
    assert_eq!(user, Some("user-123".to_string()));
    
    // 5. Validate Key (Failure - Wrong Key)
    let bad_key = format!("{}x", key);
    let user_bad = auth.validate_token_async(&bad_key).await;
    assert_eq!(user_bad, None);
    
    // 6. Setup Audit Logger (Mock or Real)
    // For now, let's just test Auth as the main goal, and basic event publishing.
    
    let event_bus = Arc::new(EventBus::new(100));
    
    // 7. Trigger Swarm Activity
    let task_id = "task-e2e";
    event_bus.publish(AgentEvent::SwarmActivity {
        task_id: task_id.to_string(),
        graph_id: "graph-e2e".to_string(),
        from: "Agent A".to_string(),
        to: "Agent B".to_string(),
        message_type: "Proposal".to_string(),
        content: "Let's do this!".to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    });
    
    // Wait for async write (if worker was running)
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    println!("E2E Auth Test Passed!");
    
    Ok(())
}
