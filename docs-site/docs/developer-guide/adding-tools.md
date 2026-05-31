---
title: Adding Tools
description: Create new built-in tools by implementing the Plugin trait
---

# :wrench: Adding Tools

Extend Crablet's capabilities by implementing new built-in tools.

## Plugin Trait

All tools implement the `Plugin` trait:

```rust
use crate::plugins::Plugin;
use async_trait::async_trait;
use anyhow::Result;

#[async_trait]
pub trait Plugin: Send + Sync {
    /// Unique tool identifier
    fn name(&self) -> &str;
    
    /// Human-readable description (shown to LLM)
    fn description(&self) -> &str;
    
    /// JSON Schema for input parameters
    fn parameters_schema(&self) -> serde_json::Value;
    
    /// Execute the tool
    async fn execute(&self, input: &str) -> Result<String>;
    
    /// Whether this tool requires safety approval
    fn requires_approval(&self) -> bool {
        false
    }
}
```

## Example: DNS Lookup Tool

```rust
// src/tools/dns_lookup.rs
use crate::plugins::Plugin;
use async_trait::async_trait;
use anyhow::Result;
use serde_json::json;

pub struct DnsLookup;

#[async_trait]
impl Plugin for DnsLookup {
    fn name(&self) -> &str { "dns_lookup" }
    
    fn description(&self) -> &str {
        "Look up DNS records for a domain name"
    }
    
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "domain": {
                    "type": "string",
                    "description": "Domain name to look up"
                },
                "record_type": {
                    "type": "string",
                    "enum": ["A", "AAAA", "MX", "TXT", "CNAME"],
                    "default": "A"
                }
            },
            "required": ["domain"]
        })
    }
    
    async fn execute(&self, input: &str) -> Result<String> {
        let args: DnsArgs = serde_json::from_str(input)?;
        // Perform DNS lookup...
        Ok(result)
    }
    
    fn requires_approval(&self) -> bool {
        false  // DNS lookups are safe
    }
}
```

## Registering Your Tool

Add your tool to the registry in `src/tools/mod.rs`:

```rust
mod dns_lookup;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(dns_lookup::DnsLookup);
    // ... other tools
}
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_dns_lookup() {
        let tool = DnsLookup;
        let result = tool.execute(r#"{"domain": "example.com"}"#).await;
        assert!(result.is_ok());
    }
}
```

## Guidelines

- Always add `#[cfg(test)]` unit tests
- Include doc comments on public API
- Pass `cargo clippy` without warnings
- Handle errors gracefully with `anyhow`
- Respect the Safety Oracle for dangerous operations
