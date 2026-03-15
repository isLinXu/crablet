// use std::collections::HashSet;
// use crate::types::Message;

#[derive(Clone, Debug, PartialEq)]
pub enum Intent {
    Greeting,
    Help,
    Status,
    Persona, // 人设/身份查询
    Chat,    // 闲聊/社交对话
    DeepResearch,
    MultiStep,
    Coding,
    Analysis,
    Creative,
    Math,
    General,
}

pub struct Classifier;

impl Classifier {
    pub fn classify_intent(input: &str) -> Intent {
        let input_lower = input.to_lowercase();
        
        // 1. Greeting / Basic
        // Include Chinese: 你好, 嗨, 测试
        if input_lower.starts_with("hi") || input_lower.starts_with("hello") || input_lower == "test" || 
           input_lower.contains("你好") || input_lower.starts_with("嗨") {
            return Intent::Greeting;
        }
        if input_lower.contains("help") || input_lower.contains("帮助") {
            return Intent::Help;
        }
        if input_lower.contains("status") || input_lower.contains("system info") || input_lower.contains("状态") {
            return Intent::Status;
        }
        
        // Persona / Identity queries - 人设/身份查询
        if input_lower.contains("who are you") || input_lower.contains("what are you") || 
           input_lower.contains("your name") || input_lower.contains("introduce yourself") ||
           input_lower.contains("你是谁") || input_lower.contains("你是什么") || 
           input_lower.contains("你叫什么") || input_lower.contains("介绍一下") ||
           input_lower.contains("你是干嘛") || input_lower.contains("你是做什么") ||
           input_lower.contains("你的身份") || input_lower.contains("你的角色") ||
           input_lower.contains("你是ai") || input_lower.contains("你是人工智能") ||
           input_lower.contains("谁创造了你") || input_lower.contains("谁开发了") {
            return Intent::Persona;
        }
        
        // Chat / Social - 闲聊/社交对话
        if input_lower.contains("how are you") || input_lower.contains("what's up") || 
           input_lower.contains("how's it going") || input_lower.contains("nice to meet") ||
           input_lower.contains("你好吗") || input_lower.contains("最近怎么样") ||
           input_lower.contains("很高兴认识") || input_lower.contains("谢谢") ||
           input_lower.contains("你多大了") || input_lower.contains("你几岁了") ||
           input_lower.contains("你喜欢什么") || input_lower.contains("你的爱好") {
            return Intent::Chat;
        }
        
        // 2. Deep Research (System 3)
        // Explicit triggers
        if input_lower.starts_with("research ") || input_lower.contains("deep research") || 
           input_lower.starts_with("研究 ") || input_lower.contains("深度研究") {
            return Intent::DeepResearch;
        }
        
        // 3. Coding
        if input_lower.contains("code") || input_lower.contains("function") || input_lower.contains("implement") || 
           input_lower.contains("代码") || input_lower.contains("编写") || input_lower.contains("实现") || 
           input_lower.contains("debug") || input_lower.contains("programming") {
            return Intent::Coding;
        }
        
        // 4. Analysis
        if input_lower.contains("analyze") || input_lower.contains("compare") || 
           input_lower.contains("分析") || input_lower.contains("比较") || input_lower.contains("investigate") {
            return Intent::Analysis;
        }

        // 5. Creative
        if input_lower.contains("write story") || input_lower.contains("generate content") || input_lower.contains("creative") {
            return Intent::Creative;
        }

        // 6. Math
        if input_lower.contains("calculate") || input_lower.contains("solve equation") || input_lower.contains("math") {
            return Intent::Math;
        }
        
        // 7. Multi-step detection (simple heuristic)
        if (input_lower.contains("first") && input_lower.contains("then")) || 
           (input_lower.contains("首先") && input_lower.contains("然后")) {
            return Intent::MultiStep;
        }
        
        Intent::General
    }
    
    // NOTE: Semantic classification logic is currently implemented in RoutingMiddleware 
    // because it depends on VectorStore which is available in the router/middleware context.
    // Ideally, we should move it here if we can pass the embedder/vector_store to this function.
    // For now, we keep the heuristic classification here and semantic logic in RoutingMiddleware (which calls this as fallback).

    pub fn assess_complexity(input: &str) -> f32 {
        let mut score: f32 = 0.0;
        
        // Length heuristic
        if input.len() > 100 { score += 0.3; } 
        if input.len() > 500 { score += 0.4; }
        
        // Keyword heuristic
        let complex_keywords = ["analyze", "compare", "reason", "explain", "design", "search", "calculate", "weather"];
        for keyword in complex_keywords {
            if input.to_lowercase().contains(keyword) {
                score += 0.2;
            }
        }
        
        // Code specific
        if input.to_lowercase().contains("function") || input.contains("```") {
            score += 0.4;
        }
        
        // Tool usage heuristic
        if input.starts_with("run ") || input.starts_with("read ") || input.starts_with("search ") {
            score += 0.6; // Strong push to cloud if tools are likely needed and local might fail tool calling
        }

        if score > 1.0 { 1.0 } else { score }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_intent() {
        assert_eq!(Classifier::classify_intent("Hello world"), Intent::Greeting);
        assert_eq!(Classifier::classify_intent("Deep research on quantum physics"), Intent::DeepResearch);
        assert_eq!(Classifier::classify_intent("Write a rust function"), Intent::Coding);
        assert_eq!(Classifier::classify_intent("Analyze the stock market"), Intent::Analysis);
        assert_eq!(Classifier::classify_intent("Just a random sentence"), Intent::General);
    }

    #[test]
    fn test_assess_complexity() {
        assert!(Classifier::assess_complexity("hi") < 0.1);
        assert!(Classifier::assess_complexity("Please analyze this complex data") >= 0.2);
        assert!(Classifier::assess_complexity("Write a function to calculate fibonacci") >= 0.4);
        // Test clamping
        assert!(Classifier::assess_complexity("run search analyze compare explain design calculate weather function ```") <= 1.0);
    }
}
