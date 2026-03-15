//! 技能安装器 UI 模块
//!
//! 提供友好的命令行界面和进度反馈。

use std::io::{self, Write};

/// 安装进度显示
pub struct InstallProgress {
    step: usize,
    total_steps: usize,
    skill_name: String,
}

impl InstallProgress {
    pub fn new(skill_name: &str) -> Self {
        Self {
            step: 0,
            total_steps: 4, // Download, Validate, Install, Verify
            skill_name: skill_name.to_string(),
        }
    }

    pub fn next_step(&mut self, description: &str) {
        self.step += 1;
        let progress = (self.step * 100) / self.total_steps;
        let bar = self.render_progress_bar(progress);
        
        println!("\r{} [{}] {} {}/{}", 
            bar,
            self.skill_name,
            description,
            self.step,
            self.total_steps
        );
        io::stdout().flush().unwrap();
    }

    pub fn success(&self) {
        println!("\n✅ {} installed successfully!\n", self.skill_name);
    }

    pub fn error(&self, message: &str) {
        eprintln!("\n❌ Installation failed: {}\n", message);
    }

    fn render_progress_bar(&self, percentage: usize) -> String {
        let filled = (percentage as usize) / 10;
        let empty = 10 - filled;
        let filled_bar = "█".repeat(filled);
        let empty_bar = "░".repeat(empty);
        format!("[{}{}] {}%", filled_bar, empty_bar, percentage)
    }
}

/// 技能信息展示
pub struct SkillInfoDisplay;

impl SkillInfoDisplay {
    /// 显示技能详情
    pub fn show_skill_details(skill: &super::SkillManifest, path: &std::path::Path) {
        println!("\n📦 Skill Information");
        println!("{}", "═".repeat(50));
        println!("  Name:        {}", skill.name);
        println!("  Version:     {}", skill.version);
        println!("  Description: {}", skill.description);
        println!("  Entrypoint:  {}", skill.entrypoint);
        println!("  Location:    {:?}", path);
        
        if !skill.requires.is_empty() {
            println!("\n  System Dependencies:");
            for dep in &skill.requires {
                println!("    • {}", dep);
            }
        }
        
        if let Some(ref deps) = skill.dependencies {
            if !deps.pip.is_empty() {
                println!("\n  Python Dependencies:");
                for dep in &deps.pip {
                    println!("    • {}", dep);
                }
            }
            if !deps.npm.is_empty() {
                println!("\n  Node.js Dependencies:");
                for dep in &deps.npm {
                    println!("    • {}", dep);
                }
            }
        }
        
        if !skill.permissions.is_empty() {
            println!("\n  Permissions Required:");
            for perm in &skill.permissions {
                let icon = match perm.split(':').next() {
                    Some("network") => "🌐",
                    Some("filesystem") => "📁",
                    Some("env") => "🔧",
                    _ => "🔒",
                };
                println!("    {} {}", icon, perm);
            }
        }
        
        println!("{}", "═".repeat(50));
    }

    /// 显示安装摘要
    pub fn show_install_summary(results: &[super::atomic_installer::InstallResult]) {
        println!("\n📊 Installation Summary");
        println!("{}", "═".repeat(50));
        println!("  Total:    {}", results.len());
        println!("  Success:  {}", results.len());
        println!("  Failed:   0");
        println!("\n  Installed Skills:");
        for result in results {
            println!("    ✅ {} v{} at {:?}", 
                result.skill_name, 
                result.version,
                result.install_path
            );
        }
        println!("{}", "═".repeat(50));
    }
}

/// 用户确认提示
pub struct UserPrompt;

impl UserPrompt {
    /// 询问用户确认
    pub fn confirm(message: &str) -> bool {
        print!("{} [Y/n]: ", message);
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        
        let trimmed = input.trim().to_lowercase();
        trimmed == "y" || trimmed == "yes" || trimmed.is_empty()
    }

    /// 询问用户选择
    pub fn select(message: &str, options: &[String]) -> Option<usize> {
        println!("{}", message);
        for (i, option) in options.iter().enumerate() {
            println!("  {}. {}", i + 1, option);
        }
        
        print!("Select (1-{}): ", options.len());
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        
        input.trim().parse::<usize>().ok().map(|n| n.saturating_sub(1))
    }

    /// 输入字符串
    pub fn input(message: &str, default: Option<&str>) -> String {
        match default {
            Some(d) => print!("{} [{}]: ", message, d),
            None => print!("{}: ", message),
        }
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        
        let trimmed = input.trim();
        if trimmed.is_empty() {
            default.unwrap_or("").to_string()
        } else {
            trimmed.to_string()
        }
    }
}

/// 动画效果
pub struct Spinner {
    message: String,
    running: bool,
}

impl Spinner {
    pub fn new(message: &str) -> Self {
        let spinner = Self {
            message: message.to_string(),
            running: true,
        };
        
        // 在实际应用中，这里会启动一个异步任务来显示旋转动画
        print!("⏳ {}...", message);
        io::stdout().flush().unwrap();
        
        spinner
    }

    pub fn finish(&self, message: &str) {
        println!("\r✅ {} {}", self.message, message);
    }

    pub fn error(&self, message: &str) {
        eprintln!("\r❌ {} {}", self.message, message);
    }
}

/// 错误显示
pub struct ErrorDisplay;

impl ErrorDisplay {
    pub fn show_error(error: &anyhow::Error) {
        eprintln!("\n❌ Error: {}", error);
        
        // 尝试提取更有用的错误信息
        let error_string = error.to_string();
        
        if error_string.contains("git") {
            eprintln!("\n💡 Suggestions:");
            eprintln!("  • Make sure git is installed: git --version");
            eprintln!("  • Check your internet connection");
            eprintln!("  • Verify the URL is correct");
        } else if error_string.contains("already exists") {
            eprintln!("\n💡 Suggestions:");
            eprintln!("  • Use 'crablet skill update <name>' to update");
            eprintln!("  • Use 'crablet skill uninstall <name>' to remove first");
        } else if error_string.contains("manifest") {
            eprintln!("\n💡 Suggestions:");
            eprintln!("  • Ensure the repository contains skill.yaml or SKILL.md");
            eprintln!("  • Check the skill documentation for correct format");
        }
    }
}

/// 日志级别显示
pub struct LogDisplay;

impl LogDisplay {
    pub fn info(message: &str) {
        println!("ℹ️  {}", message);
    }

    pub fn success(message: &str) {
        println!("✅ {}", message);
    }

    pub fn warning(message: &str) {
        println!("⚠️  {}", message);
    }

    pub fn error(message: &str) {
        eprintln!("❌ {}", message);
    }

    pub fn debug(message: &str) {
        if std::env::var("CRABLET_DEBUG").is_ok() {
            println!("🐛 {}", message);
        }
    }
}
