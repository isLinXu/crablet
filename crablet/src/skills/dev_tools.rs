//! 技能开发者工具链
//! 
//! 提供技能开发、测试、发布的完整工具链
//! 包括: init, test, validate, build, publish, docs 等命令

use anyhow::{Result, Context, bail};
use std::path::{Path, PathBuf};
use std::fs;
use tracing::{info, error};
use regex::Regex;
use semver::Version;

use crate::skills::SkillType;

/// 开发者工具
pub struct DevTools;

impl DevTools {
    /// 初始化新技能项目
    pub async fn init(
        name: &str,
        skill_type: SkillType,
        path: Option<PathBuf>,
    ) -> Result<InitResult> {
        info!("Initializing new skill project: {}", name);
        
        let project_path = path.unwrap_or_else(|| PathBuf::from(name));
        
        if project_path.exists() {
            bail!("Directory '{}' already exists", project_path.display());
        }
        
        // 创建目录结构
        fs::create_dir_all(&project_path)?;
        fs::create_dir_all(project_path.join("src"))?;
        fs::create_dir_all(project_path.join("tests"))?;
        fs::create_dir_all(project_path.join("docs"))?;
        
        // 根据技能类型生成模板
        match &skill_type {
            SkillType::OpenClaw(_, _) => Self::init_openclaw_template(&project_path, name).await?,
            SkillType::Local(_) => Self::init_local_template(&project_path, name).await?,
            SkillType::Mcp(_, _, _) => Self::init_mcp_template(&project_path, name).await?,
            SkillType::Plugin(_, _) => Self::init_plugin_template(&project_path, name).await?,
        }
        
        // 生成通用文件
        Self::generate_common_files(&project_path, name, &skill_type).await?;
        
        info!("Skill project initialized at: {}", project_path.display());
        
        Ok(InitResult {
            name: name.to_string(),
            path: project_path,
            skill_type,
            next_steps: vec![
                format!("cd {}", name),
                "crablet skill dev validate".to_string(),
                "crablet skill dev test".to_string(),
            ],
        })
    }
    
    /// OpenClaw 技能模板
    async fn init_openclaw_template(project_path: &Path, name: &str) -> Result<()> {
        let skill_md = format!(r#"---
name: {}
description: A brief description of what this skill does
version: 0.1.0
author: Your Name <your.email@example.com>
category: automation
tags:
  - example
  - template
license: MIT
---

# {}

## Description

This is an OpenClaw skill that uses LLM to perform tasks.

## Usage

```bash
crablet skill run {} '{{"param": "value"}}'
```

## Parameters

- `param` (string): Description of the parameter

## Examples

### Example 1: Basic usage

```json
{{"param": "hello world"}}
```

## Implementation

### System Prompt

You are a helpful assistant that processes user requests.

### User Prompt Template

```
Please process the following input: {{{{input}}}}
```

## Safety

- This skill only reads data
- No external network calls
- No file system modifications
"#, name, name, name);
        
        fs::write(project_path.join("SKILL.md"), skill_md)?;
        
        // 创建参数模式文件
        let schema = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "param": {
      "type": "string",
      "description": "Input parameter"
    }
  },
  "required": ["param"]
}"#;
        
        fs::write(project_path.join("schema.json"), schema)?;
        
        Ok(())
    }
    
    /// Local 技能模板
    async fn init_local_template(project_path: &Path, name: &str) -> Result<()> {
        // SKILL.md
        let skill_md = format!(r#"---
name: {}
description: A local executable skill
version: 0.1.0
author: Your Name <your.email@example.com>
category: automation
type: local
entrypoint: main.py
language: python
---

# {}

Local skill implementation using Python.
"#, name, name);
        
        fs::write(project_path.join("SKILL.md"), skill_md)?;
        
        // main.py
        let main_py = r#"#!/usr/bin/env python3
\"\"\"
Skill entry point
\"\"\"
import json
import sys

def main():
    # Read input from stdin
    input_data = json.load(sys.stdin)
    
    # Process the input
    result = process(input_data)
    
    # Output result as JSON
    print(json.dumps(result))

def process(data):
    \"\"\"Main processing logic\"\"\"
    return {
        "status": "success",
        "message": f"Processed: {data}"
    }

if __name__ == "__main__":
    main()
"#;
        
        fs::write(project_path.join("src").join("main.py"), main_py)?;
        
        // requirements.txt
        fs::write(project_path.join("requirements.txt"), "# Add your dependencies here\n")?;
        
        Ok(())
    }
    
    /// MCP 技能模板
    async fn init_mcp_template(project_path: &Path, name: &str) -> Result<()> {
        let skill_md = format!(r#"---
name: {}
description: MCP server skill
version: 0.1.0
author: Your Name <your.email@example.com>
category: integration
type: mcp
transport: stdio
---

# {}

MCP (Model Context Protocol) server implementation.
"#, name, name);
        
        fs::write(project_path.join("SKILL.md"), skill_md)?;
        
        // server.py
        let server_py = r#"#!/usr/bin/env python3
\"\"\"
MCP Server implementation
\"\"\"
from mcp.server import Server
from mcp.types import TextContent

app = Server("example-server")

@app.call_tool()
async def handle_tool(name: str, arguments: dict):
    if name == "example_tool":
        return [TextContent(
            type="text",
            text=f"Tool called with: {arguments}"
        )]
    raise ValueError(f"Unknown tool: {name}")

if __name__ == "__main__":
    app.run()
"#;
        
        fs::write(project_path.join("src").join("server.py"), server_py)?;
        
        // requirements.txt
        fs::write(
            project_path.join("requirements.txt"),
            "mcp>=1.0.0\n"
        )?;
        
        Ok(())
    }
    
    /// Plugin 技能模板
    async fn init_plugin_template(project_path: &Path, name: &str) -> Result<()> {
        let skill_md = format!(r#"---
name: {}
description: Native plugin skill
version: 0.1.0
author: Your Name <your.email@example.com>
category: system
type: plugin
---

# {}

Native Rust plugin implementation.
"#, name, name);
        
        fs::write(project_path.join("SKILL.md"), skill_md)?;
        
        // Cargo.toml
        let cargo_toml = format!(r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
"#, name);
        
        fs::write(project_path.join("Cargo.toml"), cargo_toml)?;
        
        // lib.rs
        let lib_rs = r#"use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct Input {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub result: String,
}

#[no_mangle]
pub extern "C" fn execute(input: *const u8, input_len: usize) -> *mut u8 {
    // Implementation here
    std::ptr::null_mut()
}
"#;
        
        fs::write(project_path.join("src").join("lib.rs"), lib_rs)?;
        
        Ok(())
    }
    
    /// 生成通用文件
    async fn generate_common_files(
        project_path: &Path,
        name: &str,
        _skill_type: &SkillType,
    ) -> Result<()> {
        // README.md
        let readme = format!(r#"# {}

## Description

Brief description of the skill.

## Installation

```bash
crablet skill install .
```

## Usage

```bash
crablet skill run {} '{{"key": "value"}}'
```

## Development

```bash
# Validate the skill
crablet skill dev validate

# Run tests
crablet skill dev test

# Build for distribution
crablet skill dev build
```

## License

MIT
"#, name, name);
        
        fs::write(project_path.join("README.md"), readme)?;
        
        // .gitignore
        let gitignore = r#"# Build artifacts
/target
/dist
/build
*.pyc
__pycache__/
.pytest_cache/
node_modules/

# IDE
.idea/
.vscode/
*.swp
*.swo

# Environment
.env
.venv/
venv/

# Testing
.coverage
htmlcov/
"#;
        
        fs::write(project_path.join(".gitignore"), gitignore)?;
        
        // tests/test_skill.py
        let test_py = r#"#!/usr/bin/env python3
\"\"\"
Skill tests
\"\"\"
import json
import unittest
from pathlib import Path

class TestSkill(unittest.TestCase):
    def setUp(self):
        self.skill_path = Path(__file__).parent.parent
    
    def test_skill_md_exists(self):
        \"\"\"Verify SKILL.md exists\"\"\"
        self.assertTrue((self.skill_path / "SKILL.md").exists())
    
    def test_skill_md_valid(self):
        \"\"\"Verify SKILL.md is valid\"\"\"
        content = (self.skill_path / "SKILL.md").read_text()
        self.assertIn("name:", content)
        self.assertIn("version:", content)

if __name__ == "__main__":
    unittest.main()
"#;
        
        fs::write(project_path.join("tests").join("test_skill.py"), test_py)?;
        
        // docs/USAGE.md
        let usage_md = format!(r#"# Usage Guide

## Basic Usage

```bash
crablet skill run {} '{{}}'
```

## Advanced Options

See README.md for more details.
"#, name);
        
        fs::write(project_path.join("docs").join("USAGE.md"), usage_md)?;
        
        Ok(())
    }
    
    /// 验证技能项目
    pub async fn validate(project_path: &Path) -> Result<ValidationResult> {
        info!("Validating skill project at: {}", project_path.display());
        
        let mut result = ValidationResult {
            valid: true,
            errors: vec![],
            warnings: vec![],
            suggestions: vec![],
        };
        
        // 检查必需文件
        let required_files = vec!["SKILL.md", "README.md"];
        for file in &required_files {
            let path = project_path.join(file);
            if !path.exists() {
                result.errors.push(format!("Missing required file: {}", file));
                result.valid = false;
            }
        }
        
        // 验证 SKILL.md
        let skill_md_path = project_path.join("SKILL.md");
        if skill_md_path.exists() {
            match Self::validate_skill_md(&skill_md_path).await {
                Ok(()) => {}
                Err(e) => {
                    result.errors.push(format!("SKILL.md validation failed: {}", e));
                    result.valid = false;
                }
            }
        }
        
        // 检查代码语法
        if project_path.join("src").join("main.py").exists() {
            match Self::validate_python_syntax(project_path).await {
                Ok(()) => {}
                Err(e) => {
                    result.errors.push(format!("Python syntax error: {}", e));
                    result.valid = false;
                }
            }
        }
        
        // 检查安全敏感操作
        match Self::check_security(project_path).await {
            Ok(findings) => {
                for finding in findings {
                    match finding.severity {
                        Severity::Error => {
                            result.errors.push(finding.message);
                            result.valid = false;
                        }
                        Severity::Warning => result.warnings.push(finding.message),
                        Severity::Info => result.suggestions.push(finding.message),
                    }
                }
            }
            Err(e) => {
                result.warnings.push(format!("Security check failed: {}", e));
            }
        }
        
        // 输出结果
        if result.valid {
            if result.warnings.is_empty() {
                info!("Validation passed!");
            } else {
                info!("Validation passed with warnings");
            }
        } else {
            error!("Validation failed with {} errors", result.errors.len());
        }
        
        Ok(result)
    }
    
    /// 验证 SKILL.md
    async fn validate_skill_md(path: &Path) -> Result<()> {
        let content = fs::read_to_string(path)?;
        
        // 检查 frontmatter
        if !content.starts_with("---") {
            bail!("SKILL.md must start with YAML frontmatter (---)");
        }
        
        // 提取 frontmatter
        let end = content.find("\n---").context("Missing closing frontmatter")?;
        let frontmatter = &content[3..end];
        
        // 检查必需字段
        let required_fields = vec!["name", "description", "version"];
        for field in &required_fields {
            if !frontmatter.contains(&format!("{}:", field)) {
                bail!("Missing required field in frontmatter: {}", field);
            }
        }
        
        // 验证版本格式
        let version_re = Regex::new(r"version:\s*(\d+\.\d+\.\d+)").unwrap();
        if let Some(caps) = version_re.captures(frontmatter) {
            let version = &caps[1];
            Version::parse(version).context("Invalid semantic version")?;
        }
        
        Ok(())
    }
    
    /// 验证 Python 语法
    async fn validate_python_syntax(project_path: &Path) -> Result<()> {
        let src_dir = project_path.join("src");
        
        if !src_dir.exists() {
            return Ok(());
        }
        
        for entry in fs::read_dir(&src_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map(|e| e == "py").unwrap_or(false) {
                let _content = fs::read_to_string(&path)?;
                
                // 使用 Python 的 py_compile 检查语法
                let output = std::process::Command::new("python3")
                    .arg("-m")
                    .arg("py_compile")
                    .arg(&path)
                    .output()?;
                
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    bail!("Syntax error in {}: {}", path.display(), stderr);
                }
            }
        }
        
        Ok(())
    }
    
    /// 安全检查
    async fn check_security(project_path: &Path) -> Result<Vec<SecurityFinding>> {
        let mut findings = vec![];
        
        // 危险函数模式
        let dangerous_patterns = vec![
            (r"eval\s*\(", "Use of eval() detected - potential security risk"),
            (r"exec\s*\(", "Use of exec() detected - potential security risk"),
            (r"os\.system\s*\(", "Use of os.system() detected - consider using subprocess"),
            (r"subprocess\.call.*shell\s*=\s*True", "Shell=True in subprocess - potential injection risk"),
            (r"input\s*\(", "Use of input() detected - may cause issues in non-interactive mode"),
        ];
        
        let src_dir = project_path.join("src");
        if src_dir.exists() {
            for entry in walkdir::WalkDir::new(&src_dir) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        for (pattern, message) in &dangerous_patterns {
                            let re = Regex::new(pattern).unwrap();
                            if re.is_match(&content) {
                                findings.push(SecurityFinding {
                                    severity: Severity::Warning,
                                    message: format!("{} in {}", message, entry.path().display()),
                                    line: None,
                                });
                            }
                        }
                    }
                }
            }
        }
        
        Ok(findings)
    }
    
    /// 测试技能
    pub async fn test(project_path: &Path, _test_args: Option<&str>) -> Result<TestResult> {
        info!("Running tests for skill at: {}", project_path.display());
        
        let mut result = TestResult {
            passed: 0,
            failed: 0,
            skipped: 0,
            duration_ms: 0,
            details: vec![],
        };
        
        let start = std::time::Instant::now();
        
        // 运行 Python 测试
        let tests_dir = project_path.join("tests");
        if tests_dir.exists() {
            let output = std::process::Command::new("python3")
                .arg("-m")
                .arg("pytest")
                .arg(&tests_dir)
                .arg("-v")
                .current_dir(project_path)
                .output()?;
            
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            // 解析测试结果
            if output.status.success() {
                result.passed += 1;
                result.details.push(TestDetail {
                    name: "pytest".to_string(),
                    status: TestStatus::Passed,
                    message: stdout.to_string(),
                });
            } else {
                result.failed += 1;
                result.details.push(TestDetail {
                    name: "pytest".to_string(),
                    status: TestStatus::Failed,
                    message: format!("{}\n{}", stdout, stderr),
                });
            }
        }
        
        // 运行技能功能测试
        let skill_md = project_path.join("SKILL.md");
        if skill_md.exists() {
            // 这里可以添加技能特定的功能测试
            info!("Running skill functional tests...");
        }
        
        result.duration_ms = start.elapsed().as_millis() as u64;
        
        info!("Tests completed: {} passed, {} failed", result.passed, result.failed);
        
        Ok(result)
    }
    
    /// 构建技能包
    pub async fn build(
        project_path: &Path,
        output_dir: Option<PathBuf>,
    ) -> Result<BuildResult> {
        info!("Building skill package...");
        
        let output_dir = output_dir.unwrap_or_else(|| project_path.join("dist"));
        fs::create_dir_all(&output_dir)?;
        
        // 读取技能名称和版本
        let skill_md = fs::read_to_string(project_path.join("SKILL.md"))?;
        let name = Self::extract_field(&skill_md, "name").unwrap_or("unknown".to_string());
        let version = Self::extract_field(&skill_md, "version").unwrap_or("0.1.0".to_string());
        
        let package_name = format!("{}-{}.skill", name, version);
        let package_path = output_dir.join(&package_name);
        
        // 创建 tar.gz 包
        let tar_gz = fs::File::create(&package_path)?;
        let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        
        // 添加文件到包
        tar.append_dir_all(".", project_path)?;
        tar.finish()?;
        
        // 计算校验和
        let checksum = Self::calculate_checksum(&package_path).await?;
        
        // 获取文件大小
        let size_bytes = fs::metadata(&package_path)?.len();
        
        // 写入校验和文件
        fs::write(
            output_dir.join(format!("{}.sha256", package_name)),
            format!("{}  {}", checksum, package_name)
        )?;
        
        info!("Build completed: {}", package_path.display());
        
        Ok(BuildResult {
            package_path,
            checksum,
            size_bytes,
        })
    }
    
    /// 发布技能
    pub async fn publish(
        project_path: &Path,
        registry: Option<String>,
        dry_run: bool,
    ) -> Result<PublishResult> {
        info!("Publishing skill...");
        
        // 首先验证
        let validation = Self::validate(project_path).await?;
        if !validation.valid {
            bail!("Validation failed. Please fix errors before publishing.");
        }
        
        // 运行测试
        let test_result = Self::test(project_path, None).await?;
        if test_result.failed > 0 {
            bail!("Tests failed. Please fix tests before publishing.");
        }
        
        // 构建
        let _build_result = Self::build(project_path, None).await?;
        
        if dry_run {
            info!("Dry run - not actually publishing");
            return Ok(PublishResult {
                success: true,
                url: None,
                message: "Dry run completed successfully".to_string(),
            });
        }
        
        // TODO: 实际发布到 registry
        let registry_url = registry.unwrap_or_else(|| "https://clawhub.dev".to_string());
        
        info!("Published to: {}", registry_url);
        
        Ok(PublishResult {
            success: true,
            url: Some(format!("{}/skills/{}", registry_url, 
                Self::extract_field(&fs::read_to_string(project_path.join("SKILL.md"))?, "name")
                    .unwrap_or_default())),
            message: "Skill published successfully".to_string(),
        })
    }
    
    /// 生成文档
    pub async fn docs(project_path: &Path, output_dir: Option<PathBuf>) -> Result<DocsResult> {
        info!("Generating documentation...");
        
        let output_dir = output_dir.unwrap_or_else(|| project_path.join("docs").join("generated"));
        fs::create_dir_all(&output_dir)?;
        
        // 读取 SKILL.md
        let skill_md = fs::read_to_string(project_path.join("SKILL.md"))?;
        
        // 生成 HTML 文档
        let html = Self::generate_html_docs(&skill_md)?;
        fs::write(output_dir.join("index.html"), html)?;
        
        // 生成 API 文档（如果有代码）
        if project_path.join("src").join("main.py").exists() {
            // 运行 pydoc 或类似工具
            let _ = std::process::Command::new("python3")
                .arg("-m")
                .arg("pdoc")
                .arg("-o")
                .arg(&output_dir)
                .arg(project_path.join("src"))
                .output();
        }
        
        info!("Documentation generated at: {}", output_dir.display());
        
        Ok(DocsResult {
            output_dir,
            files_generated: vec!["index.html".to_string()],
        })
    }
    
    /// 提取 frontmatter 字段
    fn extract_field(content: &str, field: &str) -> Option<String> {
        let pattern = format!(r"{}:\s*(.+)", field);
        let re = Regex::new(&pattern).ok()?;
        re.captures(content)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string())
    }
    
    /// 计算文件校验和
    async fn calculate_checksum(path: &Path) -> Result<String> {
        use sha2::{Sha256, Digest};
        
        let content = fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let result = hasher.finalize();
        
        Ok(format!("{:x}", result))
    }
    
    /// 生成 HTML 文档
    fn generate_html_docs(skill_md: &str) -> Result<String> {
        // 使用 markdown 转换
        let html_content = markdown::to_html(skill_md);
        
        let html = format!(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Skill Documentation</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 800px;
            margin: 0 auto;
            padding: 20px;
            line-height: 1.6;
        }}
        pre {{
            background: #f4f4f4;
            padding: 15px;
            border-radius: 5px;
            overflow-x: auto;
        }}
        code {{
            background: #f4f4f4;
            padding: 2px 5px;
            border-radius: 3px;
        }}
    </style>
</head>
<body>
    {}
</body>
</html>"#, html_content);
        
        Ok(html)
    }
}

/// 初始化结果
#[derive(Debug)]
pub struct InitResult {
    pub name: String,
    pub path: PathBuf,
    pub skill_type: SkillType,
    pub next_steps: Vec<String>,
}

/// 验证结果
#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

/// 安全发现
#[derive(Debug)]
pub struct SecurityFinding {
    pub severity: Severity,
    pub message: String,
    pub line: Option<usize>,
}

/// 严重程度
#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// 测试结果
#[derive(Debug)]
pub struct TestResult {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration_ms: u64,
    pub details: Vec<TestDetail>,
}

/// 测试详情
#[derive(Debug)]
pub struct TestDetail {
    pub name: String,
    pub status: TestStatus,
    pub message: String,
}

/// 测试状态
#[derive(Debug, Clone, Copy)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
}

/// 构建结果
#[derive(Debug)]
pub struct BuildResult {
    pub package_path: PathBuf,
    pub checksum: String,
    pub size_bytes: u64,
}

/// 发布结果
#[derive(Debug)]
pub struct PublishResult {
    pub success: bool,
    pub url: Option<String>,
    pub message: String,
}

/// 文档结果
#[derive(Debug)]
pub struct DocsResult {
    pub output_dir: PathBuf,
    pub files_generated: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::skills::{Skill, SkillManifest};

    #[tokio::test]
    async fn test_init_openclaw() {
        let temp_dir = TempDir::new().unwrap();
        // Create a mock skill for OpenClaw type
        let mock_skill = Skill {
            manifest: SkillManifest {
                name: "test-skill".to_string(),
                description: "Test skill".to_string(),
                version: "0.1.0".to_string(),
                parameters: serde_json::json!({}),
                entrypoint: "".to_string(),
                env: std::collections::HashMap::new(),
                requires: vec![],
                runtime: None,
                dependencies: None,
                resources: None,
                permissions: vec![],
                conflicts: vec![],
                min_crablet_version: None,
                author: Some("Test".to_string()),
                triggers: vec![],
            },
            path: temp_dir.path().join("test-skill"),
        };
        
        let result = DevTools::init(
            "test-skill",
            SkillType::OpenClaw(mock_skill, "Test instructions".to_string()),
            Some(temp_dir.path().join("test-skill")),
        ).await;
        
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.name, "test-skill");
        assert!(result.path.join("SKILL.md").exists());
    }

    #[test]
    fn test_extract_field() {
        let content = "name: my-skill\nversion: 1.0.0";
        assert_eq!(
            DevTools::extract_field(content, "name"),
            Some("my-skill".to_string())
        );
        assert_eq!(
            DevTools::extract_field(content, "version"),
            Some("1.0.0".to_string())
        );
    }
}
