use crablet::cognitive::system1::System1;
use crablet::cognitive::CognitiveSystem;

#[tokio::test]
async fn test_system1_fuzzy_matching() {
    let sys1 = System1::new();

    // 1. Exact Match
    let (resp, _) = sys1
        .process("status", &[])
        .await
        .expect("Exact match failed");
    assert!(resp.contains("在线") || resp.contains("正常"), "Status response: {resp}");

    // 2. Fuzzy Match (1 char error)
    // "statu" vs "status" (delete 's') -> dist 1
    let (resp, _) = sys1
        .process("statu", &[])
        .await
        .expect("Fuzzy match 'statu' failed");
    assert!(resp.contains("在线") || resp.contains("正常"), "Fuzzy response: {resp}");

    // "statuss" (insert 's') -> dist 1
    let (resp, _) = sys1
        .process("statuss", &[])
        .await
        .expect("Fuzzy match 'statuss' failed");
    assert!(resp.contains("在线") || resp.contains("正常"), "Fuzzy response: {resp}");

    // 3. Non-matching input should fail
    let result = sys1.process("xyzzy_unknown_command_12345", &[]).await;
    assert!(result.is_err(), "Should not match random input");

    // 4. Help command fuzzy match
    // "helpp" (len 5) -> threshold 1. Dist 1. Matches.
    let (resp, _) = sys1
        .process("helpp", &[])
        .await
        .expect("Fuzzy match 'helpp' failed");
    assert!(resp.contains("命令") || resp.contains("command") || resp.contains("帮助"),
        "Help response: {resp}");

    // Completely unrelated input should fail
    let result = sys1.process("qqqqq", &[]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_system1_aliases() {
    let sys1 = System1::new();

    // "stats" is alias for "status"
    let (resp, _) = sys1
        .process("stats", &[])
        .await
        .expect("Alias 'stats' failed");
    assert!(resp.contains("在线") || resp.contains("正常"), "Stats response: {resp}");

    // "state" is also a status alias
    let (resp, _) = sys1
        .process("state", &[])
        .await
        .expect("Alias 'state' failed");
    assert!(resp.contains("在线") || resp.contains("正常"), "State response: {resp}");
}
