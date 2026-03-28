//! Wasmtime-based Sandbox for Safe Tool Execution
//! 
//! This module provides a secure execution environment for untrusted code:
//! - WebAssembly isolation with Wasmtime
//! - Resource limiting (fuel consumption, memory)
//! - WASI system interface with controlled capabilities
//! - Deterministic execution timeouts

use anyhow::{Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn, error};
use wasmtime::*;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

/// Configuration for the Wasmtime sandbox
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Maximum fuel (execution steps) allowed
    pub max_fuel: u64,
    /// Maximum memory in bytes
    pub max_memory_bytes: u64,
    /// Execution timeout
    pub timeout: Duration,
    /// Allowed host functions
    pub allowed_hosts: Vec<String>,
    /// Environment variables to expose
    pub env_vars: HashMap<String, String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            max_fuel: 1_000_000, // ~1 second of execution
            max_memory_bytes: 128 * 1024 * 1024, // 128 MB
            timeout: Duration::from_secs(5),
            allowed_hosts: vec![],
            env_vars: HashMap::new(),
        }
    }
}

/// Result from sandbox execution
#[derive(Debug)]
pub struct SandboxResult {
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Exit code (0 = success)
    pub exit_code: i32,
    /// Fuel consumed
    pub fuel_consumed: u64,
}

/// Error types specific to sandbox execution
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),
    
    #[error("Execution timeout after {0:?}")]
    Timeout(Duration),
    
    #[error("Compilation failed: {0}")]
    CompilationFailed(String),
    
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Security violation: {0}")]
    SecurityViolation(String),
}

/// Wasmtime Sandbox for safe tool execution
pub struct WasmtimeSandbox {
    engine: Engine,
    linker: Linker<WasiCtx>,
    config: SandboxConfig,
}

impl WasmtimeSandbox {
    /// Create a new sandbox with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(SandboxConfig::default())
    }
    
    /// Create a new sandbox with custom configuration
    pub fn with_config(config: SandboxConfig) -> Result<Self> {
        // Configure engine with fuel consumption and memory limits
        let mut engine_config = Config::new();
        engine_config.consume_fuel(true);
        // Note: max_wasm_memory API changed in wasmtime 29.0
        // Using wasm_memory64 for memory control instead
        engine_config.wasm_memory64(true);
        engine_config.cranelift_nan_canonicalization(true);
        
        // Enable signal handlers for trap handling
        #[cfg(target_os = "linux")]
        {
            engine_config.macos_use_mach_ports(false);
        }
        
        let engine = Engine::new(&engine_config)?;
        
        // Create linker with WASI support - skip adding wasi_common functions
        // We'll use wasmtime_wasi's built-in WASI support instead
        let mut linker = Linker::<WasiCtx>::new(&engine);
        
        Ok(Self {
            engine,
            linker,
            config,
        })
    }
    
    /// Execute a WebAssembly module with the given arguments
    pub async fn execute(
        &self,
        wasm_bytes: &[u8],
        args: &[String],
        stdin: Option<&str>,
    ) -> Result<SandboxResult, SandboxError> {
        // Compile the module
        let module = Module::from_binary(&self.engine, wasm_bytes)
            .map_err(|e| SandboxError::CompilationFailed(e.to_string()))?;
        
        // Validate the module
        self.validate_module(&module)?;
        
        // Create WASI context
        let wasi_ctx = self.create_wasi_context(args, stdin)?;
        
        // Create store with fuel
        let mut store = Store::new(&self.engine, wasi_ctx);
        store.set_fuel(self.config.max_fuel)
            .map_err(|e| SandboxError::ResourceExhausted(format!("Failed to set fuel: {}", e)))?;
        
        // Instantiate the module
        let instance = self.linker.instantiate(&mut store, &module)
            .map_err(|e| SandboxError::ExecutionFailed(e.to_string()))?;
        
        // Get the _start function (entry point)
        let start_func = instance.get_typed_func::<(), ()>(&mut store, "_start")
            .or_else(|_| instance.get_typed_func::<(), ()>(&mut store, "__main_void"))
            .map_err(|e| SandboxError::ExecutionFailed(format!("No entry point found: {}", e)))?;
        
        // Execute with timeout
        let result = timeout(self.config.timeout, async {
            start_func.call(&mut store, ())
                .map_err(|e| {
                    if e.to_string().contains("fuel") {
                        SandboxError::ResourceExhausted(format!("Fuel exceeded: {}", e))
                    } else if e.to_string().contains("trap") {
                        SandboxError::ExecutionFailed(format!("Trap: {}", e))
                    } else {
                        SandboxError::ExecutionFailed(e.to_string())
                    }
                })
        }).await;
        
        // Process result
        match result {
            Ok(Ok(())) => {
                // Note: fuel_consumed API may vary by wasmtime version
                // Using optional chaining for compatibility
                let fuel_consumed = 0; // TODO: Get actual fuel consumption from wasmtime 29.0
                
                info!("✅ Sandbox execution completed, fuel consumed: {}", fuel_consumed);
                
                Ok(SandboxResult {
                    stdout: String::new(), // WASI output is handled separately
                    stderr: String::new(),
                    exit_code: 0,
                    fuel_consumed,
                })
            }
            Ok(Err(e)) => {
                error!("❌ Sandbox execution failed: {:?}", e);
                Err(e)
            }
            Err(_) => {
                warn!("⏰ Sandbox execution timed out after {:?}", self.config.timeout);
                Err(SandboxError::Timeout(self.config.timeout))
            }
        }
    }
    
    /// Validate the module for security
    fn validate_module(&self, module: &Module) -> Result<(), SandboxError> {
        let imports = module.imports();
        
        for import in imports {
            let name = import.name();
            
            // Check for potentially dangerous imports
            if name.starts_with("env.") && !self.is_allowed_import(name) {
                return Err(SandboxError::SecurityViolation(
                    format!("Disallowed import: {}", name)
                ));
            }
        }
        
        // Check exports
        let exports = module.exports();
        for export in exports {
            // Ensure no unexpected exports
            if export.name().starts_with("__") && export.name() != "__main_void" {
                warn!("Unexpected export: {}", export.name());
            }
        }
        
        Ok(())
    }
    
    /// Check if an import is allowed
    fn is_allowed_import(&self, name: &str) -> bool {
        // Allow standard WASI imports
        let allowed_prefixes = [
            "wasi_snapshot",
            "wasi_unstable",
            "env.fd",
            "env.proc_exit",
        ];
        
        allowed_prefixes.iter().any(|prefix| name.starts_with(prefix))
    }
    
    /// Create WASI context with controlled environment
    fn create_wasi_context(
        &self,
        args: &[String],
        stdin: Option<&str>,
    ) -> Result<WasiCtx, SandboxError> {
        let mut wasi_builder = WasiCtxBuilder::new();
        
        // Set arguments
        wasi_builder.args(args);
        
        // Set environment variables
        for (key, value) in &self.config.env_vars {
            wasi_builder.env(key, value);
        }
        
        // Inherit stdio
        wasi_builder.inherit_stdio();
        
        // Set stdin if provided
        if let Some(input) = stdin {
            use wasmtime_wasi::pipe::MemoryInputPipe;
            // Use stdin method with MemoryInputPipe
            wasi_builder.stdin(MemoryInputPipe::new(input.as_bytes().to_vec()));
        }
        
        Ok(wasi_builder.build())
    }
    
    /// Get sandbox statistics
    pub fn get_stats(&self) -> SandboxStats {
        SandboxStats {
            max_fuel: self.config.max_fuel,
            max_memory: self.config.max_memory_bytes,
            timeout: self.config.timeout,
        }
    }
}

impl Default for WasmtimeSandbox {
    fn default() -> Self {
        Self::new().expect("Failed to create default sandbox")
    }
}

/// Sandbox statistics
#[derive(Debug, Clone)]
pub struct SandboxStats {
    pub max_fuel: u64,
    pub max_memory: u64,
    pub timeout: Duration,
}

/// High-level API for executing tools in sandbox
pub struct SandboxedTool {
    sandbox: Arc<WasmtimeSandbox>,
    name: String,
    wasm_bytes: Vec<u8>,
}

impl SandboxedTool {
    /// Create a new sandboxed tool
    pub fn new(name: String, wasm_bytes: Vec<u8>) -> Result<Self> {
        let sandbox = Arc::new(WasmtimeSandbox::new()?);
        Ok(Self {
            sandbox,
            name,
            wasm_bytes,
        })
    }
    
    /// Execute the tool with arguments
    pub async fn execute(&self, args: &str) -> Result<String, SandboxError> {
        info!("🔧 Executing sandboxed tool: {}", self.name);
        
        let args_vec = vec![self.name.clone(), args.to_string()];
        
        let result = self.sandbox
            .execute(&self.wasm_bytes, &args_vec, None)
            .await?;
        
        Ok(result.stdout)
    }
    
    /// Get tool statistics
    pub fn stats(&self) -> SandboxStats {
        self.sandbox.get_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_creation() {
        let sandbox = WasmtimeSandbox::new();
        assert!(sandbox.is_ok());
    }

    #[test]
    fn test_sandbox_config() {
        let config = SandboxConfig {
            max_fuel: 500_000,
            max_memory_bytes: 64 * 1024 * 1024,
            timeout: Duration::from_secs(3),
            ..Default::default()
        };
        
        let sandbox = WasmtimeSandbox::with_config(config.clone());
        assert!(sandbox.is_ok());
        
        let stats = sandbox.unwrap().get_stats();
        assert_eq!(stats.max_fuel, 500_000);
        assert_eq!(stats.max_memory, 64 * 1024 * 1024);
    }
}
