---
title: Middleware
description: Build cognitive middleware for the ReAct engine
---

# :filter: Middleware

Cognitive middleware allows you to intercept and modify the agent's thinking process at various stages.

## CognitiveMiddleware Trait

```rust
#[async_trait]
pub trait CognitiveMiddleware: Send + Sync {
    /// Name of this middleware
    fn name(&self) -> &str;
    
    /// Pre-process a request before the cognitive engine
    async fn pre_process(&self, request: &mut CognitiveRequest) -> Result<MiddlewareAction>;
    
    /// Post-process a response after the cognitive engine
    async fn post_process(&self, response: &mut CognitiveResponse) -> Result<()>;
}

pub enum MiddlewareAction {
    Continue,           // Pass through to next middleware
    Skip,               // Skip remaining middleware
    Abort(String),      // Abort with error message
    Redirect(CognitiveLayer),  // Redirect to different cognitive layer
}
```

## Example: Rate Limiting Middleware

```rust
pub struct RateLimitMiddleware {
    limiter: RateLimiter,
}

#[async_trait]
impl CognitiveMiddleware for RateLimitMiddleware {
    fn name(&self) -> &str { "rate_limit" }
    
    async fn pre_process(&self, request: &mut CognitiveRequest) -> Result<MiddlewareAction> {
        if !self.limiter.allow() {
            return Ok(MiddlewareAction::Abort(
                "Rate limit exceeded. Please try again later.".to_string()
            ));
        }
        Ok(MiddlewareAction::Continue)
    }
    
    async fn post_process(&self, _response: &mut CognitiveResponse) -> Result<()> {
        Ok(())
    }
}
```

## Registering Middleware

```rust
// In your initialization code
engine.add_middleware(RateLimitMiddleware::new(config));
engine.add_middleware(LoggingMiddleware::new());
engine.add_middleware(CachingMiddleware::new(cache));
```

## Execution Order

Middleware executes in registration order for pre-processing, and reverse order for post-processing:

```
Pre:  MW1 → MW2 → MW3 → [Engine]
Post: [Engine] → MW3 → MW2 → MW1
```
