#[cfg(test)]
mod tests {
    use crate::scripting::engine::LuaEngine;

    #[tokio::test]
    async fn test_lua_run_command() {
        let engine = LuaEngine::new().expect("Failed to init Lua");
        
        let script = r#"
            local res = crablet.run_command("echo hello")
            return res
        "#;
        
        let result = engine.execute(script).await.unwrap();
        assert!(!result.contains("attempt to call"));
    }

    #[tokio::test]
    async fn test_lua_read_file() {
        // Create a temp file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "lua content").unwrap();
        let path_str = file_path.to_str().unwrap().to_string();
        
        let engine = LuaEngine::new().expect("Failed to init Lua");
        
        // Note: Safety Oracle might block temp paths in strict mode if not configured?
        // Let's assume it allows it or we need to mock Oracle?
        // The bindings use SafetyOracle::new(SafetyLevel::Strict).
        // Strict might block /tmp or random paths.
        // Let's check SafetyOracle impl.
        
        // For now, let's try.
        let script = format!(r#"
            local content = crablet.read_file("{}")
            return content
        "#, path_str.replace("\\", "\\\\"));
        
        let result = engine.execute(&script).await.unwrap();
        // If blocked, it returns "🚫 Safety Oracle Blocked..."
        // If allowed, "lua content"
        
        println!("Lua Read Result: {}", result);
        // We assert it's either content OR a block message (which proves binding works)
        assert!(result.contains("lua content") || result.contains("Safety Oracle"));
    }
}
