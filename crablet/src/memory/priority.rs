use std::time::Instant;

/// MemoryPriority tracks the importance and access patterns of a memory block.
#[derive(Clone, Debug)]
pub struct MemoryPriority {
    pub access_count: usize,
    pub last_access: Instant,
    pub importance: f32, // 0.0 - 1.0
    pub decay_rate: f32, // decay per hour
}

impl MemoryPriority {
    pub fn new(importance: f32) -> Self {
        Self {
            access_count: 1,
            last_access: Instant::now(),
            importance,
            decay_rate: 0.05, // 5% decay per hour by default
        }
    }

    /// Calculate the current priority score (importance + access frequency - decay)
    pub fn score(&self) -> f32 {
        let hours_elapsed = self.last_access.elapsed().as_secs_f32() / 3600.0;
        // Exponential decay: score = importance * (1 - decay_rate)^hours
        let decayed_importance = self.importance * (1.0 - self.decay_rate).powf(hours_elapsed);
        
        // Boost score based on access count (logarithmic)
        let access_boost = (self.access_count as f32).log2().max(0.0);
        
        decayed_importance + access_boost * 0.1
    }

    /// Check if the memory should be archived based on its priority score
    pub fn should_archive(&self) -> bool {
        self.score() < 0.1
    }

    /// Record an access to this memory
    pub fn touch(&mut self) {
        self.access_count += 1;
        self.last_access = Instant::now();
    }
}
