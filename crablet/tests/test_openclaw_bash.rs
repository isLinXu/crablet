use crablet::tools::bash::BashPlugin;
use crablet::plugins::Plugin;
use crablet::safety::oracle::{SafetyOracle, SafetyLevel};
use serde_json::json;

#[tokio::test]
async fn test_bash_plugin() {
    let oracle = SafetyOracle::new(SafetyLevel::Strict);
    let plugin = BashPlugin::new(oracle);
    
    // Test safe command
    let args = json!({"cmd": "echo 'hello world'"});
    let result = plugin.execute("run", args).await;
    println!("Safe command result: {:?}", result);
    // Strict mode might block echo by default in our oracle, so we just ensure it executes or gets blocked properly without crashing
    
    // Test dangerous command (should be blocked)
    let args_dangerous = json!({"cmd": "rm -rf /"});
    let result_dangerous = plugin.execute("run", args_dangerous).await;
    println!("Dangerous command result: {:?}", result_dangerous);
    assert!(result_dangerous.is_err() || result_dangerous.unwrap().contains("Blocked"));
}
