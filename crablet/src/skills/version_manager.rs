//! 技能版本管理模块
//! 
//! 支持版本解析、约束检查、更新检测、依赖解析

use anyhow::{Result, Context, bail};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};
use regex::Regex;
use chrono::{DateTime, Utc};

/// 语义化版本
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub prerelease: Option<String>,
    pub build: Option<String>,
}

impl SemVer {
    /// 解析版本字符串
    pub fn parse(version: &str) -> Result<Self> {
        let version = version.trim_start_matches('v');
        
        // 语义化版本正则: MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]
        let re = Regex::new(r"^(\d+)\.(\d+)\.(\d+)(?:-([a-zA-Z0-9.]+))?(?:\+([a-zA-Z0-9.]+))?$")
            .context("Failed to compile semver regex")?;
        
        let caps = re.captures(version)
            .context(format!("Invalid semantic version: {}", version))?;
        
        let major = caps[1].parse()?;
        let minor = caps[2].parse()?;
        let patch = caps[3].parse()?;
        let prerelease = caps.get(4).map(|m| m.as_str().to_string());
        let build = caps.get(5).map(|m| m.as_str().to_string());
        
        Ok(Self {
            major,
            minor,
            patch,
            prerelease,
            build,
        })
    }

    /// 转换为字符串
    pub fn to_string(&self) -> String {
        let mut s = format!("{}.{}.{}", self.major, self.minor, self.patch);
        if let Some(ref pre) = self.prerelease {
            s.push_str(&format!("-{}", pre));
        }
        if let Some(ref build) = self.build {
            s.push_str(&format!("+{}", build));
        }
        s
    }

    /// 是否为预发布版本
    pub fn is_prerelease(&self) -> bool {
        self.prerelease.is_some()
    }

    /// 获取下一个主版本
    pub fn next_major(&self) -> Self {
        Self {
            major: self.major + 1,
            minor: 0,
            patch: 0,
            prerelease: None,
            build: None,
        }
    }

    /// 获取下一个次版本
    pub fn next_minor(&self) -> Self {
        Self {
            major: self.major,
            minor: self.minor + 1,
            patch: 0,
            prerelease: None,
            build: None,
        }
    }

    /// 获取下一个补丁版本
    pub fn next_patch(&self) -> Self {
        Self {
            major: self.major,
            minor: self.minor,
            patch: self.patch + 1,
            prerelease: None,
            build: None,
        }
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        // 先比较主、次、补丁版本
        match self.major.cmp(&other.major) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.minor.cmp(&other.minor) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.patch.cmp(&other.patch) {
            Ordering::Equal => {}
            ord => return ord,
        }
        
        // 预发布版本比较
        match (&self.prerelease, &other.prerelease) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater, // 正式版 > 预发布版
            (Some(_), None) => Ordering::Less,
            (Some(a), Some(b)) => compare_prerelease(a, b),
        }
    }
}

/// 比较预发布版本
fn compare_prerelease(a: &str, b: &str) -> Ordering {
    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();
    
    for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
        // 尝试作为数字比较
        match (a_part.parse::<u64>(), b_part.parse::<u64>()) {
            (Ok(a_num), Ok(b_num)) => {
                match a_num.cmp(&b_num) {
                    Ordering::Equal => continue,
                    ord => return ord,
                }
            }
            (Ok(_), Err(_)) => return Ordering::Less, // 数字 < 字符串
            (Err(_), Ok(_)) => return Ordering::Greater,
            (Err(_), Err(_)) => {
                // 字符串比较
                match a_part.cmp(b_part) {
                    Ordering::Equal => continue,
                    ord => return ord,
                }
            }
        }
    }
    
    // 较短的预发布版本较小
    a_parts.len().cmp(&b_parts.len())
}

/// 版本约束
#[derive(Debug, Clone)]
pub enum VersionConstraint {
    /// 精确版本 =1.2.3
    Exact(SemVer),
    /// 大于 >1.2.3
    GreaterThan(SemVer),
    /// 大于等于 >=1.2.3
    GreaterThanOrEqual(SemVer),
    /// 小于 <1.2.3
    LessThan(SemVer),
    /// 小于等于 <=1.2.3
    LessThanOrEqual(SemVer),
    /// 兼容版本 ^1.2.3 (允许次版本和补丁更新)
    Compatible(SemVer),
    /// 近似版本 ~1.2.3 (允许补丁更新)
    Approximate(SemVer),
    /// 通配符 1.2.x 或 1.x
    Wildcard { major: u64, minor: Option<u64> },
    /// 范围 1.2.3 - 2.0.0
    Range(SemVer, SemVer),
    /// 任意版本 *
    Any,
    /// 复合约束 (与)
    And(Vec<VersionConstraint>),
    /// 复合约束 (或)
    Or(Vec<VersionConstraint>),
}

impl VersionConstraint {
    /// 解析约束字符串
    pub fn parse(constraint: &str) -> Result<Self> {
        let constraint = constraint.trim();
        
        // 任意版本
        if constraint == "*" || constraint == "x" || constraint == "X" {
            return Ok(Self::Any);
        }
        
        // 范围: 1.2.3 - 2.0.0
        if constraint.contains(" - ") {
            let parts: Vec<&str> = constraint.split(" - ").collect();
            if parts.len() == 2 {
                let min = SemVer::parse(parts[0].trim())?;
                let max = SemVer::parse(parts[1].trim())?;
                return Ok(Self::Range(min, max));
            }
        }
        
        // 复合约束 (或): 1.2.3 || 2.0.0
        if constraint.contains("||") {
            let parts: Vec<&str> = constraint.split("||").collect();
            let constraints: Result<Vec<_>> = parts
                .iter()
                .map(|p| Self::parse(p.trim()))
                .collect();
            return Ok(Self::Or(constraints?));
        }
        
        // 复合约束 (与): >=1.2.3 <2.0.0
        if constraint.contains(' ') && !constraint.starts_with('~') && !constraint.starts_with('^') {
            let parts: Vec<&str> = constraint.split_whitespace().collect();
            let constraints: Result<Vec<_>> = parts
                .iter()
                .map(|p| Self::parse_single(p.trim()))
                .collect();
            return Ok(Self::And(constraints?));
        }
        
        Self::parse_single(constraint)
    }
    
    fn parse_single(constraint: &str) -> Result<Self> {
        // 兼容版本: ^1.2.3
        if let Some(version) = constraint.strip_prefix('^') {
            return Ok(Self::Compatible(SemVer::parse(version)?));
        }
        
        // 近似版本: ~1.2.3
        if let Some(version) = constraint.strip_prefix('~') {
            return Ok(Self::Approximate(SemVer::parse(version)?));
        }
        
        // 大于等于: >=1.2.3
        if let Some(version) = constraint.strip_prefix(">=") {
            return Ok(Self::GreaterThanOrEqual(SemVer::parse(version)?));
        }
        
        // 小于等于: <=1.2.3
        if let Some(version) = constraint.strip_prefix("<=") {
            return Ok(Self::LessThanOrEqual(SemVer::parse(version)?));
        }
        
        // 大于: >1.2.3
        if let Some(version) = constraint.strip_prefix('>') {
            return Ok(Self::GreaterThan(SemVer::parse(version)?));
        }
        
        // 小于: <1.2.3
        if let Some(version) = constraint.strip_prefix('<') {
            return Ok(Self::LessThan(SemVer::parse(version)?));
        }
        
        // 精确版本: =1.2.3 或 1.2.3
        let version = constraint.strip_prefix('=').unwrap_or(constraint);
        
        // 检查通配符: 1.2.x 或 1.x
        if version.contains('x') || version.contains('X') || version.contains('*') {
            return Self::parse_wildcard(version);
        }
        
        Ok(Self::Exact(SemVer::parse(version)?))
    }
    
    fn parse_wildcard(pattern: &str) -> Result<Self> {
        let parts: Vec<&str> = pattern.split('.').collect();
        
        let major = parts[0].parse()?;
        let minor = if parts.len() > 1 && parts[1] != "x" && parts[1] != "X" && parts[1] != "*" {
            Some(parts[1].parse()?)
        } else {
            None
        };
        
        Ok(Self::Wildcard { major, minor })
    }
    
    /// 检查版本是否满足约束
    pub fn matches(&self, version: &SemVer) -> bool {
        match self {
            Self::Exact(v) => version == v,
            Self::GreaterThan(v) => version > v,
            Self::GreaterThanOrEqual(v) => version >= v,
            Self::LessThan(v) => version < v,
            Self::LessThanOrEqual(v) => version <= v,
            Self::Compatible(v) => {
                // ^1.2.3 允许 >=1.2.3 <2.0.0
                version >= v && version.major == v.major
            }
            Self::Approximate(v) => {
                // ~1.2.3 允许 >=1.2.3 <1.3.0
                version >= v && version.major == v.major && version.minor == v.minor
            }
            Self::Wildcard { major, minor } => {
                if version.major != *major {
                    return false;
                }
                if let Some(min) = minor {
                    return version.minor == *min;
                }
                true
            }
            Self::Range(min, max) => version >= min && version <= max,
            Self::Any => true,
            Self::And(constraints) => constraints.iter().all(|c| c.matches(version)),
            Self::Or(constraints) => constraints.iter().any(|c| c.matches(version)),
        }
    }
    
    /// 转换为字符串表示
    pub fn to_string(&self) -> String {
        match self {
            Self::Exact(v) => format!("={}", v),
            Self::GreaterThan(v) => format!(">{}", v),
            Self::GreaterThanOrEqual(v) => format!(">={}", v),
            Self::LessThan(v) => format!("<{}", v),
            Self::LessThanOrEqual(v) => format!("<={}", v),
            Self::Compatible(v) => format!("^{}", v),
            Self::Approximate(v) => format!("~{}", v),
            Self::Wildcard { major, minor: None } => format!("{}.*", major),
            Self::Wildcard { major, minor: Some(m) } => format!("{}.{}.*", major, m),
            Self::Range(min, max) => format!("{} - {}", min, max),
            Self::Any => "*".to_string(),
            Self::And(constraints) => constraints.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" "),
            Self::Or(constraints) => constraints.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" || "),
        }
    }
}

/// 技能版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillVersionInfo {
    pub name: String,
    pub current_version: SemVer,
    pub latest_version: Option<SemVer>,
    pub installed_at: DateTime<Utc>,
    pub last_checked: Option<DateTime<Utc>>,
    pub update_available: bool,
    pub changelog: Option<String>,
}

/// 版本差异类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionDiff {
    /// 主版本更新 (破坏性变更)
    Major,
    /// 次版本更新 (新功能)
    Minor,
    /// 补丁更新 (Bug 修复)
    Patch,
    /// 预发布版本
    Prerelease,
    /// 无差异
    None,
}

impl VersionDiff {
    /// 计算两个版本的差异
    pub fn between(current: &SemVer, latest: &SemVer) -> Self {
        if current.major != latest.major {
            return Self::Major;
        }
        if current.minor != latest.minor {
            return Self::Minor;
        }
        if current.patch != latest.patch {
            return Self::Patch;
        }
        if current.prerelease != latest.prerelease {
            return Self::Prerelease;
        }
        Self::None
    }
    
    /// 获取更新优先级
    pub fn priority(&self) -> u8 {
        match self {
            Self::Major => 4,
            Self::Minor => 3,
            Self::Patch => 2,
            Self::Prerelease => 1,
            Self::None => 0,
        }
    }
    
    /// 是否为破坏性更新
    pub fn is_breaking(&self) -> bool {
        matches!(self, Self::Major)
    }
    
    /// 获取描述
    pub fn description(&self) -> &'static str {
        match self {
            Self::Major => "Major update (breaking changes)",
            Self::Minor => "Minor update (new features)",
            Self::Patch => "Patch update (bug fixes)",
            Self::Prerelease => "Prerelease update",
            Self::None => "No updates available",
        }
    }
}

/// 版本管理器
pub struct VersionManager {
    /// 已安装技能的版本信息
    installed: HashMap<String, SkillVersionInfo>,
    /// 技能目录
    skills_dir: std::path::PathBuf,
}

impl VersionManager {
    pub fn new(skills_dir: std::path::PathBuf) -> Self {
        Self {
            installed: HashMap::new(),
            skills_dir,
        }
    }
    
    /// 加载已安装技能信息
    pub async fn load(&mut self) -> Result<()> {
        let version_file = self.skills_dir.join(".versions.json");
        
        if version_file.exists() {
            let content = tokio::fs::read_to_string(&version_file).await?;
            let versions: HashMap<String, SkillVersionInfo> = serde_json::from_str(&content)?;
            self.installed = versions;
        }
        
        // 扫描目录更新版本信息
        self.scan_directory().await?;
        
        Ok(())
    }
    
    /// 扫描技能目录
    async fn scan_directory(&mut self) -> Result<()> {
        if !self.skills_dir.exists() {
            return Ok(());
        }
        
        let mut entries = tokio::fs::read_dir(&self.skills_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                
                // 跳过隐藏目录
                if name.starts_with('.') {
                    continue;
                }
                
                // 尝试读取版本
                if let Some(version) = self.read_skill_version(&path).await {
                    let info = SkillVersionInfo {
                        name: name.to_string(),
                        current_version: version,
                        latest_version: None,
                        installed_at: Utc::now(),
                        last_checked: None,
                        update_available: false,
                        changelog: None,
                    };
                    
                    self.installed.insert(name.to_string(), info);
                }
            }
        }
        
        Ok(())
    }
    
    /// 读取技能版本
    async fn read_skill_version(&self, skill_path: &Path) -> Option<SemVer> {
        // 尝试从 SKILL.md 解析
        let skill_md = skill_path.join("SKILL.md");
        if skill_md.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&skill_md).await {
                if let Some(version) = self.parse_version_from_skill_md(&content) {
                    return Some(version);
                }
            }
        }
        
        // 尝试从 package.json 读取
        let package_json = skill_path.join("package.json");
        if package_json.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&package_json).await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(version) = json.get("version").and_then(|v| v.as_str()) {
                        if let Ok(semver) = SemVer::parse(version) {
                            return Some(semver);
                        }
                    }
                }
            }
        }
        
        // 尝试从 Cargo.toml 读取
        let cargo_toml = skill_path.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&cargo_toml).await {
                for line in content.lines() {
                    if line.starts_with("version") {
                        if let Some(version) = line.split('=').nth(1) {
                            let version = version.trim().trim_matches('"').trim_matches('\'');
                            if let Ok(semver) = SemVer::parse(version) {
                                return Some(semver);
                            }
                        }
                    }
                }
            }
        }
        
        None
    }
    
    fn parse_version_from_skill_md(&self, content: &str) -> Option<SemVer> {
        if content.starts_with("---") {
            if let Some(end) = content.find("\n---") {
                let frontmatter = &content[3..end];
                for line in frontmatter.lines() {
                    if let Some((key, value)) = line.split_once(':') {
                        if key.trim() == "version" {
                            let version = value.trim().trim_matches('"').trim_matches('\'');
                            if let Ok(semver) = SemVer::parse(version) {
                                return Some(semver);
                            }
                        }
                    }
                }
            }
        }
        None
    }
    
    /// 检查更新
    pub async fn check_updates(&mut self) -> Result<Vec<UpdateInfo>> {
        let mut updates = Vec::new();
        
        // 先收集所有技能名称，避免同时借用
        let skill_names: Vec<String> = self.installed.keys().cloned().collect();
        
        for name in skill_names {
            if let Some(info) = self.installed.get(&name) {
                let current_version = info.current_version.clone();
                let changelog = info.changelog.clone();
                
                match self.fetch_latest_version(&name).await {
                    Ok(latest) => {
                        let diff = VersionDiff::between(&current_version, &latest);
                        
                        if diff != VersionDiff::None {
                            // 获取可变引用更新信息
                            if let Some(info_mut) = self.installed.get_mut(&name) {
                                info_mut.latest_version = Some(latest.clone());
                                info_mut.update_available = true;
                                info_mut.last_checked = Some(Utc::now());
                            }
                            
                            updates.push(UpdateInfo {
                                skill_name: name.clone(),
                                current_version: current_version.clone(),
                                latest_version: latest.clone(),
                                diff: diff.clone(),
                                changelog: changelog.clone(),
                            });
                            
                            info!("Update available for {}: {} -> {} ({:?})", 
                                name, current_version, latest, diff);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to check updates for {}: {}", name, e);
                    }
                }
            }
        }
        
        // 保存更新后的信息
        self.save().await?;
        
        // 按优先级排序
        updates.sort_by(|a, b| b.diff.priority().cmp(&a.diff.priority()));
        
        Ok(updates)
    }
    
    /// 获取最新版本（从远程）
    async fn fetch_latest_version(&self, skill_name: &str) -> Result<SemVer> {
        // TODO: 实现从远程 registry 获取最新版本
        // 目前返回模拟数据
        
        // 模拟网络请求延迟
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // 模拟返回更高版本
        if let Some(info) = self.installed.get(skill_name) {
            let next_patch = info.current_version.next_patch();
            return Ok(next_patch);
        }
        
        bail!("Skill not found: {}", skill_name)
    }
    
    /// 保存版本信息
    async fn save(&self) -> Result<()> {
        let version_file = self.skills_dir.join(".versions.json");
        let content = serde_json::to_string_pretty(&self.installed)?;
        tokio::fs::write(&version_file, content).await?;
        Ok(())
    }
    
    /// 获取技能版本信息
    pub fn get_version_info(&self, skill_name: &str) -> Option<&SkillVersionInfo> {
        self.installed.get(skill_name)
    }
    
    /// 列出所有已安装技能
    pub fn list_installed(&self) -> Vec<&SkillVersionInfo> {
        self.installed.values().collect()
    }
    
    /// 检查是否有可用更新
    pub fn has_updates(&self) -> bool {
        self.installed.values().any(|info| info.update_available)
    }
    
    /// 获取更新统计
    pub fn get_update_stats(&self) -> UpdateStats {
        let mut stats = UpdateStats::default();
        
        for info in self.installed.values() {
            if let Some(ref latest) = info.latest_version {
                let diff = VersionDiff::between(&info.current_version, latest);
                match diff {
                    VersionDiff::Major => stats.major += 1,
                    VersionDiff::Minor => stats.minor += 1,
                    VersionDiff::Patch => stats.patch += 1,
                    VersionDiff::Prerelease => stats.prerelease += 1,
                    VersionDiff::None => {}
                }
            }
        }
        
        stats.total = stats.major + stats.minor + stats.patch + stats.prerelease;
        stats
    }
    
    /// 更新技能版本记录
    pub fn record_update(&mut self, skill_name: &str, new_version: SemVer) {
        if let Some(info) = self.installed.get_mut(skill_name) {
            info.current_version = new_version;
            info.latest_version = None;
            info.update_available = false;
            info.last_checked = Some(Utc::now());
        }
    }
}

/// 更新信息
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub skill_name: String,
    pub current_version: SemVer,
    pub latest_version: SemVer,
    pub diff: VersionDiff,
    pub changelog: Option<String>,
}

/// 更新统计
#[derive(Debug, Clone, Default)]
pub struct UpdateStats {
    pub total: usize,
    pub major: usize,
    pub minor: usize,
    pub patch: usize,
    pub prerelease: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_parsing() {
        let v = SemVer::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        
        let v = SemVer::parse("v2.0.0-alpha.1+build.123").unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.prerelease, Some("alpha.1".to_string()));
        assert_eq!(v.build, Some("build.123".to_string()));
    }

    #[test]
    fn test_semver_comparison() {
        let v1 = SemVer::parse("1.0.0").unwrap();
        let v2 = SemVer::parse("1.0.1").unwrap();
        assert!(v1 < v2);
        
        let v3 = SemVer::parse("2.0.0").unwrap();
        assert!(v2 < v3);
        
        let v4 = SemVer::parse("1.0.0-alpha").unwrap();
        assert!(v4 < v1); // 预发布版本 < 正式版本
    }

    #[test]
    fn test_version_constraint() {
        let v = SemVer::parse("1.2.3").unwrap();
        
        // 精确匹配
        let c = VersionConstraint::parse("=1.2.3").unwrap();
        assert!(c.matches(&v));
        
        // 兼容版本
        let c = VersionConstraint::parse("^1.0.0").unwrap();
        assert!(c.matches(&v));
        
        let v2 = SemVer::parse("2.0.0").unwrap();
        assert!(!c.matches(&v2)); // 主版本不匹配
        
        // 通配符
        let c = VersionConstraint::parse("1.*").unwrap();
        assert!(c.matches(&v));
        assert!(!c.matches(&v2));
    }

    #[test]
    fn test_version_diff() {
        let v1 = SemVer::parse("1.0.0").unwrap();
        let v2 = SemVer::parse("2.0.0").unwrap();
        assert_eq!(VersionDiff::between(&v1, &v2), VersionDiff::Major);
        
        let v3 = SemVer::parse("1.1.0").unwrap();
        assert_eq!(VersionDiff::between(&v1, &v3), VersionDiff::Minor);
        
        let v4 = SemVer::parse("1.0.1").unwrap();
        assert_eq!(VersionDiff::between(&v1, &v4), VersionDiff::Patch);
    }
}
