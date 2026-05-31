use anyhow::Result;
use async_trait::async_trait;
use crablet::plugins::Plugin;
use crablet::skills::{registry::SkillRegistry, SkillManifest, SkillTrigger, SkillType};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn test_skill_registry_load() {
    let mut registry = SkillRegistry::new();

    // We expect the loader to find some skills in the mock/actual directory
    // Even if it doesn't find any, the registry shouldn't panic
    let result = registry.load_from_dir("../skills").await;
    assert!(result.is_ok());

    // Check if we can get a list of registered skills
    let skills = registry.list_skills();
    println!("Loaded skills: {:?}", skills);
}

struct NoopPlugin;

#[async_trait]
impl Plugin for NoopPlugin {
    fn name(&self) -> &str {
        "intent_router"
    }

    fn description(&self) -> &str {
        "No-op plugin for trigger tests"
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, _args: serde_json::Value) -> Result<String> {
        Ok("noop".to_string())
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn test_skill_registry_builds_intent_trigger_engine() {
    let mut registry = SkillRegistry::new();
    let manifest = SkillManifest {
        name: "intent_router".to_string(),
        description: "Routes coding requests".to_string(),
        version: "1.0.0".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        }),
        entrypoint: "plugin".to_string(),
        env: Default::default(),
        requires: vec![],
        runtime: None,
        dependencies: None,
        resources: None,
        permissions: vec![],
        conflicts: vec![],
        min_crablet_version: None,
        author: None,
        triggers: vec![SkillTrigger::Intent {
            intent: "coding".to_string(),
            confidence_threshold: 0.7,
        }],
    };
    let plugin: Arc<Box<dyn Plugin>> = Arc::new(Box::new(NoopPlugin) as Box<dyn Plugin>);

    registry.insert_skill(
        "intent_router".to_string(),
        SkillType::Plugin(manifest, plugin),
    );

    let engine = registry.build_trigger_engine();
    let best = engine.match_best("Please implement a Rust function", 0.7);

    assert!(best.is_some());
    let matched = best.unwrap();
    assert_eq!(matched.skill_name, "intent_router");
    assert_eq!(matched.trigger_type, "intent");
}
