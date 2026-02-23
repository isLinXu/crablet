use crablet::safety::oracle::{SafetyOracle, SafetyLevel, SafetyDecision};

#[test]
fn test_safety_oracle_strict() {
    let oracle = SafetyOracle::new(SafetyLevel::Strict);
    
    // Allowed commands
    assert_eq!(oracle.check_bash_command("ls -la"), SafetyDecision::Allowed);
    assert_eq!(oracle.check_bash_command("echo hello"), SafetyDecision::Allowed);
    
    // Dangerous commands requiring confirmation
    let decision = oracle.check_bash_command("rm -rf /");
    match decision {
        SafetyDecision::RequireConfirmation(_) => {},
        _ => panic!("Expected RequireConfirmation, got {:?}", decision),
    }
    
    // Blocked commands
    let decision = oracle.check_bash_command("netcat evil.com");
    match decision {
        SafetyDecision::Blocked(_) => {},
        _ => panic!("Expected Blocked, got {:?}", decision),
    }
}

#[test]
fn test_safety_oracle_file_access() {
    let oracle = SafetyOracle::new(SafetyLevel::Strict);
    
    // Allowed paths
    assert_eq!(oracle.check_file_access("data/file.txt"), SafetyDecision::Allowed);
    
    // Blocked path traversal
    let decision = oracle.check_file_access("../../etc/passwd");
    match decision {
        SafetyDecision::Blocked(msg) => assert!(msg.contains("Path traversal")),
        _ => panic!("Expected Blocked, got {:?}", decision),
    }
}
