use crablet::cognitive::system1::System1;
use crablet::cognitive::CognitiveSystem;

#[tokio::test]
async fn test_system1_fuzzy_matching() {
    let sys1 = System1::new();
    
    // 1. Exact Match (Trie)
    let (resp, _) = sys1.process("status", &[]).await.expect("Exact match failed");
    assert!(resp.contains("ONLINE"));

    // 2. Fuzzy Match (1 char error, length 6 -> threshold 1)
    // "statu" vs "status" (delete 's') -> dist 1
    let (resp, _) = sys1.process("statu", &[]).await.expect("Fuzzy match 'statu' failed");
    assert!(resp.contains("ONLINE"));
    
    // "statuss" (insert 's') -> dist 1
    let (resp, _) = sys1.process("statuss", &[]).await.expect("Fuzzy match 'statuss' failed");
    assert!(resp.contains("ONLINE"));
    
    // "sttus" (substitute 'a' with nothing? no, 'a' missing) -> dist 1
    let (resp, _) = sys1.process("sttus", &[]).await.expect("Fuzzy match 'sttus' failed");
    assert!(resp.contains("ONLINE"));

    // 3. Fuzzy Match Fail
    // "st" vs "status" -> dist 4. "st" len 2 -> threshold 0. Should fail.
    let result = sys1.process("st", &[]).await;
    assert!(result.is_err(), "Should not match 'st'");

    // 4. Short command "help" (4 chars)
    // "hlp" (len 3) -> threshold 0. Dist 1. Fails.
    let result = sys1.process("hlp", &[]).await;
    assert!(result.is_err(), "Short input 'hlp' should not fuzzily match");

    // "helpp" (len 5) -> threshold 1. Dist 1. Matches.
    let (resp, _) = sys1.process("helpp", &[]).await.expect("Fuzzy match 'helpp' failed");
    assert!(resp.contains("Available commands"));
    
    // "heelp" (len 5) -> threshold 1. Dist 1. Matches.
    let (resp, _) = sys1.process("heelp", &[]).await.expect("Fuzzy match 'heelp' failed");
    assert!(resp.contains("Available commands"));
    
    // "hp" -> dist 2. Should fail.
    let result = sys1.process("hp", &[]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_system1_aliases() {
    let sys1 = System1::new();
    
    // "stats" is alias for "status"
    let (resp, _) = sys1.process("stats", &[]).await.expect("Alias 'stats' failed");
    assert!(resp.contains("ONLINE"));
    
    // Fuzzy alias: "statz" vs "stats" (dist 1)
    // "stats" len 5 -> threshold 1
    let (resp, _) = sys1.process("statz", &[]).await.expect("Fuzzy alias 'statz' failed");
    assert!(resp.contains("ONLINE"));
}
