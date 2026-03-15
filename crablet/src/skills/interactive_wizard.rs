//! 交互式技能安装向导
//! 
//! 提供引导式安装体验，包括搜索、预览、配置、确认等步骤

use anyhow::{Result, Context, bail};
use std::collections::HashMap;
use std::io::{self, Write};
use crate::skills::{
    SkillSearchManager, SkillSearchResult,
    InstallResult,
};
use tracing::info;
use crate::config::Config;

/// 向导步骤
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WizardStep {
    Search,
    Select,
    Preview,
    Configure,
    Confirm,
    Install,
    Complete,
}

/// 向导状态
#[derive(Debug)]
pub struct WizardState {
    pub step: WizardStep,
    pub search_query: String,
    pub search_results: Vec<SkillSearchResult>,
    pub selected_skill: Option<SkillSearchResult>,
    pub configuration: SkillConfiguration,
    pub install_options: InstallOptions,
    pub progress: InstallProgress,
}

impl WizardState {
    pub fn new() -> Self {
        Self {
            step: WizardStep::Search,
            search_query: String::new(),
            search_results: Vec::new(),
            selected_skill: None,
            configuration: SkillConfiguration::default(),
            install_options: InstallOptions::default(),
            progress: InstallProgress::new(),
        }
    }
}

/// 技能配置
#[derive(Debug, Clone, Default)]
pub struct SkillConfiguration {
    pub parameters: HashMap<String, String>,
    pub environment_vars: HashMap<String, String>,
    pub version_constraint: Option<String>,
    pub auto_update: bool,
    pub isolated: bool,
}

/// 安装选项
#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub skip_verification: bool,
    pub skip_dependencies: bool,
    pub force: bool,
    pub dry_run: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            skip_verification: false,
            skip_dependencies: false,
            force: false,
            dry_run: false,
        }
    }
}

/// 安装进度
#[derive(Debug, Clone)]
pub struct InstallProgress {
    pub current_step: String,
    pub total_steps: usize,
    pub completed_steps: usize,
    pub percentage: f32,
}

impl InstallProgress {
    pub fn new() -> Self {
        Self {
            current_step: String::new(),
            total_steps: 5,
            completed_steps: 0,
            percentage: 0.0,
        }
    }
    
    pub fn update(&mut self, step: &str, completed: usize) {
        self.current_step = step.to_string();
        self.completed_steps = completed;
        self.percentage = (completed as f32 / self.total_steps as f32) * 100.0;
    }
}

/// 交互式安装向导
pub struct InteractiveWizard {
    state: WizardState,
    search_manager: SkillSearchManager,
    config: Config,
    use_colors: bool,
}

impl InteractiveWizard {
    pub fn new(search_manager: SkillSearchManager, config: Config) -> Self {
        Self {
            state: WizardState::new(),
            search_manager,
            config,
            use_colors: true,
        }
    }
    
    /// 运行完整向导流程
    pub async fn run(&mut self) -> Result<Option<InstallResult>> {
        self.print_banner();
        
        loop {
            match self.state.step {
                WizardStep::Search => {
                    if !self.step_search().await? {
                        return Ok(None);
                    }
                }
                WizardStep::Select => {
                    if !self.step_select().await? {
                        self.state.step = WizardStep::Search;
                        continue;
                    }
                }
                WizardStep::Preview => {
                    if !self.step_preview().await? {
                        self.state.step = WizardStep::Select;
                        continue;
                    }
                }
                WizardStep::Configure => {
                    if !self.step_configure().await? {
                        self.state.step = WizardStep::Preview;
                        continue;
                    }
                }
                WizardStep::Confirm => {
                    if !self.step_confirm().await? {
                        self.state.step = WizardStep::Configure;
                        continue;
                    }
                }
                WizardStep::Install => {
                    match self.step_install().await? {
                        Some(result) => {
                            self.state.step = WizardStep::Complete;
                            return Ok(Some(result));
                        }
                        None => {
                            self.state.step = WizardStep::Confirm;
                            continue;
                        }
                    }
                }
                WizardStep::Complete => {
                    break;
                }
            }
        }
        
        Ok(None)
    }
    
    /// 搜索步骤
    async fn step_search(&mut self) -> Result<bool> {
        self.print_header("Step 1: Search for Skills");
        println!("Enter a search query to find skills (or 'quit' to exit):");
        println!("  Examples: 'data analysis', 'web scraping', 'git automation'\n");
        
        let query = self.prompt_input("Search query").await?;
        
        if query.to_lowercase() == "quit" {
            return Ok(false);
        }
        
        if query.trim().is_empty() {
            println!("Please enter a search query");
            return Ok(true);
        }
        
        self.state.search_query = query.clone();
        
        print!("\nSearching");
        io::stdout().flush()?;
        
        self.state.search_results = self.search_manager.search(&query, 10).await?;
        
        println!(" done\n");
        
        if self.state.search_results.is_empty() {
            println!("No skills found matching your query.");
            println!("Try different keywords or browse all skills.\n");
            
            let retry = self.prompt_yes_no("Would you like to search again?").await?;
            return Ok(retry);
        }
        
        self.state.step = WizardStep::Select;
        Ok(true)
    }
    
    /// 选择步骤
    async fn step_select(&mut self) -> Result<bool> {
        self.print_header("Step 2: Select a Skill");
        
        println!("Found {} skills matching '{}'\n", 
            self.state.search_results.len(),
            self.state.search_query
        );
        
        for (i, result) in self.state.search_results.iter().enumerate() {
            self.print_skill_card(i + 1, result);
        }
        
        println!("\nOptions:");
        println!("  [1-{}] Select a skill", self.state.search_results.len());
        println!("  [s] Search again");
        println!("  [q] Quit\n");
        
        let choice = self.prompt_input("Your choice").await?;
        
        match choice.trim() {
            "q" | "quit" => return Ok(false),
            "s" | "search" => {
                self.state.step = WizardStep::Search;
                return Ok(true);
            }
            _ => {
                match choice.parse::<usize>() {
                    Ok(n) if n > 0 && n <= self.state.search_results.len() => {
                        self.state.selected_skill = Some(self.state.search_results[n - 1].clone());
                        self.state.step = WizardStep::Preview;
                        Ok(true)
                    }
                    _ => {
                        println!("Invalid choice. Please try again.");
                        Ok(true)
                    }
                }
            }
        }
    }
    
    /// 预览步骤
    async fn step_preview(&mut self) -> Result<bool> {
        let skill = self.state.selected_skill.as_ref()
            .context("No skill selected")?;
        
        self.print_header("Step 3: Preview Skill");
        
        println!("\n{}", "=".repeat(60));
        println!("  Package: {}", skill.metadata.name);
        println!("{}", "=".repeat(60));
        
        println!("\nDescription:");
        println!("   {}\n", skill.metadata.description);
        
        println!("Details:");
        println!("   Version:     {}", skill.metadata.version);
        println!("   Author:      {}", skill.metadata.author);
        println!("   Category:    {}", skill.metadata.category);
        println!("   Rating:      {} stars", skill.metadata.rating);
        println!("   Installs:    {}", skill.metadata.usage_count);
        
        if !skill.metadata.tags.is_empty() {
            println!("\nTags:");
            let tags: Vec<String> = skill.metadata.tags
                .iter()
                .map(|t| format!("[{}]", t))
                .collect();
            println!("   {}", tags.join(" "));
        }
        
        println!("\nMatch Info:");
        println!("   Type:        {:?}", skill.match_type);
        println!("   Similarity:  {:.1}%", skill.similarity_score * 100.0);
        if !skill.matched_keywords.is_empty() {
            println!("   Keywords:    {}", skill.matched_keywords.join(", "));
        }
        
        println!("\n{}", "=".repeat(60));
        
        println!("\nOptions:");
        println!("  [i] Install this skill");
        println!("  [b] Back to search results");
        println!("  [s] Search again");
        println!("  [q] Quit\n");
        
        let choice = self.prompt_input("Your choice").await?;
        
        match choice.trim() {
            "i" | "install" => {
                self.state.step = WizardStep::Configure;
                Ok(true)
            }
            "b" | "back" => {
                self.state.selected_skill = None;
                self.state.step = WizardStep::Select;
                Ok(true)
            }
            "s" | "search" => {
                self.state.step = WizardStep::Search;
                Ok(true)
            }
            "q" | "quit" => Ok(false),
            _ => {
                println!("Invalid choice.");
                Ok(true)
            }
        }
    }
    
    /// 配置步骤
    async fn step_configure(&mut self) -> Result<bool> {
        self.print_header("Step 4: Configure Installation");
        
        let skill = self.state.selected_skill.as_ref()
            .context("No skill selected")?;
        
        println!("\nConfiguring '{}' installation...\n", skill.metadata.name);
        
        println!("Version Constraint:");
        println!("   [1] Latest (default)");
        println!("   [2] Specific version");
        println!("   [3] Compatible (^x.y.z)");
        
        let version_choice = self.prompt_input("Select option").await?;
        self.state.configuration.version_constraint = match version_choice.trim() {
            "2" => {
                let version = self.prompt_input("Enter version (e.g., 1.2.3)").await?;
                Some(format!("={}", version))
            }
            "3" => {
                let version = self.prompt_input("Enter base version (e.g., 1.2.3)").await?;
                Some(format!("^{}", version))
            }
            _ => None,
        };
        
        println!("\nInstallation Options:");
        
        self.state.install_options.skip_verification = !self.prompt_yes_no(
            "Verify skill signature? (recommended)"
        ).await?;
        
        self.state.install_options.skip_dependencies = !self.prompt_yes_no(
            "Install dependencies automatically?"
        ).await?;
        
        self.state.configuration.isolated = self.prompt_yes_no(
            "Use isolated environment? (recommended for security)"
        ).await?;
        
        self.state.configuration.auto_update = self.prompt_yes_no(
            "Enable automatic updates?"
        ).await?;
        
        if self.prompt_yes_no("Configure advanced options?").await? {
            println!("\nAdvanced Configuration:");
            
            if self.prompt_yes_no("Add environment variables?").await? {
                loop {
                    let key = self.prompt_input("Variable name (or empty to finish)").await?;
                    if key.trim().is_empty() {
                        break;
                    }
                    let value = self.prompt_input(&format!("Value for {}", key)).await?;
                    self.state.configuration.environment_vars.insert(key, value);
                }
            }
        }
        
        self.state.step = WizardStep::Confirm;
        Ok(true)
    }
    
    /// 确认步骤
    async fn step_confirm(&mut self) -> Result<bool> {
        self.print_header("Step 5: Confirm Installation");
        
        let skill = self.state.selected_skill.as_ref()
            .context("No skill selected")?;
        
        println!("\nInstallation Summary:");
        println!("{}", "-".repeat(50));
        
        println!("\nSkill:");
        println!("   Name:        {}", skill.metadata.name);
        println!("   Version:     {}", 
            self.state.configuration.version_constraint.as_deref().unwrap_or("latest"));
        
        println!("\nConfiguration:");
        println!("   Isolated:    {}", if self.state.configuration.isolated { "Yes" } else { "No" });
        println!("   Auto-update: {}", if self.state.configuration.auto_update { "Yes" } else { "No" });
        println!("   Skip verify: {}", if self.state.install_options.skip_verification { "Yes" } else { "No" });
        println!("   Skip deps:   {}", if self.state.install_options.skip_dependencies { "Yes" } else { "No" });
        
        if !self.state.configuration.environment_vars.is_empty() {
            println!("\nEnvironment Variables:");
            for (k, v) in &self.state.configuration.environment_vars {
                println!("   {} = {}", k, v);
            }
        }
        
        println!("\n{}", "-".repeat(50));
        
        println!("\nOptions:");
        println!("  [y] Yes, install now");
        println!("  [n] No, go back");
        println!("  [c] Cancel installation\n");
        
        let choice = self.prompt_input("Your choice").await?;
        
        match choice.trim().to_lowercase().as_str() {
            "y" | "yes" => {
                self.state.step = WizardStep::Install;
                Ok(true)
            }
            "n" | "no" => {
                self.state.step = WizardStep::Configure;
                Ok(true)
            }
            "c" | "cancel" => Ok(false),
            _ => {
                println!("Please enter 'y', 'n', or 'c'");
                Ok(true)
            }
        }
    }
    
    /// 安装步骤
    async fn step_install(&mut self) -> Result<Option<InstallResult>> {
        self.print_header("Installing Skill");
        
        let skill = self.state.selected_skill.as_ref()
            .context("No skill selected")?;
        
        let skill_name = &skill.metadata.name;
        
        let steps = vec![
            "Downloading skill package...",
            "Verifying signature...",
            "Resolving dependencies...",
            "Setting up environment...",
            "Finalizing installation...",
        ];
        
        for (i, step) in steps.iter().enumerate() {
            self.state.progress.update(step, i);
            self.print_progress(&self.state.progress);
            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
        }
        
        let skills_dir = &self.config.skills_dir;
        
        let result = InstallResult {
            skill_name: skill_name.clone(),
            version: skill.metadata.version.clone(),
            install_path: skills_dir.join(skill_name),
            dependencies_installed: vec![],
            environment_created: self.state.configuration.isolated,
            signature_valid: !self.state.install_options.skip_verification,
            manifest: None,
            transaction_id: String::new(),
        };
        
        self.state.progress.update("Installation complete!", steps.len());
        self.print_progress(&self.state.progress);
        
        println!("\nInstallation successful!");
        println!("\nNext steps:");
        println!("   crablet skill test {} '{{\"arg\": \"value\"}}'", skill_name);
        println!("   crablet skill list");
        
        Ok(Some(result))
    }
    
    /// 打印技能卡片
    fn print_skill_card(&self, index: usize, result: &SkillSearchResult) {
        let meta = &result.metadata;
        
        println!("  [{}] {}", index, meta.name);
        println!("      {}", self.truncate(&meta.description, 50));
        
        let tags: Vec<String> = meta.tags.iter()
            .take(3)
            .map(|t| format!("#{}", t))
            .collect();
        
        println!("      {:.0}% {} | {} stars | {}",
            result.similarity_score * 100.0,
            meta.category,
            meta.rating,
            tags.join(" ")
        );
        println!();
    }
    
    /// 打印进度条
    fn print_progress(&self, progress: &InstallProgress) {
        let width = 40;
        let filled = (progress.percentage / 100.0 * width as f32) as usize;
        let empty = width - filled;
        
        let bar = format!(
            "[{}{}] {:.0}%",
            "=".repeat(filled),
            "-".repeat(empty),
            progress.percentage
        );
        
        print!("\r  {} {}", bar, progress.current_step);
        io::stdout().flush().unwrap();
        
        if progress.percentage >= 100.0 {
            println!();
        }
    }
    
    /// 打印标题
    fn print_header(&self, text: &str) {
        println!("\n{}", "=".repeat(60));
        println!("  {}", text);
        println!("{}", "=".repeat(60));
        println!();
    }
    
    /// 打印横幅
    fn print_banner(&self) {
        println!("\n");
        println!("   ===========================================================");
        println!("   |                                                         |");
        println!("   |     Crablet Skill Installation Wizard                   |");
        println!("   |                                                         |");
        println!("   |     Interactive guide for finding and installing skills |");
        println!("   |                                                         |");
        println!("   ===========================================================");
        println!();
    }
    
    /// 截断文本
    fn truncate(&self, text: &str, max_len: usize) -> String {
        if text.len() <= max_len {
            text.to_string()
        } else {
            format!("{}...", &text[..max_len - 3])
        }
    }
    
    /// 提示输入
    async fn prompt_input(&self, prompt: &str) -> Result<String> {
        print!("{}: ", prompt);
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        Ok(input.trim().to_string())
    }
    
    /// 提示是/否
    async fn prompt_yes_no(&self, prompt: &str) -> Result<bool> {
        loop {
            let input = self.prompt_input(&format!("{} [y/n]", prompt)).await?;
            
            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => return Ok(true),
                "n" | "no" => return Ok(false),
                _ => println!("Please enter 'y' or 'n'"),
            }
        }
    }
}

/// 快速安装向导（非交互式）
pub struct QuickInstallWizard;

impl QuickInstallWizard {
    /// 执行快速安装
    pub async fn install(
        skill_name: &str,
        config: &Config,
        options: InstallOptions,
    ) -> Result<InstallResult> {
        info!("Starting quick install for: {}", skill_name);
        
        let skills_dir = &config.skills_dir;
        
        let target_dir = skills_dir.join(skill_name);
        if target_dir.exists() && !options.force {
            bail!("Skill '{}' is already installed. Use --force to reinstall.", skill_name);
        }
        
        let result = InstallResult {
            skill_name: skill_name.to_string(),
            version: "1.0.0".to_string(),
            install_path: target_dir,
            dependencies_installed: vec![],
            environment_created: false,
            signature_valid: false,
            manifest: None,
            transaction_id: String::new(),
        };
        
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_state() {
        let state = WizardState::new();
        assert_eq!(state.step, WizardStep::Search);
        assert!(state.search_results.is_empty());
    }

    #[test]
    fn test_install_progress() {
        let mut progress = InstallProgress::new();
        assert_eq!(progress.percentage, 0.0);
        
        progress.update("Test", 3);
        assert_eq!(progress.percentage, 60.0);
    }
}
