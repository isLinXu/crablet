pub mod limits {
    pub const MAX_INPUT_SIZE: usize = 10 * 1024;      // 10KB
    pub const MAX_OUTPUT_SIZE: usize = 10 * 1024;      // 10KB 
    pub const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;  // 10MB
    pub const BASH_TIMEOUT_SECS: u64 = 10;
    pub const LLM_TIMEOUT_SECS: u64 = 120;
    pub const SWARM_MSG_TIMEOUT_SECS: u64 = 30;
    pub const SWARM_TOTAL_TIMEOUT_SECS: u64 = 120;
}

pub mod cache {
    pub const LRU_CAPACITY: usize = 100;
    // pub const SQLITE_CACHE_SIZE: i32 = -64000;         // 64MB (Not used directly in code yet, usually passed to pragma)
    pub const DB_POOL_MIN: u32 = 2;
    pub const DB_POOL_MAX: u32 = 10;
}

pub mod react {
    pub const DEFAULT_MAX_STEPS: usize = 10;
    pub const PARALLEL_TOOL_SEMAPHORE: usize = 4;
    pub const LOOP_SIMILARITY_THRESHOLD: f32 = 0.85; // Changed to f32 to match usage
}

pub mod complexity {
    pub const TEMPORAL_KEYWORDS: &[&str] = &["yesterday", "tomorrow", "schedule", "timeline", "future", "past"];
    pub const DOMAIN_KEYWORDS: &[&str] = &["code", "function", "error", "debug", "api", "database", "sql", "rust", "python"];
    pub const ANALYTICAL_KEYWORDS: &[&str] = &["analyze", "compare", "evaluate", "assess", "pros", "cons"];
}
