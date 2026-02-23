use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use crate::plugins::Plugin;
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
    client: Client,
    api_key: Option<String>,
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .cookie_store(true)
                .build()
                .unwrap(),
            api_key: env::var("SERPER_API_KEY").ok(),
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        // 1. Try Serper (Google API) if key is present
        if let Some(api_key) = &self.api_key {
            match self.search_serper(query, api_key).await {
                Ok(results) if !results.is_empty() => return Ok(results),
                Ok(_) => { /* Empty results, fall through to fallback */ },
                Err(e) => {
                    tracing::warn!("Serper API failed, falling back to DuckDuckGo: {}", e);
                }
            }
        }

        // 2. Fallback to DuckDuckGo HTML scraping
        self.search_duckduckgo_html(query).await
    }

    async fn search_serper(&self, query: &str, api_key: &str) -> Result<Vec<SearchResult>> {
        let url = "https://google.serper.dev/search";
        let payload = serde_json::json!({
            "q": query,
            "num": 5
        });

        let response = self.client
            .post(url)
            .header("X-API-KEY", api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Serper API failed: {}", response.status()));
        }

        let json: serde_json::Value = response.json().await?;
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

    async fn search_duckduckgo_html(&self, query: &str) -> Result<Vec<SearchResult>> {
        // Use html.duckduckgo.com which is easier to scrape than the JS version
        let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding::encode(query));
        
        let response = self.client.get(&url).send().await?;
        let html_content = response.text().await?;
        
        let document = Html::parse_document(&html_content);
        
        // DuckDuckGo HTML structure selectors
        let result_selector = Selector::parse(".result").unwrap();
        let title_selector = Selector::parse(".result__a").unwrap();
        let snippet_selector = Selector::parse(".result__snippet").unwrap();
        let _link_selector = Selector::parse(".result__url").unwrap();

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
