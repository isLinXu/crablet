use crablet::skills::openclaw::OpenClawSkillLoader;
use std::path::Path;

#[tokio::test]
async fn test_load_weather_skill() {
    let manifest_path = Path::new("../skills/weather/SKILL.md");
    if manifest_path.exists() {
        let skill = OpenClawSkillLoader::load(manifest_path).await;
        assert!(skill.is_ok());
        let loaded = skill.unwrap();
        assert_eq!(loaded.manifest.name, "weather");
        assert!(loaded.manifest.parameters.get("properties").is_some());
    }
}
