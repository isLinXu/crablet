use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use crate::plugins::Plugin;
use crate::error::CrabletError;
use async_trait::async_trait;
use serde_json::Value;
use scraper::{Html, Selector};

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub link: String,
    pub snippet: String,
}

pub struct WebSearchPlugin {
    tool: WebSearchTool,
}

impl Default for WebSearchPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSearchPlugin {
    pub fn new() -> Self {
        Self {
            tool: WebSearchTool::new(),
        }
    }
}

#[async_trait]
impl Plugin for WebSearchPlugin {
    fn name(&self) -> &str {
        "search"
    }

    fn description(&self) -> &str {
        "Search the web. Args: {\"query\": \"...\"}"
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .context("Missing 'query' argument")?;
            
        let results = self.tool.search(query).await?;
        
        let mut output = String::new();
        for (i, res) in results.iter().enumerate() {
            output.push_str(&format!("{}. [{}]({})\n   {}\n\n", i + 1, res.title, res.link, res.snippet));
        }
        
        if output.is_empty() {
            Ok("No results found.".to_string())
        } else {
            Ok(output)
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct WebSearchTool {
    client: Result<Client, String>,
    api_key: Option<String>,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .cookie_store(true)
            .build()
            .map_err(|e| e.to_string());

        Self {
            client,
            api_key: env::var("SERPER_API_KEY").ok(),
        }
    }

    pub async fn search(&self, query: &str) -> std::result::Result<Vec<SearchResult>, CrabletError> {
        let client = self.client.as_ref().map_err(|e| CrabletError::SearchError(format!("HTTP Client init failed: {}", e)))?;

        // 1. Try Serper (Google API) if key is present
        if let Some(api_key) = &self.api_key {
            match self.search_serper(client, query, api_key).await {
                Ok(results) if !results.is_empty() => return Ok(results),
                Ok(_) => { /* Empty results, fall through to fallback */ },
                Err(e) => {
                    tracing::warn!("Serper API failed, falling back to DuckDuckGo: {}", e);
                }
            }
        }

        // 2. Fallback to DuckDuckGo HTML scraping
        self.search_duckduckgo_html(client, query).await
    }

    async fn search_serper(&self, client: &Client, query: &str, api_key: &str) -> std::result::Result<Vec<SearchResult>, CrabletError> {
        let url = "https://google.serper.dev/search";
        let payload = serde_json::json!({
            "q": query,
            "num": 5
        });

        let response = client
            .post(url)
            .header("X-API-KEY", api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| CrabletError::SearchError(format!("Serper API request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(CrabletError::SearchError(format!("Serper API failed: {}", response.status())));
        }

        let json: serde_json::Value = response.json().await.map_err(|e| CrabletError::SearchError(format!("Failed to parse Serper JSON: {}", e)))?;
        let mut results = Vec::new();
        
        if let Some(organic) = json.get("organic").and_then(|v| v.as_array()) {
            for item in organic {
                let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let link = item.get("link").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let snippet = item.get("snippet").and_then(|v| v.as_str()).unwrap_or("").to_string();
                
                if !title.is_empty() && !link.is_empty() {
                    results.push(SearchResult { title, link, snippet });
                }
            }
        }
        Ok(results)
    }

    async fn search_duckduckgo_html(&self, client: &Client, query: &str) -> std::result::Result<Vec<SearchResult>, CrabletError> {
        // Use html.duckduckgo.com which is easier to scrape than the JS version
        let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding::encode(query));
        
        let response = client.get(&url).send().await.map_err(|e| CrabletError::SearchError(format!("DuckDuckGo request failed: {}", e)))?;
        let html_content = response.text().await.map_err(|e| CrabletError::SearchError(format!("Failed to read DuckDuckGo body: {}", e)))?;
        
        let document = Html::parse_document(&html_content);
        
        // DuckDuckGo HTML structure selectors
        // Safe to unwrap here as these are hardcoded valid selectors
        let result_selector = Selector::parse(".result").expect("Invalid result selector");
        let title_selector = Selector::parse(".result__a").expect("Invalid title selector");
        let snippet_selector = Selector::parse(".result__snippet").expect("Invalid snippet selector");
        let _link_selector = Selector::parse(".result__url").expect("Invalid link selector");

        let mut results = Vec::new();

        for element in document.select(&result_selector).take(5) {
            let title: String = element.select(&title_selector).next()
                .map(|e| e.text().collect::<String>())
                .unwrap_or_default();
                
            let link: String = element.select(&title_selector).next()
                .and_then(|e| e.value().attr("href"))
                .map(|s| s.to_string())
                .unwrap_or_default();
                
            let snippet: String = element.select(&snippet_selector).next()
                .map(|e| e.text().collect::<String>())
                .unwrap_or_default();

            if !title.is_empty() && !link.is_empty() {
                results.push(SearchResult { 
                    title: title.trim().to_string(), 
                    link: link.trim().to_string(), 
                    snippet: snippet.trim().to_string() 
                });
            }
        }

        if results.is_empty() {
            // Fallback: Return a direct link if scraping failed (likely due to bot protection)
            return Ok(vec![SearchResult {
                title: format!("Search: {}", query),
                link: format!("https://duckduckgo.com/?q={}", urlencoding::encode(query)),
                snippet: "Could not parse search results directly (Access Denied/Captcha). Please click the link.".to_string(),
            }]);
        }

        Ok(results)
    }
}
