use super::{Sandbox, Language, ExecutionResult};
use async_trait::async_trait;
use bollard::Docker;
use bollard::container::{Config, CreateContainerOptions, LogOutput, StartContainerOptions, WaitContainerOptions};
use bollard::image::CreateImageOptions;
use futures::StreamExt;
use anyhow::{Result, Context, anyhow};
use tracing::{info, warn, debug};
use std::time::Instant;
use uuid::Uuid;

pub struct DockerSandbox {
    docker: Docker,
    image_map: std::collections::HashMap<String, String>,
}

impl DockerSandbox {
    pub fn new() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker daemon. Ensure Docker is running.")?;
            
        let mut image_map = std::collections::HashMap::new();
        // Use lightweight images
        image_map.insert("python".to_string(), "python:3.11-slim".to_string());
        image_map.insert("node".to_string(), "node:20-alpine".to_string());
        image_map.insert("shell".to_string(), "alpine:latest".to_string());
        image_map.insert("lua".to_string(), "nickblah/lua:5.4-alpine".to_string());
        
        Ok(Self { docker, image_map })
    }
    
    fn get_image(&self, language: Language) -> Result<&str> {
        let key = match language {
            Language::Python => "python",
            Language::JavaScript => "node",
            Language::Shell => "shell",
            Language::Lua => "lua",
        };
        self.image_map.get(key).map(|s| s.as_str()).ok_or_else(|| anyhow::anyhow!("Image not found for language {:?}", language))
    }
    
    fn get_cmd(&self, language: Language, code: &str) -> Vec<String> {
        match language {
            Language::Python => vec!["python".to_string(), "-c".to_string(), code.to_string()],
            Language::JavaScript => vec!["node".to_string(), "-e".to_string(), code.to_string()],
            Language::Shell => vec!["sh".to_string(), "-c".to_string(), code.to_string()],
            Language::Lua => vec!["lua".to_string(), "-e".to_string(), code.to_string()],
        }
    }
}

#[async_trait]
impl Sandbox for DockerSandbox {
    async fn init(&self) -> Result<()> {
        for image in self.image_map.values() {
            info!("Checking/Pulling Docker image: {}", image);
            
            // Check if image exists locally first to save time
            if self.docker.inspect_image(image).await.is_ok() {
                debug!("Image {} exists locally", image);
                continue;
            }
            
            info!("Pulling image {}", image);
            let mut stream = self.docker.create_image(
                Some(CreateImageOptions {
                    from_image: image.clone(),
                    ..Default::default()
                }),
                None,
                None
            );
            
            while let Some(result) = stream.next().await {
                if let Err(e) = result {
                    warn!("Error pulling image {}: {}", image, e);
                    // Don't fail completely, maybe network issue, try to proceed if cached
                }
            }
        }
        Ok(())
    }

    async fn execute(&self, language: Language, code: &str) -> Result<ExecutionResult> {
        let image = self.get_image(language)?;
        let cmd = self.get_cmd(language, code);
        let container_name = format!("crablet-sandbox-{}", Uuid::new_v4());
        
        // 1. Create Container
        let config = Config {
            image: Some(image),
            cmd: Some(cmd.iter().map(|s| s.as_str()).collect()),
            network_disabled: Some(true), // Security: No network access
            host_config: Some(bollard::service::HostConfig {
                memory: Some(100 * 1024 * 1024), // 100MB Limit
                cpu_quota: Some(50000), // 50% CPU
                ..Default::default()
            }),
            ..Default::default()
        };
        
        let _id = self.docker.create_container(
            Some(CreateContainerOptions { name: container_name.clone(), platform: None }),
            config
        ).await?.id;
        
        let start = Instant::now();
        
        // 2. Start Container
        self.docker.start_container(&container_name, None::<StartContainerOptions<String>>).await?;
        
        // 3. Wait for finish
        let mut wait_stream = self.docker.wait_container(
            &container_name,
            Some(WaitContainerOptions { condition: "not-running" })
        );
        
        let status_code = if let Some(Ok(res)) = wait_stream.next().await {
            res.status_code
        } else {
            -1
        };
        
        let duration = start.elapsed();
        
        // 4. Get Logs
        let mut stdout = String::new();
        let mut stderr = String::new();
        
        let mut logs_stream = self.docker.logs(
            &container_name,
            Some(bollard::container::LogsOptions {
                stdout: true,
                stderr: true,
                ..Default::default()
            })
        );
        
        while let Some(Ok(log)) = logs_stream.next().await {
            match log {
                LogOutput::StdOut { message } => stdout.push_str(&String::from_utf8_lossy(&message)),
                LogOutput::StdErr { message } => stderr.push_str(&String::from_utf8_lossy(&message)),
                _ => {}
            }
        }
        
        // 5. Cleanup (Remove Container)
        let _ = self.docker.remove_container(
            &container_name,
            Some(bollard::container::RemoveContainerOptions { force: true, ..Default::default() })
        ).await;
        
        Ok(ExecutionResult {
            stdout,
            stderr,
            exit_code: status_code as i32,
            duration_ms: duration.as_millis() as u64,
        })
    }

    async fn cleanup(&self) -> Result<()> {
        // Nothing persistent to clean up for now as we remove containers after execution
        Ok(())
    }
}
