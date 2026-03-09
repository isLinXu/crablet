#[cfg(feature = "knowledge")]
use crablet::knowledge::chunking::{Chunker, RecursiveCharacterChunker};
use crablet::gateway::events::{EventBus, GatewayEvent};
use crablet::agent::swarm::SwarmMessage;
use crablet::skills::openclaw::OpenClawSkillLoader;
use tokio::fs;

#[cfg(feature = "knowledge")]
#[tokio::test]
async fn verify_chunking_logic() {
    let chunker = RecursiveCharacterChunker::new(50, 10);
    let text = "This is a long sentence that should be split into multiple chunks because it exceeds the limit of 50 characters.";
    let chunks = chunker.chunk(text).unwrap();
    
    assert!(!chunks.is_empty(), "Should produce chunks");
    for chunk in &chunks {
        assert!(chunk.content.len() <= 50, "Chunk size should be <= 50, got {}", chunk.content.len());
    }
    
    // Check overlap (roughly)
    if chunks.len() > 1 {
        // Just verify we have multiple chunks
        assert!(chunks.len() >= 2);
    }
}

#[tokio::test]
async fn verify_event_bus() {
    let bus = EventBus::new(100);
    let mut rx = bus.subscribe();
    
    let event = GatewayEvent::SystemAlert("Test Alert".to_string());
    bus.publish(event).unwrap();
    
    let received = rx.recv().await.unwrap();
    match received {
        GatewayEvent::SystemAlert(msg) => assert_eq!(msg, "Test Alert"),
        _ => panic!("Wrong event type received"),
    }
}

#[test]
fn verify_swarm_message_serialization() {
    let msg = SwarmMessage::Task {
        task_id: "task-1".to_string(),
        description: "Hello".to_string(),
        context: vec![],
        payload: None,
    };
    
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: SwarmMessage = serde_json::from_str(&json).unwrap();
    
    if let SwarmMessage::Task { task_id, description, .. } = deserialized {
        assert_eq!(task_id, "task-1");
        assert_eq!(description, "Hello");
    } else {
        panic!("Wrong variant");
    }
}

#[tokio::test]
async fn verify_openclaw_skill_loader() {
    // Create a temporary SKILL.md
    let temp_dir = std::env::temp_dir().join("crablet_test_skill");
    fs::create_dir_all(&temp_dir).await.unwrap();
    let skill_path = temp_dir.join("SKILL.md");
    
    let content = r#"---
name: test-skill
description: A test skill for verification
metadata:
  openclaw:
    requires:
      bins: ["python"]
      env: ["API_KEY"]
---
Here are the instructions for the skill.
Do verify this content is loaded.
"#;

    fs::write(&skill_path, content).await.unwrap();
    
    // Test Load
    let skill = OpenClawSkillLoader::load(&skill_path).await.unwrap();
    assert_eq!(skill.manifest.name, "test-skill");
    assert_eq!(skill.manifest.description, "A test skill for verification");
    // In src/skills/openclaw.rs we set entrypoint to "openclaw"
    assert_eq!(skill.manifest.entrypoint, "openclaw");
    
    // Test Instruction Extraction
    let instruction = OpenClawSkillLoader::get_instruction(&skill_path).await.unwrap();
    assert!(instruction.contains("Here are the instructions"));
    
    // Cleanup
    let _ = fs::remove_dir_all(temp_dir).await;
}
