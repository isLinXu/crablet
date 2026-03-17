use std::sync::Arc;
use std::net::IpAddr;
use std::time::{Duration, Instant};
use governor::{
    clock::DefaultClock,
    state::{keyed::DashMapStateStore, direct::NotKeyed},
    Quota, RateLimiter,
};
use std::num::NonZeroU32;
use dashmap::DashMap;
use crate::error::{CrabletError, Result};

// Slow request tracking
#[derive(Clone, Debug)]
struct SlowRequestTracker {
    request_count: u32,
    total_duration_ms: u64,
    last_check: Instant,
}

pub struct MultiLayerRateLimiter {
    // Global limiter (Not keyed, applies to total requests)
    global_limiter: RateLimiter<NotKeyed, governor::state::InMemoryState, DefaultClock>,
    // IP limiter (Keyed by IpAddr)
    ip_limiter: RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock>,
    // User limiter (Keyed by String - UserID)
    user_limiter: Arc<DashMap<String, RateLimiter<NotKeyed, governor::state::InMemoryState, DefaultClock>>>,
    // Slow request tracker (IP-based)
    slow_request_tracker: Arc<DashMap<IpAddr, SlowRequestTracker>>,
    // API Key limiter (Keyed by API Key)
    api_key_limiter: Arc<DashMap<String, RateLimiter<NotKeyed, governor::state::InMemoryState, DefaultClock>>>,
    // Config
    user_quota: Quota,
    api_key_quota: Quota,
    // Slow request threshold (in ms)
    slow_request_threshold: u64,
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

    // 4. API Key Quota: 100 req/s (burst 200) - higher than user limit for API access
    let api_key_quota = Quota::per_second(NonZeroU32::new(100).expect("Quota limit must be non-zero"))
        .allow_burst(NonZeroU32::new(200).expect("Burst limit must be non-zero"));

    // Slow request threshold: 5 seconds
    let slow_request_threshold = 5000;

    Arc::new(MultiLayerRateLimiter {
        global_limiter: RateLimiter::direct(global_quota),
        ip_limiter: RateLimiter::keyed(ip_quota),
        user_limiter: Arc::new(DashMap::new()),
        slow_request_tracker: Arc::new(DashMap::new()),
        api_key_limiter: Arc::new(DashMap::new()),
        user_quota,
        api_key_quota,
        slow_request_threshold,
    })
}

impl MultiLayerRateLimiter {
    pub fn check_key(&self, ip: &IpAddr) -> Result<()> {
        // 1. Global Check
        if self.global_limiter.check().is_err() {
             return Err(CrabletError::Other(anyhow::anyhow!("Global rate limit exceeded")));
        }

        // 2. Slow Request Check (mitigate slow DoS attacks)
        if let Some(tracker) = self.slow_request_tracker.get(ip) {
            // Check if too many slow requests in last minute
            if tracker.request_count > 10 && tracker.total_duration_ms / tracker.request_count as u64 > self.slow_request_threshold {
                if tracker.last_check.elapsed() < Duration::from_secs(60) {
                    return Err(CrabletError::Other(anyhow::anyhow!("Too many slow requests. Please optimize your requests.")));
                }
            }
        }

        // 3. IP Check
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

    pub fn check_api_key(&self, api_key: &str) -> Result<()> {
        // Get or create limiter for API key
        let limiter = self.api_key_limiter.entry(api_key.to_string())
            .or_insert_with(|| RateLimiter::direct(self.api_key_quota));

        if limiter.check().is_err() {
            return Err(CrabletError::Other(anyhow::anyhow!("API key rate limit exceeded")));
        }
        Ok(())
    }

    /// Track a slow request for DoS detection
    pub fn track_slow_request(&self, ip: &IpAddr, duration_ms: u64) {
        if duration_ms < self.slow_request_threshold {
            return; // Not a slow request
        }

        let mut tracker = self.slow_request_tracker.entry(*ip)
            .or_insert_with(|| SlowRequestTracker {
                request_count: 0,
                total_duration_ms: 0,
                last_check: Instant::now(),
            });

        tracker.request_count += 1;
        tracker.total_duration_ms += duration_ms;
        tracker.last_check = Instant::now();
    }

    /// Cleanup old slow request trackers (should be called periodically)
    pub fn cleanup_slow_request_trackers(&self) {
        let mut to_remove = Vec::new();
        for entry in self.slow_request_tracker.iter() {
            if entry.last_check.elapsed() > Duration::from_secs(300) {
                // Remove trackers older than 5 minutes
                to_remove.push(*entry.key());
            }
        }
        for key in to_remove {
            self.slow_request_tracker.remove(&key);
        }
    }
}

