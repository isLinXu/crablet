use std::sync::Arc;
use std::net::IpAddr;
use governor::{
    clock::DefaultClock,
    state::{keyed::DashMapStateStore, direct::NotKeyed},
    Quota, RateLimiter,
};
use std::num::NonZeroU32;
use dashmap::DashMap;
use crate::error::{CrabletError, Result};

pub struct MultiLayerRateLimiter {
    // Global limiter (Not keyed, applies to total requests)
    global_limiter: RateLimiter<NotKeyed, governor::state::InMemoryState, DefaultClock>,
    // IP limiter (Keyed by IpAddr)
    ip_limiter: RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock>,
    // User limiter (Keyed by String - UserID)
    user_limiter: Arc<DashMap<String, RateLimiter<NotKeyed, governor::state::InMemoryState, DefaultClock>>>,
    // Config
    user_quota: Quota,
}

// Keep the type alias for compatibility if needed, or update usages
pub type GlobalRateLimiter = MultiLayerRateLimiter;

pub fn create_limiter() -> Arc<GlobalRateLimiter> {
    // 1. Global Quota: 1000 req/s
    let global_quota = Quota::per_second(NonZeroU32::new(1000).expect("Quota limit must be non-zero"));
    
    // 2. IP Quota: 10 req/s (burst 20)
    let ip_quota = Quota::per_second(NonZeroU32::new(10).expect("Quota limit must be non-zero"))
        .allow_burst(NonZeroU32::new(20).expect("Burst limit must be non-zero"));
    
    // 3. User Quota: 50 req/s (burst 100) - stored for dynamic creation
    let user_quota = Quota::per_second(NonZeroU32::new(50).expect("Quota limit must be non-zero"))
        .allow_burst(NonZeroU32::new(100).expect("Burst limit must be non-zero"));

    Arc::new(MultiLayerRateLimiter {
        global_limiter: RateLimiter::direct(global_quota),
        ip_limiter: RateLimiter::keyed(ip_quota),
        user_limiter: Arc::new(DashMap::new()),
        user_quota,
    })
}

impl MultiLayerRateLimiter {
    pub fn check_key(&self, ip: &IpAddr) -> Result<()> {
        // 1. Global Check
        if self.global_limiter.check().is_err() {
             return Err(CrabletError::Other(anyhow::anyhow!("Global rate limit exceeded")));
        }
        
        // 2. IP Check
        if self.ip_limiter.check_key(ip).is_err() {
             return Err(CrabletError::Other(anyhow::anyhow!("IP rate limit exceeded")));
        }
        
        Ok(())
    }
    
    pub fn check_user(&self, user_id: &str) -> Result<()> {
        // Get or create limiter for user
        let limiter = self.user_limiter.entry(user_id.to_string())
            .or_insert_with(|| RateLimiter::direct(self.user_quota));
            
        if limiter.check().is_err() {
            return Err(CrabletError::Other(anyhow::anyhow!("User rate limit exceeded")));
        }
        Ok(())
    }
}

