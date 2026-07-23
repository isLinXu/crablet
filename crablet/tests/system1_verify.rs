use crablet::cognitive::system1::System1;
use crablet::cognitive::CognitiveSystem;

#[tokio::test]
async fn verify_system1_regex() {
    let sys1 = System1::new();

    // Test Greeting (Regex: (hello|hi|hey|你好|您好).*)
    let (resp, _) = sys1.process("hello world", &[]).await.unwrap();
    assert!(resp.contains("Crablet") || resp.contains("小螃蟹"));

    // Test Case Insensitive
    let (resp, _) = sys1.process("Hi there", &[]).await.unwrap();
    assert!(resp.contains("Crablet") || resp.contains("小螃蟹"));

    // Test Help Command
    let (resp, _) = sys1.process("help", &[]).await.unwrap();
    assert!(resp.contains("命令") || resp.contains("帮助") || resp.contains("command"));

    // Test Status
    let (resp, _) = sys1.process("status", &[]).await.unwrap();
    assert!(resp.contains("在线") || resp.contains("正常"));

    // Test Time query (now supported in System1Enhanced)
    let (resp, _) = sys1.process("what time is it", &[]).await.unwrap();
    assert!(!resp.is_empty());

    // Test No Match
    let result = sys1.process("calculate the mass of sun", &[]).await;
    assert!(result.is_err());
}
