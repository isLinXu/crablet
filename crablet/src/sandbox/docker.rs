use anyhow::{anyhow, Result};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{info, warn};
/// 安全的 Docker 执行器
/// 提供完全隔离的执行环境
pub struct DockerExecutor {
    /// 容器内存限制 (MB)
    pub memory_limit: usize,
    /// CPU 限制 (核心数)
    pub cpu_limit: f32,
    /// 网络访问权限
    pub network_enabled: bool,
    /// 工作目录挂载
    pub work_dir: Option<String>,
    /// 超时时间 (秒)
    pub timeout_secs: u64,
}

impl DockerExecutor {
    /// 创建默认配置的执行器 (严格隔离)
    pub fn strict() -> Self {
        Self {
            memory_limit: 128,
            cpu_limit: 0.5,
            network_enabled: false,
            work_dir: None,
            timeout_secs: 30,
        }
    }

    /// 创建宽松配置的执行器 (允许网络访问)
    pub fn with_network() -> Self {
        Self {
            memory_limit: 256,
            cpu_limit: 1.0,
            network_enabled: true,
            work_dir: None,
            timeout_secs: 60,
        }
    }

    /// 配置自定义参数
    pub fn with_memory(mut self, mb: usize) -> Self {
        self.memory_limit = mb;
        self
    }

    pub fn with_cpu(mut self, cores: f32) -> Self {
        self.cpu_limit = cores;
        self
    }

    pub fn with_work_dir(mut self, dir: impl Into<String>) -> Self {
        self.work_dir = Some(dir.into());
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// 在 Docker 容器中执行命令
    pub async fn execute(&self, image: &str, cmd: &[&str]) -> Result<ExecutionResult> {
        // 检查 Docker 是否可用
        if !self.is_docker_available().await {
            return Err(anyhow!("Docker is not available. Please ensure Docker is installed and running."));
        }

        let mut docker_args = vec![
            "run".to_string(),
            "--rm".to_string(),           // 自动删除容器
            "--network".to_string(),
            if self.network_enabled { "bridge".to_string() } else { "none".to_string() },
            "--memory".to_string(),
            format!("{}m", self.memory_limit),
            "--cpus".to_string(),
            self.cpu_limit.to_string(),
            "--read-only".to_string(),     // 只读根文件系统
            "--tmpfs".to_string(),         // 临时文件系统
            "/tmp:noexec,nosuid,size=50m".to_string(),
            "--security-opt".to_string(),
            "no-new-privileges:true".to_string(),  // 禁止提升权限
            "--cap-drop".to_string(),
            "ALL".to_string(),              // 丢弃所有能力
        ];

        // 挂载工作目录（如果需要）
        if let Some(ref work_dir) = self.work_dir {
            let abs_path = std::fs::canonicalize(work_dir)
                .unwrap_or_else(|_| std::path::PathBuf::from(work_dir));
            docker_args.push("-v".to_string());
            docker_args.push(format!("{}:/workspace:rw", abs_path.display()));
            docker_args.push("-w".to_string());
            docker_args.push("/workspace".to_string());
        }

        docker_args.push(image.to_string());
        docker_args.extend(cmd.iter().map(|s| s.to_string()));

        info!("Executing Docker command: docker {:?}", docker_args);

        let output = tokio::time::timeout(
            tokio::time::Duration::from_secs(self.timeout_secs),
            Command::new("docker")
                .args(&docker_args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await
        .map_err(|_| anyhow!("Docker execution timed out after {} seconds", self.timeout_secs))?
        .map_err(|e| anyhow!("Failed to execute Docker command: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            warn!(
                "Docker command failed with exit code {:?}",
                output.status.code()
            );
            return Ok(ExecutionResult {
                success: false,
                exit_code: output.status.code().unwrap_or(-1),
                stdout,
                stderr,
            });
        }

        Ok(ExecutionResult {
            success: true,
            exit_code: 0,
            stdout,
            stderr,
        })
    }

    /// 检查 Docker 是否可用
    async fn is_docker_available(&self) -> bool {
        match Command::new("docker")
            .args(&["info", "--format", "{{.ServerVersion}}"])
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// 构建安全的 Docker 镜像（用于执行不受信任的代码）
    pub async fn build_sandbox_image(&self, dockerfile: &str, tag: &str) -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let dockerfile_path = temp_dir.path().join("Dockerfile");
        tokio::fs::write(&dockerfile_path, dockerfile).await?;

        let output = Command::new("docker")
            .args(&[
                "build",
                "-t",
                tag,
                temp_dir.path().to_str().unwrap(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| anyhow!("Failed to build Docker image: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Docker build failed: {}", stderr));
        }

        info!("Successfully built sandbox image: {}", tag);
        Ok(())
    }
}

/// 执行结果
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl ExecutionResult {
    /// 获取格式化后的输出
    pub fn formatted_output(&self) -> String {
        format!(
            "Exit Code: {}\nStdout:\n{}\nStderr:\n{}",
            self.exit_code, self.stdout, self.stderr
        )
    }

    /// 检查是否包含错误关键词
    pub fn contains_error_keywords(&self) -> bool {
        let error_keywords = ["error", "panic", "exception", "fatal"];
        let combined = format!("{} {}", self.stdout, self.stderr).to_lowercase();
        error_keywords.iter().any(|kw| combined.contains(kw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_docker_executor_strict() {
        let executor = DockerExecutor::strict();
        assert_eq!(executor.memory_limit, 128);
        assert!(!executor.network_enabled);
        assert_eq!(executor.timeout_secs, 30);
    }

    #[tokio::test]
    async fn test_docker_executor_with_network() {
        let executor = DockerExecutor::with_network();
        assert_eq!(executor.memory_limit, 256);
        assert!(executor.network_enabled);
        assert_eq!(executor.timeout_secs, 60);
    }

    #[test]
    fn test_execution_result_formatted_output() {
        let result = ExecutionResult {
            success: true,
            exit_code: 0,
            stdout: "Hello".to_string(),
            stderr: "".to_string(),
        };
        let output = result.formatted_output();
        assert!(output.contains("Exit Code: 0"));
        assert!(output.contains("Hello"));
    }

    #[test]
    fn test_execution_result_contains_error_keywords() {
        let result_with_error = ExecutionResult {
            success: false,
            exit_code: 1,
            stdout: "".to_string(),
            stderr: "Runtime error occurred".to_string(),
        };
        assert!(result_with_error.contains_error_keywords());

        let result_clean = ExecutionResult {
            success: true,
            exit_code: 0,
            stdout: "Success output".to_string(),
            stderr: "".to_string(),
        };
        assert!(!result_clean.contains_error_keywords());
    }
}
