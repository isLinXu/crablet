//! Advanced Intent Classifier
//! 
//! Provides multi-level intent classification with confidence scores,
//! supporting both rule-based and semantic-based approaches.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// Intent types with confidence levels
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Intent {
    Greeting,
    Help,
    Status,
    DeepResearch,
    MultiStep,
    Coding,
    Analysis,
    Creative,
    Math,
    SkillExecution(String), // Specific skill to execute
    General,
    Unknown,
}

/// Classification result with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub intent: Intent,
    pub confidence: f32,
    pub alternative_intents: Vec<(Intent, f32)>,
    pub requires_clarification: bool,
    pub suggested_skills: Vec<String>,
}

/// Advanced intent classifier
pub struct IntentClassifier {
    /// Rule-based patterns
    patterns: HashMap<Intent, Vec<String>>,
    /// Confidence threshold for automatic classification
    threshold: f32,
    /// Semantic similarity threshold
    semantic_threshold: f32,
}

impl IntentClassifier {
    /// Create a new classifier with default patterns
    pub fn new() -> Self {
        let mut patterns: HashMap<Intent, Vec<String>> = HashMap::new();
        
        // Greeting patterns
        patterns.insert(Intent::Greeting, vec![
            "hi", "hello", "hey", "greetings", "good morning", "good afternoon", "good evening",
            "你好", "嗨", "您好", "早上好", "下午好", "晚上好", "在吗", "在么"
        ].into_iter().map(String::from).collect());

        // Help patterns
        patterns.insert(Intent::Help, vec![
            "help", "assist", "support", "how to", "how do i", "what can you do",
            "帮助", "怎么用", "如何使用", "你能做什么", "有什么功能"
        ].into_iter().map(String::from).collect());

        // Status patterns
        patterns.insert(Intent::Status, vec![
            "status", "system info", "health", "check", "diagnostics",
            "状态", "系统信息", "健康检查", "诊断"
        ].into_iter().map(String::from).collect());

        // Deep research patterns
        patterns.insert(Intent::DeepResearch, vec![
            "research", "deep research", "investigate", "explore in depth", "comprehensive analysis",
            "研究", "深度研究", "深入分析", "全面调查", "详细探讨"
        ].into_iter().map(String::from).collect());

        // Coding patterns
        patterns.insert(Intent::Coding, vec![
            "code", "function", "implement", "program", "debug", "refactor", "algorithm",
            "write", "develop", "script", "class", "module", "api",
            "代码", "编写", "实现", "函数", "调试", "程序", "算法", "开发"
        ].into_iter().map(String::from).collect());

        // Analysis patterns
        patterns.insert(Intent::Analysis, vec![
            "analyze", "compare", "evaluate", "assess", "review", "examine",
            "analysis", "comparison", "pros and cons", "advantages",
            "分析", "比较", "评估", "评价", "优缺点", "优势劣势"
        ].into_iter().map(String::from).collect());

        // Creative patterns
        patterns.insert(Intent::Creative, vec![
            "write", "create", "generate", "compose", "draft", "story", "poem", "article",
            "creative", "imagine", "design",
            "写", "创作", "生成", "故事", "诗歌", "文章", "创意"
        ].into_iter().map(String::from).collect());

        // Math patterns
        patterns.insert(Intent::Math, vec![
            "calculate", "compute", "solve", "equation", "formula", "math", "mathematics",
            "sum", "add", "subtract", "multiply", "divide", "percentage", "statistics",
            "计算", "求解", "方程", "数学", "公式", "统计"
        ].into_iter().map(String::from).collect());

        // Multi-step patterns
        patterns.insert(Intent::MultiStep, vec![
            "first", "then", "next", "after", "finally", "step by step",
            "首先", "然后", "接着", "最后", "一步步", "步骤"
        ].into_iter().map(String::from).collect());

        Self {
            patterns,
            threshold: 0.7,
            semantic_threshold: 0.6,
        }
    }

    /// Set custom threshold
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Classify intent with full result
    pub fn classify(&self, input: &str) -> ClassificationResult {
        let input_lower = input.to_lowercase();
        let mut scores: HashMap<Intent, f32> = HashMap::new();

        // 1. Rule-based scoring
        for (intent, patterns) in &self.patterns {
            let score = self.calculate_pattern_score(&input_lower, patterns);
            if score > 0.0 {
                scores.insert(intent.clone(), score);
            }
        }

        // 2. Heuristic scoring
        let heuristic_scores = self.heuristic_classification(&input_lower);
        for (intent, score) in heuristic_scores {
            scores.entry(intent).and_modify(|s| *s += score).or_insert(score);
        }

        // 3. Find best match
        let mut sorted_scores: Vec<(Intent, f32)> = scores.into_iter().collect();
        sorted_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Determine primary intent and confidence
        let (primary_intent, confidence) = sorted_scores
            .first()
            .cloned()
            .unwrap_or((Intent::General, 0.0));

        // Get alternative intents (top 3 excluding primary)
        let alternatives: Vec<(Intent, f32)> = sorted_scores
            .iter()
            .skip(1)
            .take(3)
            .cloned()
            .collect();

        // Determine if clarification is needed
        let requires_clarification = confidence < self.threshold || 
            (alternatives.first().map(|(_, s)| *s).unwrap_or(0.0) > confidence * 0.8);

        // Extract suggested skills based on context
        let suggested_skills = self.suggest_skills(&primary_intent, &input_lower);

        ClassificationResult {
            intent: primary_intent,
            confidence,
            alternative_intents: alternatives,
            requires_clarification,
            suggested_skills,
        }
    }

    /// Calculate pattern matching score
    fn calculate_pattern_score(&self, input: &str, patterns: &[String]) -> f32 {
        let mut max_score: f32 = 0.0;

        for pattern in patterns {
            let pattern_lower = pattern.to_lowercase();
            
            // Exact match
            if input == pattern_lower {
                return 1.0;
            }
            
            // Starts with pattern
            if input.starts_with(&pattern_lower) {
                max_score = max_score.max(0.9);
            }
            
            // Contains pattern as word
            if input.contains(&format!(" {} ", pattern_lower)) ||
               input.starts_with(&format!("{} ", pattern_lower)) ||
               input.ends_with(&format!(" {}", pattern_lower)) {
                max_score = max_score.max(0.8);
            }
            
            // Contains pattern
            if input.contains(&pattern_lower) {
                max_score = max_score.max(0.6);
            }

            // Fuzzy match using Jaro-Winkler
            let similarity = strsim::jaro_winkler(input, &pattern_lower) as f32;
            if similarity > 0.8 {
                max_score = max_score.max(similarity * 0.7);
            }
        }

        max_score
    }

    /// Heuristic classification for complex cases
    fn heuristic_classification(&self, input: &str) -> Vec<(Intent, f32)> {
        let mut scores = Vec::new();
        let word_count = input.split_whitespace().count();

        // Length-based heuristics
        if word_count > 20 {
            scores.push((Intent::DeepResearch, 0.2));
        }

        // Code detection
        if input.contains("```") || 
           input.contains("fn ") || 
           input.contains("def ") ||
           input.contains("class ") ||
           input.contains("import ") ||
           input.contains("function") {
            scores.push((Intent::Coding, 0.4));
        }

        // Question detection
        if input.contains("?") || input.contains("？") {
            // Check if it's a how/what/why question
            if input.starts_with("how ") || input.starts_with("what ") || 
               input.starts_with("why ") || input.starts_with("when ") ||
               input.starts_with("怎么") || input.starts_with("什么") ||
               input.starts_with("为什么") || input.starts_with("何时") {
                scores.push((Intent::Analysis, 0.2));
            }
        }

        // Multi-step detection
        let step_indicators = ["step", "stage", "phase", "part", "步骤", "阶段"];
        let step_count = step_indicators.iter()
            .filter(|&indicator| input.contains(indicator))
            .count();
        if step_count >= 2 || (input.contains("first") && input.contains("then")) {
            scores.push((Intent::MultiStep, 0.5));
        }

        scores
    }

    /// Suggest relevant skills based on intent
    fn suggest_skills(&self, intent: &Intent, input: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        let input_lower = input.to_lowercase();

        match intent {
            Intent::Coding => {
                if input_lower.contains("rust") || input_lower.contains("cargo") {
                    suggestions.push("rust_analyzer".to_string());
                }
                if input_lower.contains("python") || input_lower.contains("pip") {
                    suggestions.push("python_runner".to_string());
                }
                if input_lower.contains("test") || input_lower.contains("debug") {
                    suggestions.push("code_tester".to_string());
                }
                suggestions.push("code_search".to_string());
            }
            Intent::Analysis => {
                if input_lower.contains("file") || input_lower.contains("directory") {
                    suggestions.push("file_analyzer".to_string());
                }
                if input_lower.contains("data") || input_lower.contains("csv") || input_lower.contains("json") {
                    suggestions.push("data_analyzer".to_string());
                }
            }
            Intent::DeepResearch => {
                suggestions.push("web_search".to_string());
                suggestions.push("knowledge_graph".to_string());
            }
            Intent::SkillExecution(skill_name) => {
                suggestions.push(skill_name.clone());
            }
            _ => {}
        }

        suggestions
    }

    /// Quick classify for simple cases
    pub fn quick_classify(&self, input: &str) -> Intent {
        self.classify(input).intent
    }

    /// Check if input is likely a skill invocation
    pub fn is_skill_invocation(&self, input: &str) -> Option<String> {
        let input_lower = input.to_lowercase().trim().to_string();
        
        // Check for explicit skill calls: "use skill_name" or "run skill_name"
        let prefixes = ["use ", "run ", "call ", "execute ", "skill ", "使用", "运行", "调用"];
        for prefix in &prefixes {
            if let Some(rest) = input_lower.strip_prefix(prefix) {
                let skill_name = rest.trim().split_whitespace().next()?;
                return Some(skill_name.to_string());
            }
        }

        // Check for skill name pattern: skill_name(args)
        if let Some(start) = input_lower.find('(') {
            let skill_name = input_lower[..start].trim();
            if !skill_name.is_empty() && !skill_name.contains(' ') {
                return Some(skill_name.to_string());
            }
        }

        None
    }

    /// Adapt threshold based on feedback
    pub fn adapt_threshold(&mut self, success_rate: f32) {
        let target_rate = 0.85;
        let adjustment = (success_rate - target_rate) * 0.05;
        self.threshold = (self.threshold - adjustment).clamp(0.5, 0.9);
        info!("Adapted intent classifier threshold to {:.2}", self.threshold);
    }
}

impl Default for IntentClassifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy classifier for backward compatibility
pub struct Classifier;

impl Classifier {
    pub fn classify_intent(input: &str) -> Intent {
        IntentClassifier::new().quick_classify(input)
    }

    pub fn assess_complexity(input: &str) -> f32 {
        let mut score: f32 = 0.0;
        let input_lower = input.to_lowercase();
        
        // Length heuristic
        if input.len() > 100 { score += 0.3; } 
        if input.len() > 500 { score += 0.4; }
        
        // Keyword heuristic
        let complex_keywords = ["analyze", "compare", "reason", "explain", "design", "search", "calculate", "weather"];
        for keyword in complex_keywords {
            if input_lower.contains(keyword) {
                score += 0.2;
            }
        }
        
        // Code specific
        if input_lower.contains("function") || input.contains("```") {
            score += 0.4;
        }
        
        // Tool usage heuristic
        if input.starts_with("run ") || input.starts_with("read ") || input.starts_with("search ") {
            score += 0.6;
        }

        score.min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_greeting() {
        let classifier = IntentClassifier::new();
        
        let result = classifier.classify("Hello!");
        assert_eq!(result.intent, Intent::Greeting);
        assert!(result.confidence > 0.8);

        let result = classifier.classify("你好");
        assert_eq!(result.intent, Intent::Greeting);
    }

    #[test]
    fn test_classify_coding() {
        let classifier = IntentClassifier::new();
        
        let result = classifier.classify("Write a function to sort an array");
        assert_eq!(result.intent, Intent::Coding);
        assert!(!result.suggested_skills.is_empty());
    }

    #[test]
    fn test_skill_invocation_detection() {
        let classifier = IntentClassifier::new();
        
        assert_eq!(
            classifier.is_skill_invocation("use weather"),
            Some("weather".to_string())
        );
        
        assert_eq!(
            classifier.is_skill_invocation("run code_search"),
            Some("code_search".to_string())
        );
        
        assert_eq!(
            classifier.is_skill_invocation("weather()"),
            Some("weather".to_string())
        );
        
        assert!(classifier.is_skill_invocation("Hello world").is_none());
    }

    #[test]
    fn test_clarification_needed() {
        let classifier = IntentClassifier::new();
        
        // Ambiguous input should require clarification
        let result = classifier.classify("do something");
        assert!(result.requires_clarification || result.confidence < 0.7);
    }
}
