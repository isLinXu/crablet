use crablet::skills::registry::SkillRegistry;
use crablet::skills::openclaw::OpenClawSkillLoader;

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
