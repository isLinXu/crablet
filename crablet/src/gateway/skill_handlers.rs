//! Skill-related web handlers
//!
//! Handles skill listing, search, installation, and execution.

use std::sync::Arc;
use std::sync::OnceLock;
use std::path::PathBuf;
use axum::{
    extract::{State, Json, Query},
    http::StatusCode,
};
use tokio::sync::RwLock;
use tokio::process::Command;
use regex::Regex;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::gateway::server::CrabletGateway;

// Use web_handlers' disabled_skills_store
use crate::gateway::web_handlers::disabled_skills_store;

#[derive(Deserialize)]
pub struct ToggleSkillRequest {
    enabled: bool,
}

#[derive(Deserialize)]
pub struct SearchSkillsQuery {
    q: Option<String>,
}

#[derive(Deserialize)]
pub struct InstallSkillRequest {
    name: Option<String>,
    url: Option<String>,
    source: Option<String>,
    skill_id: Option<String>,
}

#[derive(Deserialize)]
pub struct BatchTestSkillsRequest {
    skills: Vec<String>,
}

#[derive(Deserialize)]
pub struct SkillsShTopQuery {
    limit: Option<usize>,
}

// ============================================================================
// SkillHub API 请求结构体
// ============================================================================

#[derive(Deserialize)]
pub struct SkillHubSearchQuery {
    q: Option<String>,
    page: Option<usize>,
    page_size: Option<usize>,
}

#[derive(Deserialize)]
pub struct SkillHubInstallRequest {
    name: String,
    target_dir: Option<String>,
    use_cli: Option<bool>,
}

#[derive(Deserialize)]
pub struct SkillHubTestRequest {
    skill_name: Option<String>,
    skill_path: Option<String>,
}

#[derive(Deserialize)]
pub struct SkillHubBatchRequest {
    skills: Vec<String>,
    target_dir: Option<String>,
}

#[derive(Deserialize)]
struct ClawhubSearchItem {
    slug: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    summary: Option<String>,
}

#[derive(Deserialize)]
struct ClawhubSearchResponse {
    results: Vec<ClawhubSearchItem>,
}

pub async fn list_skills(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    let skills = gateway.router.shared_skills.read().await.list_skills();
    let disabled = disabled_skills_store().read().await;
    let enriched: Vec<serde_json::Value> = skills
        .into_iter()
        .map(|skill| {
            let mut value = serde_json::to_value(skill).unwrap_or_else(|_| serde_json::json!({}));
            if let Some(obj) = value.as_object_mut() {
                let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or_default();
                obj.insert("enabled".to_string(), serde_json::json!(!disabled.contains(name)));
            }
            value
        })
        .collect();
    Json(serde_json::json!(enriched))
}

pub async fn toggle_skill(
    State(_gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(req): Json<ToggleSkillRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut disabled = disabled_skills_store().write().await;
    if req.enabled {
        disabled.remove(&name);
    } else {
        disabled.insert(name.clone());
    }
    Ok(Json(serde_json::json!({
        "status": "ok",
        "name": name,
        "enabled": req.enabled
    })))
}

pub async fn search_registry_skills(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Query(query): axum::extract::Query<SearchSkillsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let q = query.q.unwrap_or_default();
    let clawhub_url = format!("https://clawhub.ai/api/search?q={}&limit=20", urlencoding::encode(&q));
    if let Ok(resp) = reqwest::get(clawhub_url).await {
        if let Ok(payload) = resp.json::<ClawhubSearchResponse>().await {
            let items: Vec<serde_json::Value> = payload.results.into_iter().map(|x| {
                serde_json::json!({
                    "name": x.slug,
                    "description": x.summary.unwrap_or_default(),
                    "version": "latest",
                    "url": format!("clawhub://{}", x.slug),
                    "author": serde_json::Value::Null,
                    "rating": serde_json::Value::Null,
                    "downloads": serde_json::Value::Null,
                    "display_name": x.display_name.unwrap_or_default()
                })
            }).collect();
            return Ok(Json(serde_json::json!({
                "status": "ok",
                "source": "clawhub",
                "items": items
            })));
        }
    }

    let registry = gateway.router.shared_skills.read().await;
    match registry.search(&q).await {
        Ok(items) => Ok(Json(serde_json::json!({
            "status": "ok",
            "source": "local-registry",
            "items": items
        }))),
        Err(e) => {
            tracing::warn!("Registry search unavailable: {}", e);
            Ok(Json(serde_json::json!({
                "status": "unavailable",
                "source": "none",
                "items": []
            })))
        }
    }
}

pub async fn get_skills_sh_top(
    State(_gateway): State<Arc<CrabletGateway>>,
    axum::extract::Query(query): axum::extract::Query<SkillsShTopQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = query.limit.unwrap_or(100).min(100);
    let body = reqwest::get("https://skills.sh/")
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?
        .text()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let body = body.replace("\\\"", "\"");

    let re = Regex::new(r#"\{"source":"([^"]+)","skillId":"([^"]+)","name":"([^"]+)","installs":(\d+)\}"#)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut items: Vec<serde_json::Value> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for cap in re.captures_iter(&body) {
        let source = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
        let skill_id = cap.get(2).map(|m| m.as_str()).unwrap_or_default();
        let name = cap.get(3).map(|m| m.as_str()).unwrap_or_default();
        let installs = cap
            .get(4)
            .and_then(|m| m.as_str().parse::<u64>().ok())
            .unwrap_or(0);
        let key = format!("{}::{}", source, skill_id);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        items.push(serde_json::json!({
            "source": source,
            "skill_id": skill_id,
            "name": name,
            "installs": installs
        }));
        if items.len() >= limit {
            break;
        }
    }

    Ok(Json(serde_json::json!({
        "status": "ok",
        "source": "skills.sh",
        "items": items
    })))
}

pub async fn install_skill(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<InstallSkillRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let target_dir = PathBuf::from("./skills");
    let mut registry = gateway.router.shared_skills.write().await;
    if let Some(name) = req.name.as_deref() {
        if registry.get_skill(name).is_some() {
            return Ok(Json(serde_json::json!({
                "status": "already_installed",
                "name": name
            })));
        }
    }
    let result = if let Some(url) = req.url.as_deref() {
        match crate::skills::installer::SkillInstaller::install_from_git(url, &target_dir).await {
            Ok(_) => registry.load_from_dir(&target_dir).await,
            Err(e) => Err(e),
        }
    } else if let Some(source) = req.source.as_deref() {
        let status = Command::new("npx")
            .arg("skillsadd")
            .arg(source)
            .status()
            .await;
        match status {
            Ok(s) if s.success() => registry.load_from_dir(&target_dir).await,
            _ => {
                let fallback_url = format!("https://github.com/{}.git", source);
                match crate::skills::installer::SkillInstaller::install_from_git(&fallback_url, &target_dir).await {
                    Ok(_) => registry.load_from_dir(&target_dir).await,
                    Err(e) => Err(e),
                }
            }
        }
    } else if let Some(name) = req.name.as_deref() {
        match registry.install(name, target_dir.clone()).await {
            Ok(_) => Ok(()),
            Err(_) => {
                let status = Command::new("npx")
                    .arg("clawhub@latest")
                    .arg("install")
                    .arg(name)
                    .arg("--workdir")
                    .arg(".")
                    .arg("--dir")
                    .arg("skills")
                    .arg("--no-input")
                    .status()
                    .await;
                match status {
                    Ok(s) if s.success() => registry.load_from_dir(&target_dir).await,
                    Ok(_) => Err(anyhow::anyhow!("clawhub install failed")),
                    Err(e) => Err(anyhow::anyhow!("failed to run clawhub install: {}", e)),
                }
            }
        }
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    match result {
        Ok(_) => Ok(Json(serde_json::json!({
            "status": "installed",
            "skill_id": req.skill_id
        }))),
        Err(e) => {
            tracing::error!("Failed to install skill: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn batch_test_skills(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<BatchTestSkillsRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let registry = gateway.router.shared_skills.read().await;
    let disabled = disabled_skills_store().read().await;
    let results: Vec<serde_json::Value> = req.skills.into_iter().map(|name| {
        let installed = registry.get_skill(&name).is_some();
        let enabled = !disabled.contains(&name);
        serde_json::json!({
            "name": name,
            "installed": installed,
            "enabled": enabled,
            "passed": installed && enabled
        })
    }).collect();
    Ok(Json(serde_json::json!({
        "status": "ok",
        "results": results
    })))
}

// ============================================================================
// Semantic Skill Search
// ============================================================================

#[derive(Deserialize)]
pub struct SemanticSearchRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_min_similarity")]
    pub min_similarity: f32,
}

fn default_limit() -> usize { 10 }
fn default_min_similarity() -> f32 { 0.5 }

#[derive(Serialize)]
pub struct SemanticSearchResult {
    pub skill_name: String,
    pub description: String,
    pub version: String,
    pub similarity_score: f32,
    pub match_type: String,
    pub tags: Vec<String>,
    pub author: String,
    pub category: String,
}

pub async fn semantic_search_skills(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<SemanticSearchRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let results = perform_semantic_search(&gateway, &req.query, req.limit, req.min_similarity).await;

    match results {
        Ok(items) => Ok(Json(serde_json::json!({
            "status": "ok",
            "query": req.query,
            "results": items
        }))),
        Err(e) => {
            tracing::warn!("Semantic search failed: {}, falling back to keyword search", e);
            let fallback = perform_keyword_search(&gateway, &req.query, req.limit).await;
            Ok(Json(serde_json::json!({
                "status": "fallback",
                "query": req.query,
                "results": fallback,
                "note": "Semantic search unavailable, using keyword search"
            })))
        }
    }
}

async fn perform_semantic_search(
    gateway: &Arc<CrabletGateway>,
    query: &str,
    limit: usize,
    min_similarity: f32,
) -> anyhow::Result<Vec<SemanticSearchResult>> {
    let registry = gateway.router.shared_skills.read().await;
    let all_skills = registry.list_skills();
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();

    let mut results: Vec<(SemanticSearchResult, f32)> = Vec::new();

    for skill in all_skills {
        let name_lower = skill.name.to_lowercase();
        let desc_lower = skill.description.to_lowercase();

        let mut keyword_score = 0.0f32;
        let mut matched_keywords = 0;

        for word in &query_words {
            if name_lower.contains(word) {
                keyword_score += 0.4;
                matched_keywords += 1;
            }
            if desc_lower.contains(word) {
                keyword_score += 0.2;
                matched_keywords += 1;
            }
        }

        let semantic_score = calculate_semantic_similarity(&query_lower, &desc_lower);
        let total_score = (keyword_score * 0.6 + semantic_score * 0.4).min(1.0);

        if total_score >= min_similarity || matched_keywords > 0 {
            results.push((SemanticSearchResult {
                skill_name: skill.name.clone(),
                description: skill.description.clone(),
                version: skill.version.clone(),
                similarity_score: total_score,
                match_type: if matched_keywords > 0 && semantic_score > 0.3 {
                    "hybrid".to_string()
                } else if matched_keywords > 0 {
                    "keyword".to_string()
                } else {
                    "semantic".to_string()
                },
                tags: Vec::new(),
                author: skill.author.clone().unwrap_or_else(|| "Unknown".to_string()),
                category: "General".to_string(),
            }, total_score));
        }
    }

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);

    Ok(results.into_iter().map(|(r, _)| r).collect())
}

async fn perform_keyword_search(
    gateway: &Arc<CrabletGateway>,
    query: &str,
    limit: usize,
) -> Vec<SemanticSearchResult> {
    let registry = gateway.router.shared_skills.read().await;
    let all_skills = registry.list_skills();
    let query_lower = query.to_lowercase();

    let mut results: Vec<(SemanticSearchResult, f32)> = Vec::new();

    for skill in all_skills {
        let name_lower = skill.name.to_lowercase();
        let desc_lower = skill.description.to_lowercase();

        let mut score = 0.0f32;
        if name_lower.contains(&query_lower) {
            score += 1.0;
        }
        if desc_lower.contains(&query_lower) {
            score += 0.5;
        }

        for word in query_lower.split_whitespace() {
            if name_lower.contains(word) {
                score += 0.3;
            }
            if desc_lower.contains(word) {
                score += 0.15;
            }
        }

        if score > 0.0 {
            results.push((SemanticSearchResult {
                skill_name: skill.name.clone(),
                description: skill.description.clone(),
                version: skill.version.clone(),
                similarity_score: score.min(1.0),
                match_type: "keyword".to_string(),
                tags: Vec::new(),
                author: skill.author.clone().unwrap_or_else(|| "Unknown".to_string()),
                category: "General".to_string(),
            }, score));
        }
    }

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);

    results.into_iter().map(|(r, _)| r).collect()
}

fn calculate_semantic_similarity(a: &str, b: &str) -> f32 {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    if a_chars.len() < 2 || b_chars.len() < 2 {
        return if a == b { 1.0 } else { 0.0 };
    }

    let mut a_ngrams: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut b_ngrams: std::collections::HashSet<String> = std::collections::HashSet::new();

    for i in 0..a_chars.len() - 1 {
        let ngram: String = a_chars[i..i + 2].iter().collect();
        a_ngrams.insert(ngram);
    }

    for i in 0..b_chars.len() - 1 {
        let ngram: String = b_chars[i..i + 2].iter().collect();
        b_ngrams.insert(ngram);
    }

    let intersection: std::collections::HashSet<_> = a_ngrams.intersection(&b_ngrams).collect();
    let union: std::collections::HashSet<_> = a_ngrams.union(&b_ngrams).collect();

    if union.is_empty() {
        0.0
    } else {
        intersection.len() as f32 / union.len() as f32
    }
}

// ============================================================================
// Skill Execution
// ============================================================================

#[derive(Deserialize)]
pub struct RunSkillRequest {
    pub args: Option<serde_json::Value>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 { 30 }

#[derive(Serialize)]
pub struct RunSkillResult {
    pub skill_name: String,
    pub success: bool,
    pub output: String,
    pub execution_time_ms: u64,
    pub timestamp: String,
}

pub async fn run_skill(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(req): Json<RunSkillRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let start_time = std::time::Instant::now();
    let args = req.args.unwrap_or_else(|| serde_json::json!({}));

    let registry = gateway.router.shared_skills.read().await;
    let skill = registry.get_skill(&name);

    if skill.is_none() {
        return Ok(Json(serde_json::json!({
            "status": "error",
            "error": format!("Skill '{}' not found", name)
        })));
    }
    drop(registry);

    let registry = gateway.router.shared_skills.read().await;
    let result = registry.execute(&name, args).await;
    let execution_time = start_time.elapsed().as_millis() as u64;

    let log_entry = SkillExecutionLog {
        skill_name: name.clone(),
        timestamp: Utc::now(),
        success: result.is_ok(),
        output: result.as_ref().ok().cloned().unwrap_or_default(),
        error: result.as_ref().err().map(|e| e.to_string()),
        execution_time_ms: execution_time,
    };

    store_execution_log(log_entry).await;

    match result {
        Ok(output) => Ok(Json(serde_json::json!({
            "status": "ok",
            "result": RunSkillResult {
                skill_name: name,
                success: true,
                output,
                execution_time_ms: execution_time,
                timestamp: Utc::now().to_rfc3339(),
            }
        }))),
        Err(e) => Ok(Json(serde_json::json!({
            "status": "error",
            "error": e.to_string(),
            "result": RunSkillResult {
                skill_name: name,
                success: false,
                output: e.to_string(),
                execution_time_ms: execution_time,
                timestamp: Utc::now().to_rfc3339(),
            }
        }))),
    }
}

#[derive(Clone, Serialize)]
struct SkillExecutionLog {
    skill_name: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    success: bool,
    output: String,
    error: Option<String>,
    execution_time_ms: u64,
}

static EXECUTION_LOGS: OnceLock<RwLock<Vec<SkillExecutionLog>>> = OnceLock::new();

async fn store_execution_log(log: SkillExecutionLog) {
    let logs = EXECUTION_LOGS.get_or_init(|| RwLock::new(Vec::new()));
    let mut logs_guard = logs.write().await;
    logs_guard.push(log);
    if logs_guard.len() > 1000 {
        logs_guard.remove(0);
    }
}

#[derive(Deserialize)]
pub struct GetSkillLogsQuery {
    limit: Option<usize>,
}

pub async fn get_skill_logs(
    State(_gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Query(query): Query<GetSkillLogsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = query.limit.unwrap_or(50).min(100);

    let logs = EXECUTION_LOGS.get_or_init(|| RwLock::new(Vec::new()));
    let logs_guard = logs.read().await;

    let skill_logs: Vec<&SkillExecutionLog> = logs_guard
        .iter()
        .rev()
        .filter(|log| log.skill_name == name)
        .take(limit)
        .collect();

    Ok(Json(serde_json::json!({
        "status": "ok",
        "skill_name": name,
        "logs": skill_logs,
        "total": skill_logs.len()
    })))
}

pub async fn get_all_skill_logs(
    State(_gateway): State<Arc<CrabletGateway>>,
    Query(query): Query<GetSkillLogsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = query.limit.unwrap_or(100).min(200);

    let logs = EXECUTION_LOGS.get_or_init(|| RwLock::new(Vec::new()));
    let logs_guard = logs.read().await;

    let all_logs: Vec<&SkillExecutionLog> = logs_guard
        .iter()
        .rev()
        .take(limit)
        .collect();

    Ok(Json(serde_json::json!({
        "status": "ok",
        "logs": all_logs,
        "total": all_logs.len()
    })))
}

pub async fn get_mcp_overview(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let registry = gateway.router.shared_skills.read().await;
    let skills = registry.list_skills();
    let mcp_tools = skills.iter().filter(|s| {
        s.runtime.as_deref() == Some("mcp") || s.version.contains("(MCP)")
    }).count();
    let resources = registry.list_resources();
    let prompts = registry.list_prompts();
    Ok(Json(serde_json::json!({
        "status": "ok",
        "mcp_tools": mcp_tools,
        "resources": resources.len(),
        "prompts": prompts.len(),
        "resource_items": resources,
        "prompt_items": prompts
    })))
}

// ============================================================================
// SkillHub API Handlers
// ============================================================================

static SKILLHUB_CLIENT: OnceLock<crate::skills::SkillHubClient> = OnceLock::new();

fn get_skillhub_client() -> &'static crate::skills::SkillHubClient {
    SKILLHUB_CLIENT.get_or_init(|| {
        crate::skills::SkillHubClient::default_config()
    })
}

pub async fn skillhub_search(
    Query(query): Query<SkillHubSearchQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_skillhub_client();
    let q = query.q.unwrap_or_default();
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20).min(100);
    
    match client.search_via_api(&q, page, page_size).await {
        Ok(result) => Ok(Json(serde_json::json!({
            "status": "ok",
            "query": q,
            "total": result.total,
            "page": result.page,
            "page_size": result.page_size,
            "items": result.skills
        }))),
        Err(e) => {
            tracing::warn!("SkillHub search failed: {}", e);
            // 尝试通过 CLI 搜索
            if let Ok(skills) = client.search_via_cli(&q).await {
                return Ok(Json(serde_json::json!({
                    "status": "ok",
                    "query": q,
                    "total": skills.len(),
                    "source": "cli",
                    "items": skills
                })));
            }
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string(),
                "items": []
            })))
        }
    }
}

pub async fn skillhub_get_featured(
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_skillhub_client();
    
    match client.get_featured().await {
        Ok(items) => Ok(Json(serde_json::json!({
            "status": "ok",
            "total": items.len(),
            "items": items
        }))),
        Err(e) => {
            tracing::warn!("SkillHub featured failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string(),
                "items": []
            })))
        }
    }
}

pub async fn skillhub_get_categories(
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_skillhub_client();
    
    match client.get_categories().await {
        Ok(categories) => Ok(Json(serde_json::json!({
            "status": "ok",
            "categories": categories
        }))),
        Err(e) => {
            tracing::warn!("SkillHub categories failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string(),
                "categories": []
            })))
        }
    }
}

pub async fn skillhub_get_detail(
    axum::extract::Path(skill_name): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_skillhub_client();
    
    match client.get_skill_detail(&skill_name).await {
        Ok(skill) => Ok(Json(serde_json::json!({
            "status": "ok",
            "skill": skill
        }))),
        Err(e) => {
            tracing::warn!("SkillHub detail failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn skillhub_install(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<SkillHubInstallRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_skillhub_client();
    
    let target_dir = req.target_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./skills"));
    
    let use_cli = req.use_cli.unwrap_or(true);
    
    if use_cli && !crate::skills::SkillHubClient::is_cli_installed() {
        return Ok(Json(serde_json::json!({
            "status": "error",
            "error": "SkillHub CLI not installed. Install with: curl -fsSL https://skillhub-1388575217.cos.ap-guangzhou.myqcloud.com/install/install.sh | bash"
        })));
    }
    
    match client.install(&req.name, &target_dir).await {
        Ok(result) => {
            // 重新加载 skills
            let mut registry = gateway.router.shared_skills.write().await;
            if let Err(e) = registry.load_from_dir(&target_dir).await {
                tracing::warn!("Failed to reload skills: {}", e);
            }
            
            Ok(Json(serde_json::json!({
                "status": if result.success { "ok" } else { "error" },
                "result": result
            })))
        },
        Err(e) => {
            tracing::error!("SkillHub install failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn skillhub_batch_install(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<SkillHubBatchRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_skillhub_client();
    
    let target_dir = req.target_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./skills"));
    
    match client.install_batch(&req.skills, &target_dir).await {
        Ok(results) => {
            // 重新加载 skills
            let mut registry = gateway.router.shared_skills.write().await;
            if let Err(e) = registry.load_from_dir(&target_dir).await {
                tracing::warn!("Failed to reload skills: {}", e);
            }
            
            let success_count = results.iter().filter(|r| r.success).count();
            
            Ok(Json(serde_json::json!({
                "status": "ok",
                "total": req.skills.len(),
                "success": success_count,
                "failed": req.skills.len() - success_count,
                "results": results
            })))
        },
        Err(e) => {
            tracing::error!("SkillHub batch install failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn skillhub_test(
    Json(req): Json<SkillHubTestRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_skillhub_client();
    
    let skill_path = if let Some(path) = req.skill_path {
        PathBuf::from(path)
    } else if let Some(name) = req.skill_name {
        PathBuf::from("./skills").join(&name)
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };
    
    match client.test_skill(&skill_path).await {
        Ok(results) => {
            let passed = results.iter().filter(|r| r.passed).count();
            Ok(Json(serde_json::json!({
                "status": "ok",
                "skill_path": skill_path,
                "total": results.len(),
                "passed": passed,
                "failed": results.len() - passed,
                "results": results
            })))
        },
        Err(e) => {
            tracing::warn!("SkillHub test failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn skillhub_validate(
    Json(req): Json<SkillHubTestRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_skillhub_client();
    
    let skill_path = if let Some(path) = req.skill_path {
        PathBuf::from(path)
    } else if let Some(name) = req.skill_name {
        PathBuf::from("./skills").join(&name)
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };
    
    match client.validate_skill(&skill_path).await {
        Ok(valid) => Ok(Json(serde_json::json!({
            "status": "ok",
            "skill_path": skill_path,
            "valid": valid
        }))),
        Err(e) => {
            tracing::warn!("SkillHub validation failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn skillhub_list_installed(
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_skillhub_client();
    let install_dir = PathBuf::from("./skills");
    
    match client.list_installed(&install_dir).await {
        Ok(skills) => Ok(Json(serde_json::json!({
            "status": "ok",
            "total": skills.len(),
            "skills": skills
        }))),
        Err(e) => {
            tracing::warn!("SkillHub list installed failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn skillhub_uninstall(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(skill_name): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_skillhub_client();
    let install_dir = PathBuf::from("./skills");
    
    match client.uninstall(&skill_name, &install_dir).await {
        Ok(_) => {
            // 重新加载 skills
            let mut registry = gateway.router.shared_skills.write().await;
            if let Err(e) = registry.load_from_dir(&install_dir).await {
                tracing::warn!("Failed to reload skills: {}", e);
            }
            
            Ok(Json(serde_json::json!({
                "status": "ok",
                "skill_name": skill_name
            })))
        },
        Err(e) => {
            tracing::error!("SkillHub uninstall failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn skillhub_check_update(
    axum::extract::Path(skill_name): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_skillhub_client();
    let skill_path = PathBuf::from("./skills").join(&skill_name);
    
    match client.check_for_updates(&skill_path).await {
        Ok(Some(update)) => Ok(Json(serde_json::json!({
            "status": "ok",
            "has_update": true,
            "update": update
        }))),
        Ok(None) => Ok(Json(serde_json::json!({
            "status": "ok",
            "has_update": false
        }))),
        Err(e) => {
            tracing::warn!("SkillHub check update failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

// ============================================================================
// ModelScope API Handlers
// ============================================================================

static MODELSCOPE_CLIENT: OnceLock<crate::skills::ModelScopeClient> = OnceLock::new();

fn get_modelscope_client() -> &'static crate::skills::ModelScopeClient {
    MODELSCOPE_CLIENT.get_or_init(|| {
        crate::skills::ModelScopeClient::default_config()
    })
}

#[derive(Deserialize)]
pub struct ModelScopeSearchQuery {
    q: Option<String>,
    source: Option<String>, // "official" | "community" | "all"
}

#[derive(Deserialize)]
pub struct ModelScopeInstallRequest {
    name: String,
    target_dir: Option<String>,
}

#[derive(Deserialize)]
pub struct ModelScopeTestRequest {
    skill_name: Option<String>,
    skill_path: Option<String>,
}

#[derive(Deserialize)]
pub struct ModelScopeBatchRequest {
    skills: Vec<String>,
    target_dir: Option<String>,
}

pub async fn modelscope_search(
    Query(query): Query<ModelScopeSearchQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_modelscope_client();
    let q = query.q.unwrap_or_default();
    
    match client.search_skills(&q).await {
        Ok(skills) => Ok(Json(serde_json::json!({
            "status": "ok",
            "query": q,
            "total": skills.len(),
            "items": skills
        }))),
        Err(e) => {
            tracing::warn!("ModelScope search failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string(),
                "items": []
            })))
        }
    }
}

pub async fn modelscope_list() -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_modelscope_client();
    
    match client.list_skills().await {
        Ok(skills) => Ok(Json(serde_json::json!({
            "status": "ok",
            "total": skills.len(),
            "items": skills
        }))),
        Err(e) => {
            tracing::warn!("ModelScope list failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string(),
                "items": []
            })))
        }
    }
}

pub async fn modelscope_get_featured() -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_modelscope_client();
    
    match client.get_recommended_skills().await {
        Ok(skills) => Ok(Json(serde_json::json!({
            "status": "ok",
            "total": skills.len(),
            "items": skills
        }))),
        Err(e) => {
            tracing::warn!("ModelScope get featured failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string(),
                "items": []
            })))
        }
    }
}

pub async fn modelscope_get_detail(
    axum::extract::Path(skill_name): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_modelscope_client();
    
    match client.get_skill(&skill_name).await {
        Ok(skill) => Ok(Json(serde_json::json!({
            "status": "ok",
            "item": skill
        }))),
        Err(e) => {
            tracing::warn!("ModelScope get detail failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn modelscope_install(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<ModelScopeInstallRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_modelscope_client();
    let install_dir = PathBuf::from(req.target_dir.unwrap_or_else(|| "./skills".to_string()));
    
    match client.install_skill(&req.name, &install_dir).await {
        Ok(result) => {
            if result.success {
                // 重新加载 skills
                let mut registry = gateway.router.shared_skills.write().await;
                if let Err(e) = registry.load_from_dir(&install_dir).await {
                    tracing::warn!("Failed to reload skills: {}", e);
                }
                
                Ok(Json(serde_json::json!({
                    "status": "ok",
                    "result": result
                })))
            } else {
                Ok(Json(serde_json::json!({
                    "status": "error",
                    "error": result.message,
                    "warnings": result.warnings
                })))
            }
        },
        Err(e) => {
            tracing::error!("ModelScope install failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn modelscope_install_batch(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<ModelScopeBatchRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_modelscope_client();
    let install_dir = PathBuf::from(req.target_dir.unwrap_or_else(|| "./skills".to_string()));
    
    let results = client.install_batch(&req.skills, &install_dir).await.unwrap_or_else(|e| {
        vec![crate::skills::ModelScopeInstallResult {
            success: false,
            skill_name: "batch".to_string(),
            install_path: install_dir.clone(),
            message: format!("Batch install failed: {}", e),
            warnings: vec![],
        }]
    });
    
    // 重新加载 skills
    if results.iter().any(|r| r.success) {
        let mut registry = gateway.router.shared_skills.write().await;
        if let Err(e) = registry.load_from_dir(&install_dir).await {
            tracing::warn!("Failed to reload skills: {}", e);
        }
    }
    
    Ok(Json(serde_json::json!({
        "status": "ok",
        "total": results.len(),
        "successful": results.iter().filter(|r| r.success).count(),
        "results": results
    })))
}

pub async fn modelscope_test(
    Json(req): Json<ModelScopeTestRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_modelscope_client();
    
    let skill_path = if let Some(path) = req.skill_path {
        PathBuf::from(path)
    } else if let Some(name) = req.skill_name {
        PathBuf::from("./skills").join(&name)
    } else {
        return Ok(Json(serde_json::json!({
            "status": "error",
            "error": "Either skill_name or skill_path must be provided"
        })));
    };
    
    match client.test_skill(&skill_path).await {
        Ok(results) => Ok(Json(serde_json::json!({
            "status": "ok",
            "total": results.len(),
            "passed": results.iter().filter(|r| r.passed).count(),
            "results": results
        }))),
        Err(e) => {
            tracing::warn!("ModelScope test failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn modelscope_validate(
    Json(req): Json<ModelScopeTestRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_modelscope_client();
    
    let skill_path = if let Some(path) = req.skill_path {
        PathBuf::from(path)
    } else if let Some(name) = req.skill_name {
        PathBuf::from("./skills").join(&name)
    } else {
        return Ok(Json(serde_json::json!({
            "status": "error",
            "error": "Either skill_name or skill_path must be provided"
        })));
    };
    
    match client.validate_skill(&skill_path).await {
        Ok(valid) => Ok(Json(serde_json::json!({
            "status": "ok",
            "valid": valid
        }))),
        Err(e) => {
            tracing::warn!("ModelScope validate failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn modelscope_list_installed(
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_modelscope_client();
    let install_dir = PathBuf::from("./skills");
    
    match client.list_installed(&install_dir).await {
        Ok(skills) => Ok(Json(serde_json::json!({
            "status": "ok",
            "total": skills.len(),
            "skills": skills
        }))),
        Err(e) => {
            tracing::warn!("ModelScope list installed failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn modelscope_uninstall(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(skill_name): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = get_modelscope_client();
    let install_dir = PathBuf::from("./skills");
    
    match client.uninstall_skill(&skill_name, &install_dir).await {
        Ok(_) => {
            // 重新加载 skills
            let mut registry = gateway.router.shared_skills.write().await;
            if let Err(e) = registry.load_from_dir(&install_dir).await {
                tracing::warn!("Failed to reload skills: {}", e);
            }
            
            Ok(Json(serde_json::json!({
                "status": "ok",
                "skill_name": skill_name
            })))
        },
        Err(e) => {
            tracing::error!("ModelScope uninstall failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

pub async fn modelscope_get_categories() -> Result<Json<serde_json::Value>, StatusCode> {
    // ModelScope 目前没有公开的分类 API
    // 返回一些常见的技能分类作为示例
    Ok(Json(serde_json::json!({
        "status": "ok",
        "categories": [
            "agent",
            "automation",
            "cli",
            "development",
            "documentation",
            "image-generation",
            "model-hub",
            "research",
            "testing",
            "web"
        ]
    })))
}
