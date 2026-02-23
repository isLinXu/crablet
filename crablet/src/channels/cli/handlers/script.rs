use anyhow::Result;
use tracing::info;
use crate::scripting::engine::LuaEngine;

pub async fn handle_run_script(path: &str) -> Result<()> {
    info!("Running Lua script: {}", path);
    let script = std::fs::read_to_string(path)?;
    
    let engine = match LuaEngine::new() {
        Ok(e) => e,
        Err(e) => return Err(anyhow::anyhow!("Lua init error: {}", e)),
    };

    let result = match engine.execute(&script).await {
        Ok(r) => r,
        Err(e) => return Err(anyhow::anyhow!("Lua execution error: {}", e)),
    };
    
    println!("Script Output: {}", result);
    Ok(())
}
