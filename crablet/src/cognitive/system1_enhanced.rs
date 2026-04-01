//! Enhanced System 1 - Fast Intuitive Response System
//! 
//! System 1 is designed for rapid, intuitive responses to common user inputs.
//! It uses a multi-layer matching strategy to handle a wide variety of inputs.
//!
//! Architecture:
//! 1. Pattern Matcher - Multi-pattern matching (exact, prefix, regex, semantic)
//! 2. Context Handler - Context-aware responses based on conversation history
//! 3. Dynamic Responder - Template-based dynamic response generation
//! 4. Command Registry - Extensible command library with 20+ categories

use crate::cognitive::CognitiveSystem;
use crate::types::{Message, TraceStep};
use crate::error::{Result, CrabletError};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use regex::Regex;
use chrono::{Local, Timelike};

// ============================================================================
// Types and Enums
// ============================================================================

/// Match confidence level
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum MatchConfidence {
    Exact = 100,      // Exact match
    High = 80,        // Very close match
    Medium = 60,      // Good match
    Low = 40,         // Possible match
    None = 0,         // No match
}

/// Match result from pattern matching
#[derive(Clone, Debug)]
pub struct MatchResult {
    pub command_id: String,
    pub confidence: MatchConfidence,
    pub matched_pattern: String,
    pub extracted_params: HashMap<String, String>,
    pub match_type: MatchType,
}

/// Type of pattern match
#[derive(Clone, Debug, PartialEq)]
pub enum MatchType {
    Exact,           // Character-for-character match
    Prefix,          // Prefix match
    Regex,           // Regular expression match
    Fuzzy,           // Fuzzy string match
    Semantic,        // Semantic similarity match
    Contextual,      // Context-based match
}

/// Response template with variables
#[derive(Clone)]
pub struct ResponseTemplate {
    pub templates: Vec<String>,
    pub requires_context: bool,
}

impl ResponseTemplate {
    pub fn new(templates: Vec<&str>) -> Self {
        Self {
            templates: templates.iter().map(|s| s.to_string()).collect(),
            requires_context: false,
        }
    }

    pub fn with_context(mut self) -> Self {
        self.requires_context = true;
        self
    }

    /// Render template with variables
    pub fn render(&self, vars: &HashMap<String, String>) -> String {
        use rand::prelude::IndexedRandom;
        let mut rng = rand::rng();
        
        if self.templates.is_empty() {
            return String::new();
        }
        let template = self.templates.choose(&mut rng).unwrap_or(&self.templates[0]);
        
        let mut result = template.clone();
        for (key, value) in vars {
            result = result.replace(&format!("{{{}}}", key), value);
        }
        
        // Replace time variables
        let now = Local::now();
        result = result.replace("{time}", &now.format("%H:%M").to_string());
        result = result.replace("{date}", &now.format("%Y-%m-%d").to_string());
        result = result.replace("{day}", &now.format("%A").to_string());
        
        result
    }
}

/// Handler function type for commands
type CommandHandler = Arc<dyn Fn(&str, &HashMap<String, String>, &[Message]) -> String + Send + Sync>;

/// Command definition
#[derive(Clone)]
pub struct Command {
    pub id: String,
    pub category: CommandCategory,
    pub patterns: Vec<Pattern>,
    pub response: ResponseTemplate,
    pub priority: i32,  // Higher = checked first
    pub context_aware: bool,
    pub handler: Option<CommandHandler>,
}

/// Pattern for matching
#[derive(Clone, Debug)]
pub enum Pattern {
    Exact(String),                    // Exact string match
    Prefix(String),                   // Prefix match
    Contains(String),                 // Substring match
    Regex(String),                    // Regex pattern
    Fuzzy(String, usize),            // Fuzzy match with max distance
}

/// Command categories
#[derive(Clone, Debug, PartialEq)]
pub enum CommandCategory {
    Greeting,         // Hello, hi, etc.
    Farewell,         // Goodbye, bye, etc.
    Gratitude,        // Thanks, thank you
    Identity,         // Who are you, what is your name
    Help,             // Help, assistance
    Status,           // System status
    Time,             // What time is it
    Weather,          // Weather queries
    Emotion,          // How are you, mood
    Capability,       // What can you do
    Opinion,          // What do you think
    Confirmation,     // Yes, no, ok
    Clarification,    // What, why, how come
    SmallTalk,        // Casual conversation
    Joke,             // Tell me a joke
    Compliment,       // You're great, good job
    Complaint,        // You're wrong, bad
    Meta,             // About the conversation
    Fallback,         // Catch-all
}

// ============================================================================
// Pattern Matcher
// ============================================================================

pub struct PatternMatcher {
    compiled_regexes: HashMap<String, Regex>,
}

impl Default for PatternMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternMatcher {
    pub fn new() -> Self {
        Self {
            compiled_regexes: HashMap::new(),
        }
    }

    /// Match input against a pattern
    pub fn match_pattern(&mut self, input: &str, pattern: &Pattern) -> Option<(MatchConfidence, HashMap<String, String>)> {
        let input_lower = input.trim().to_lowercase();
        
        match pattern {
            Pattern::Exact(s) => {
                if input_lower == s.to_lowercase() {
                    Some((MatchConfidence::Exact, HashMap::new()))
                } else {
                    None
                }
            }
            
            Pattern::Prefix(s) => {
                if input_lower.starts_with(&s.to_lowercase()) {
                    Some((MatchConfidence::High, HashMap::new()))
                } else {
                    None
                }
            }
            
            Pattern::Contains(s) => {
                if input_lower.contains(&s.to_lowercase()) {
                    Some((MatchConfidence::Medium, HashMap::new()))
                } else {
                    None
                }
            }
            
            Pattern::Regex(pattern_str) => {
                let regex = self.compiled_regexes.entry(pattern_str.clone())
                    .or_insert_with(|| Regex::new(pattern_str).expect("Failed to compile regex pattern"));
                
                if let Some(captures) = regex.captures(&input_lower) {
                    let mut params = HashMap::new();
                    for name in regex.capture_names().flatten() {
                        if let Some(value) = captures.name(name) {
                            params.insert(name.to_string(), value.as_str().to_string());
                        }
                    }
                    Some((MatchConfidence::High, params))
                } else {
                    None
                }
            }
            
            Pattern::Fuzzy(target, max_dist) => {
                let dist = strsim::levenshtein(&input_lower, &target.to_lowercase());
                if dist <= *max_dist {
                    let confidence = if dist == 0 {
                        MatchConfidence::Exact
                    } else if dist <= max_dist / 3 {
                        MatchConfidence::High
                    } else if dist <= max_dist * 2 / 3 {
                        MatchConfidence::Medium
                    } else {
                        MatchConfidence::Low
                    };
                    Some((confidence, HashMap::new()))
                } else {
                    None
                }
            }
        }
    }
}

// ============================================================================
// Context Handler
// ============================================================================

/// Helper function to extract text content from Message
fn get_message_text(msg: &Message) -> String {
    match &msg.content {
        Some(parts) => {
            parts.iter()
                .filter_map(|part| match part {
                    crate::types::ContentPart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("")
        }
        None => String::new(),
    }
}

pub struct ContextHandler;

impl ContextHandler {
    /// Analyze conversation context to enhance matching
    pub fn analyze_context(context: &[Message]) -> ContextInfo {
        let mut info = ContextInfo::default();
        
        if context.is_empty() {
            return info;
        }
        
        // Check last message role
        if let Some(last) = context.last() {
            info.last_role = last.role.clone();
            
            // Check if waiting for confirmation
            let last_content = get_message_text(last).to_lowercase();
            if last_content.contains("?") || 
               last_content.contains("confirm") ||
               last_content.contains("sure") ||
               last_content.contains("ok") {
                info.expecting_confirmation = true;
            }
            
            // Detect conversation topic
            if last_content.contains("code") || last_content.contains("function") {
                info.topic = Some("coding".to_string());
            } else if last_content.contains("search") || last_content.contains("find") {
                info.topic = Some("search".to_string());
            }
        }
        
        // Count turns
        info.turn_count = context.len();
        
        // Check for repeated patterns
        let user_messages: Vec<_> = context.iter()
            .filter(|m| m.role == "user")
            .collect();
        
        if user_messages.len() >= 2 {
            let last_two: Vec<_> = user_messages.iter().rev().take(2).collect();
            if get_message_text(last_two[0]).to_lowercase() == get_message_text(last_two[1]).to_lowercase() {
                info.is_repeating = true;
            }
        }
        
        info
    }
    
    /// Get contextual response modifier
    pub fn get_context_modifier(info: &ContextInfo) -> Option<String> {
        if info.is_repeating {
            Some("I notice you've asked this before. ".to_string())
        } else if info.turn_count > 10 {
            Some("We've been chatting for a while! ".to_string())
        } else {
            None
        }
    }
}

#[derive(Default, Debug)]
pub struct ContextInfo {
    pub last_role: String,
    pub turn_count: usize,
    pub expecting_confirmation: bool,
    pub topic: Option<String>,
    pub is_repeating: bool,
}

// ============================================================================
// Enhanced System 1
// ============================================================================

#[derive(Clone)]
pub struct System1Enhanced {
    commands: Vec<Command>,
    fallback_handler: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

impl Default for System1Enhanced {
    fn default() -> Self {
        Self::new()
    }
}

impl System1Enhanced {
    pub fn new() -> Self {
        let mut system = Self {
            commands: Vec::new(),
            fallback_handler: Arc::new(|input| {
                format!("I'm not sure how to respond to '{}' quickly. Let me think about it...", input)
            }),
        };
        
        system.register_default_commands();
        system
    }
    
    /// Register all default commands
    fn register_default_commands(&mut self) {
        // 1. Greetings (High priority)
        self.register_command(Command {
            id: "greeting_hello".to_string(),
            category: CommandCategory::Greeting,
            patterns: vec![
                Pattern::Exact("hello".to_string()),
                Pattern::Exact("hi".to_string()),
                Pattern::Exact("hey".to_string()),
                Pattern::Exact("你好".to_string()),
                Pattern::Exact("您好".to_string()),
                Pattern::Exact("嗨".to_string()),
                Pattern::Exact("哈喽".to_string()),
                Pattern::Prefix("hello ".to_string()),
                Pattern::Prefix("hi ".to_string()),
                Pattern::Prefix("hey ".to_string()),
                Pattern::Fuzzy("hello".to_string(), 1),
                Pattern::Fuzzy("hi".to_string(), 1),
                Pattern::Regex(r"^(?i)hello[!?.]*$".to_string()),
                Pattern::Regex(r"^(?i)hi[!?.]*$".to_string()),
                Pattern::Regex(r"^(?i)hey[!?.]*$".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "你好！我是小螃蟹🦀 —— 你的 AI 助手机器人。\n\n我可以帮你做很多事情，比如：\n• 回答问题和解释概念\n• 编写和调试代码\n• 搜索和整理信息\n• 分析数据和文档\n\n有什么我可以帮你的吗？",
            ]),
            priority: 100,
            context_aware: true,
            handler: Some(Arc::new(|_input, _vars, ctx| {
                // 根据会话历史个性化问候
                let is_returning_user = ctx.len() > 2;
                if is_returning_user {
                    "欢迎回来！我是小螃蟹🦀。今天想聊点什么？".to_string()
                } else {
                    "你好！我是小螃蟹🦀 —— 你的 AI 助手机器人。\n\n我可以帮你做很多事情，比如：\n• 回答问题和解释概念\n• 编写和调试代码\n• 搜索和整理信息\n• 分析数据和文档\n\n有什么我可以帮你的吗？".to_string()
                }
            })),
        });
        
        // 2. Time-based greetings
        self.register_command(Command {
            id: "greeting_time".to_string(),
            category: CommandCategory::Greeting,
            patterns: vec![
                Pattern::Contains("good morning".to_string()),
                Pattern::Contains("good afternoon".to_string()),
                Pattern::Contains("good evening".to_string()),
                Pattern::Contains("早上好".to_string()),
                Pattern::Contains("下午好".to_string()),
                Pattern::Contains("晚上好".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "{greeting}! 今天过得怎么样？",
            ]).with_context(),
            priority: 95,
            context_aware: true,
            handler: Some(Arc::new(|_input, _vars, _ctx| {
                let hour = Local::now().hour();
                let greeting = if hour < 12 {
                    "早上好"
                } else if hour < 18 {
                    "下午好"
                } else {
                    "晚上好"
                };
                format!("{}！有什么我可以帮你的吗？", greeting)
            })),
        });
        
        // 3. Identity
        self.register_command(Command {
            id: "identity_who".to_string(),
            category: CommandCategory::Identity,
            patterns: vec![
                Pattern::Exact("who are you".to_string()),
                Pattern::Exact("what are you".to_string()),
                Pattern::Exact("你是谁".to_string()),
                Pattern::Exact("你是什么".to_string()),
                Pattern::Contains("your name".to_string()),
                Pattern::Contains("你叫什么".to_string()),
                Pattern::Regex(r"^what('s| is) your name".to_string()),
                Pattern::Regex(r"^who (are|r) (u|you)".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "你好！我是 Crablet（小螃蟹🦀）—— 你的 AI 助手机器人。\n\n我基于大语言模型技术，可以帮你：\n• 搜索和整理信息\n• 分析数据和文档\n• 编写和调试代码\n• 解答各类问题\n\n有什么想聊的吗？",
            ]),
            priority: 90,
            context_aware: false,
            handler: None,
        });
        
        // 4. Capabilities
        self.register_command(Command {
            id: "capability_what".to_string(),
            category: CommandCategory::Capability,
            patterns: vec![
                Pattern::Exact("what can you do".to_string()),
                Pattern::Exact("你能做什么".to_string()),
                Pattern::Contains("what do you do".to_string()),
                Pattern::Contains("你会什么".to_string()),
                Pattern::Contains("your capabilities".to_string()),
                Pattern::Contains("功能".to_string()),
                Pattern::Regex(r"^help me (with|do)".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "小螃蟹🦀 可以帮你做这些事情：\n\n**信息处理**\n• 搜索和整理信息\n• 分析数据和文档\n• 解释概念和原理\n\n**编程开发**\n• 编写和调试代码\n• 代码审查和优化\n• 技术方案设计\n\n**系统管理**\n• 执行系统命令\n• 管理技能和工具\n• 创建工作流\n\n需要我帮你做什么？",
            ]),
            priority: 85,
            context_aware: false,
            handler: None,
        });
        
        // 5. Help
        self.register_command(Command {
            id: "help_general".to_string(),
            category: CommandCategory::Help,
            patterns: vec![
                Pattern::Exact("help".to_string()),
                Pattern::Exact("/?".to_string()),
                Pattern::Exact("帮助".to_string()),
                Pattern::Exact("怎么用".to_string()),
                Pattern::Contains("需要帮助".to_string()),
                Pattern::Regex(r"^help (me|us)".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "小螃蟹🦀 的使用指南：\n\n**基本交互**\n• 直接输入问题或任务描述\n• 我会根据复杂度自动选择处理方式\n\n**快捷命令**\n• `/search <关键词>` - 搜索信息\n• `/skill <技能名>` - 管理技能\n• `/tool <工具名>` - 调用工具\n\n**获取帮助**\n• 问「你能做什么」了解我的能力\n• 问「系统状态」查看运行状况\n\n有什么具体想做的吗？",
            ]),
            priority: 85,
            context_aware: false,
            handler: None,
        });
        
        // 6. Status
        self.register_command(Command {
            id: "status_check".to_string(),
            category: CommandCategory::Status,
            patterns: vec![
                Pattern::Exact("status".to_string()),
                Pattern::Exact("state".to_string()),
                Pattern::Exact("状态".to_string()),
                Pattern::Contains("系统状态".to_string()),
                Pattern::Contains("运行情况".to_string()),
                Pattern::Regex(r"^how (are|r) (u|you) (doing|running)".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "系统运行正常。所有子系统在线，响应延迟低。",
                "✓ System 1 (快速响应) - 在线\n✓ System 2 (深度分析) - 就绪\n✓ System 3 (元认知) - 就绪\n\n一切正常！",
            ]),
            priority: 80,
            context_aware: false,
            handler: None,
        });
        
        // 7. Time queries
        self.register_command(Command {
            id: "time_query".to_string(),
            category: CommandCategory::Time,
            patterns: vec![
                Pattern::Exact("what time is it".to_string()),
                Pattern::Exact("time".to_string()),
                Pattern::Exact("几点了".to_string()),
                Pattern::Exact("现在几点".to_string()),
                Pattern::Contains("current time".to_string()),
                Pattern::Regex(r"^what('s| is) the time".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "现在是 {time}。",
                "当前时间：{time}",
            ]),
            priority: 80,
            context_aware: false,
            handler: None,
        });
        
        // 8. Date queries
        self.register_command(Command {
            id: "date_query".to_string(),
            category: CommandCategory::Time,
            patterns: vec![
                Pattern::Exact("what day is it".to_string()),
                Pattern::Exact("what date is it".to_string()),
                Pattern::Exact("date".to_string()),
                Pattern::Exact("今天几号".to_string()),
                Pattern::Exact("今天星期几".to_string()),
                Pattern::Contains("today's date".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "今天是 {date}，{day}。",
            ]),
            priority: 80,
            context_aware: false,
            handler: None,
        });
        
        // 9. Emotion/How are you
        self.register_command(Command {
            id: "emotion_howareyou".to_string(),
            category: CommandCategory::Emotion,
            patterns: vec![
                Pattern::Exact("how are you".to_string()),
                Pattern::Exact("how r u".to_string()),
                Pattern::Exact("你好吗".to_string()),
                Pattern::Exact("怎么样".to_string()),
                Pattern::Regex(r"^how('s| is) it going".to_string()),
                Pattern::Regex(r"^how (are|r) (u|you) (feeling|doing)".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "我很好，谢谢关心！随时准备帮你解决问题。",
                "运行状态良好！有什么可以帮你的吗？",
                "我很好！能为你服务我很开心。",
            ]),
            priority: 75,
            context_aware: true,
            handler: None,
        });
        
        // 10. Gratitude
        self.register_command(Command {
            id: "gratitude_thanks".to_string(),
            category: CommandCategory::Gratitude,
            patterns: vec![
                Pattern::Exact("thanks".to_string()),
                Pattern::Exact("thank you".to_string()),
                Pattern::Exact("thx".to_string()),
                Pattern::Exact("ty".to_string()),
                Pattern::Exact("谢谢".to_string()),
                Pattern::Exact("感谢".to_string()),
                Pattern::Exact("多谢".to_string()),
                Pattern::Contains("thank".to_string()),
                Pattern::Regex(r"^thanks (a lot|so much)".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "不客气！很高兴能帮到你。",
                "不用谢！随时为你服务。",
                "别客气，有问题随时找我！",
                "能帮到你我也很开心！",
            ]),
            priority: 75,
            context_aware: true,
            handler: None,
        });
        
        // 11. Farewell
        self.register_command(Command {
            id: "farewell_goodbye".to_string(),
            category: CommandCategory::Farewell,
            patterns: vec![
                Pattern::Exact("bye".to_string()),
                Pattern::Exact("goodbye".to_string()),
                Pattern::Exact("see you".to_string()),
                Pattern::Exact("cya".to_string()),
                Pattern::Exact("再见".to_string()),
                Pattern::Exact("拜拜".to_string()),
                Pattern::Exact("回头见".to_string()),
                Pattern::Prefix("bye ".to_string()),
                Pattern::Prefix("goodbye ".to_string()),
                Pattern::Contains("have a good".to_string()),
                Pattern::Regex(r"^see (u|you) (later|soon|tomorrow)".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "再见！有需要随时找我。",
                "拜拜！祝你今天愉快。",
                "回头见！期待下次为你服务。",
            ]),
            priority: 75,
            context_aware: true,
            handler: None,
        });
        
        // 12. Confirmation
        self.register_command(Command {
            id: "confirm_yes".to_string(),
            category: CommandCategory::Confirmation,
            patterns: vec![
                Pattern::Exact("yes".to_string()),
                Pattern::Exact("y".to_string()),
                Pattern::Exact("yeah".to_string()),
                Pattern::Exact("yep".to_string()),
                Pattern::Exact("sure".to_string()),
                Pattern::Exact("ok".to_string()),
                Pattern::Exact("okay".to_string()),
                Pattern::Exact("好的".to_string()),
                Pattern::Exact("可以".to_string()),
                Pattern::Exact("行".to_string()),
                Pattern::Exact("是的".to_string()),
                Pattern::Exact("没错".to_string()),
                Pattern::Exact("对".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "好的！",
                "明白！",
                "收到！",
            ]),
            priority: 70,
            context_aware: true,
            handler: None,
        });
        
        // 13. Negation
        self.register_command(Command {
            id: "negation_no".to_string(),
            category: CommandCategory::Confirmation,
            patterns: vec![
                Pattern::Exact("no".to_string()),
                Pattern::Exact("n".to_string()),
                Pattern::Exact("nope".to_string()),
                Pattern::Exact("nah".to_string()),
                Pattern::Exact("不用了".to_string()),
                Pattern::Exact("不要".to_string()),
                Pattern::Exact("不行".to_string()),
                Pattern::Exact("不对".to_string()),
                Pattern::Exact("不是".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "好的，明白了。",
                "了解。",
                "没关系，还有其他我可以帮你的吗？",
            ]),
            priority: 70,
            context_aware: true,
            handler: None,
        });
        
        // 14. Joke request
        self.register_command(Command {
            id: "joke_request".to_string(),
            category: CommandCategory::Joke,
            patterns: vec![
                Pattern::Exact("tell me a joke".to_string()),
                Pattern::Exact("joke".to_string()),
                Pattern::Exact("讲个笑话".to_string()),
                Pattern::Exact("说个笑话".to_string()),
                Pattern::Contains("funny".to_string()),
                Pattern::Regex(r"^(tell|say) (me )?a joke".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "为什么程序员总是分不清万圣节和圣诞节？\n\n因为 Oct 31 == Dec 25！",
                "一个 SQL 语句走进酒吧，走到两张桌子中间问：'我可以 join 你们吗？'",
                "程序员最讨厌的四件事：\n1. 写注释\n2. 写文档\n3. 别人不写注释\n4. 别人不写文档",
            ]),
            priority: 65,
            context_aware: false,
            handler: None,
        });
        
        // 15. Compliment
        self.register_command(Command {
            id: "compliment_received".to_string(),
            category: CommandCategory::Compliment,
            patterns: vec![
                Pattern::Exact("good job".to_string()),
                Pattern::Exact("well done".to_string()),
                Pattern::Exact("awesome".to_string()),
                Pattern::Exact("great".to_string()),
                Pattern::Exact("excellent".to_string()),
                Pattern::Exact("perfect".to_string()),
                Pattern::Exact("amazing".to_string()),
                Pattern::Exact("太棒了".to_string()),
                Pattern::Exact("厉害".to_string()),
                Pattern::Exact("真棒".to_string()),
                Pattern::Contains("good work".to_string()),
                Pattern::Contains("nice work".to_string()),
                Pattern::Regex(r"^you('re|r) (great|awesome|amazing|good)".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "谢谢夸奖！我会继续努力的。",
                "很高兴能帮到你！",
                "你的认可让我很开心！",
            ]),
            priority: 65,
            context_aware: true,
            handler: None,
        });
        
        // 16. Apology/Complaint
        self.register_command(Command {
            id: "apology_response".to_string(),
            category: CommandCategory::Complaint,
            patterns: vec![
                Pattern::Exact("sorry".to_string()),
                Pattern::Exact("my bad".to_string()),
                Pattern::Exact("抱歉".to_string()),
                Pattern::Exact("对不起".to_string()),
                Pattern::Contains("i apologize".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "没关系！",
                "没事的，不用在意。",
                "别担心，我们继续！",
            ]),
            priority: 65,
            context_aware: true,
            handler: None,
        });
        
        // 17. Opinion
        self.register_command(Command {
            id: "opinion_what".to_string(),
            category: CommandCategory::Opinion,
            patterns: vec![
                Pattern::Regex(r"^what do you think (about|of)".to_string()),
                Pattern::Regex(r"^how do you feel about".to_string()),
                Pattern::Contains("你怎么看".to_string()),
                Pattern::Contains("你觉得呢".to_string()),
                Pattern::Contains("你的看法".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "作为 AI，我没有个人情感，但我可以基于已有信息为你分析...",
                "让我从几个角度分析一下...",
            ]),
            priority: 60,
            context_aware: true,
            handler: None,
        });
        
        // 18. Weather (placeholder - would need integration)
        self.register_command(Command {
            id: "weather_query".to_string(),
            category: CommandCategory::Weather,
            patterns: vec![
                Pattern::Exact("weather".to_string()),
                Pattern::Exact("天气".to_string()),
                Pattern::Contains("weather today".to_string()),
                Pattern::Contains("今天天气".to_string()),
                Pattern::Regex(r"^what('s| is) the weather".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "我无法直接获取实时天气数据。你可以尝试问：'搜索今天的天气'，我会帮你查找。",
            ]),
            priority: 60,
            context_aware: false,
            handler: None,
        });
        
        // 19. Clarification
        self.register_command(Command {
            id: "clarification_what".to_string(),
            category: CommandCategory::Clarification,
            patterns: vec![
                Pattern::Exact("what".to_string()),
                Pattern::Exact("what?".to_string()),
                Pattern::Exact("什么".to_string()),
                Pattern::Exact("什么？".to_string()),
                Pattern::Exact("huh".to_string()),
                Pattern::Exact("嗯？".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "有什么不清楚的地方吗？我可以再解释一下。",
                "需要我详细说明吗？",
            ]),
            priority: 55,
            context_aware: true,
            handler: None,
        });
        
        // 20. Small talk
        self.register_command(Command {
            id: "smalltalk_general".to_string(),
            category: CommandCategory::SmallTalk,
            patterns: vec![
                Pattern::Exact("really".to_string()),
                Pattern::Exact("interesting".to_string()),
                Pattern::Exact("cool".to_string()),
                Pattern::Exact("nice".to_string()),
                Pattern::Exact("wow".to_string()),
                Pattern::Exact("真的吗".to_string()),
                Pattern::Exact("有意思".to_string()),
                Pattern::Exact("酷".to_string()),
            ],
            response: ResponseTemplate::new(vec![
                "是啊！",
                "没错！",
                "😊",
            ]),
            priority: 50,
            context_aware: true,
            handler: None,
        });
        
        // Sort by priority (highest first)
        self.commands.sort_by(|a, b| b.priority.cmp(&a.priority));
    }
    
    /// Register a new command
    pub fn register_command(&mut self, command: Command) {
        self.commands.push(command);
    }
    
    /// Find best matching command
    pub fn find_best_match(&self, input: &str, context: &[Message]) -> Option<(MatchResult, &Command)> {
        let mut best_match: Option<(MatchResult, &Command)> = None;
        let mut best_confidence = MatchConfidence::None;
        
        let mut matcher = PatternMatcher::new();
        
        for command in &self.commands {
            for pattern in &command.patterns {
                if let Some((confidence, params)) = matcher.match_pattern(input, pattern) {
                    if confidence > best_confidence {
                        best_confidence = confidence;
                        best_match = Some((MatchResult {
                            command_id: command.id.clone(),
                            confidence,
                            matched_pattern: format!("{:?}", pattern),
                            extracted_params: params,
                            match_type: match pattern {
                                Pattern::Exact(_) => MatchType::Exact,
                                Pattern::Prefix(_) => MatchType::Prefix,
                                Pattern::Contains(_) => MatchType::Prefix,
                                Pattern::Regex(_) => MatchType::Regex,
                                Pattern::Fuzzy(_, _) => MatchType::Fuzzy,
                            },
                        }, command));
                    }
                }
            }
        }
        
        // Check context-based matching for context-aware commands
        if best_match.is_none() || best_confidence < MatchConfidence::Medium {
            let ctx_info = ContextHandler::analyze_context(context);
            
            // Context-based matching logic
            if ctx_info.expecting_confirmation {
                // Look for confirmation commands
                for command in &self.commands {
                    if command.category == CommandCategory::Confirmation {
                        // Simple check for yes/no patterns
                        let input_lower = input.trim().to_lowercase();
                        if ["yes", "y", "yeah", "好的", "可以", "no", "n", "nah", "不用"].contains(&input_lower.as_str()) {
                            return Some((MatchResult {
                                command_id: command.id.clone(),
                                confidence: MatchConfidence::Medium,
                                matched_pattern: "context_confirmation".to_string(),
                                extracted_params: HashMap::new(),
                                match_type: MatchType::Contextual,
                            }, command));
                        }
                    }
                }
            }
        }
        
        best_match
    }
    
    /// Generate response from matched command
    fn generate_response(&self, command: &Command, input: &str, params: &HashMap<String, String>, context: &[Message]) -> String {
        // Use custom handler if available
        if let Some(handler) = &command.handler {
            return handler(input, params, context);
        }
        
        // Use template-based response
        let vars = params.clone();
        
        // Add context modifier if applicable
        let ctx_info = ContextHandler::analyze_context(context);
        if command.context_aware {
            if let Some(modifier) = ContextHandler::get_context_modifier(&ctx_info) {
                let base_response = command.response.render(&vars);
                return format!("{}{}", modifier, base_response);
            }
        }
        
        command.response.render(&vars)
    }
}

#[async_trait]
impl CognitiveSystem for System1Enhanced {
    fn name(&self) -> &str {
        "System 1 (Enhanced Intuitive)"
    }
    
    async fn process(&self, input: &str, context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
        // Find best matching command
        if let Some((match_result, command)) = self.find_best_match(input, context) {
            let response = self.generate_response(command, input, &match_result.extracted_params, context);
            
            let trace = vec![TraceStep {
                step: 0,
                thought: format!(
                    "System 1 Match: {} (category: {:?}, confidence: {:?}, type: {:?})",
                    command.id, command.category, match_result.confidence, match_result.match_type
                ),
                action: Some("FastRespond".to_string()),
                action_input: Some(input.to_string()),
                observation: Some(format!("Matched pattern: {}", match_result.matched_pattern)),
            }];
            
            return Ok((response, trace));
        }
        
        // No match found - return error to fall through to System 2
        Err(CrabletError::NotFound(format!(
            "No System 1 match for: '{}'",
            input.chars().take(50).collect::<String>()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ContentPart;

    #[tokio::test]
    async fn test_greeting_patterns() {
        let system = System1Enhanced::new();
        
        let greetings = vec![
            "hello", "hi", "hey", "你好", "您好", "嗨", "哈喽",
            "Hello there", "Hi!", "HELLO",
        ];
        
        for greeting in greetings {
            let result = system.process(greeting, &[]).await;
            assert!(result.is_ok(), "Failed to match greeting: {}", greeting);
            let (response, _) = result.unwrap();
            assert!(!response.is_empty());
        }
    }
    
    #[tokio::test]
    async fn test_identity_patterns() {
        let system = System1Enhanced::new();
        
        let identity_queries = vec![
            "who are you", "你是谁", "what is your name", "你叫什么",
            "who r u", "what's your name",
        ];
        
        for query in identity_queries {
            let result = system.process(query, &[]).await;
            assert!(result.is_ok(), "Failed to match identity query: {}", query);
            let (response, _) = result.unwrap();
            assert!(response.contains("Crablet"));
        }
    }
    
    #[tokio::test]
    async fn test_time_queries() {
        let system = System1Enhanced::new();
        
        let time_queries = vec![
            "what time is it", "time", "几点了", "现在几点",
        ];
        
        for query in time_queries {
            let result = system.process(query, &[]).await;
            assert!(result.is_ok(), "Failed to match time query: {}", query);
        }
    }
    
    #[tokio::test]
    async fn test_gratitude_patterns() {
        let system = System1Enhanced::new();
        
        let thanks = vec![
            "thanks", "thank you", "thx", "ty", "谢谢", "感谢", "多谢",
            "thanks a lot", "thank you so much",
        ];
        
        for t in thanks {
            let result = system.process(t, &[]).await;
            assert!(result.is_ok(), "Failed to match gratitude: {}", t);
        }
    }
    
    #[tokio::test]
    async fn test_confirmation_patterns() {
        let system = System1Enhanced::new();
        
        let confirmations = vec![
            "yes", "y", "yeah", "yep", "sure", "ok", "okay",
            "好的", "可以", "行", "是的", "没错", "对",
        ];
        
        for c in confirmations {
            let result = system.process(c, &[]).await;
            assert!(result.is_ok(), "Failed to match confirmation: {}", c);
        }
    }
    
    #[tokio::test]
    async fn test_no_match_fallback() {
        let system = System1Enhanced::new();
        
        // Complex queries should fall through to System 2
        let complex_queries = vec![
            "analyze the stock market trends for the past year",
            "write a rust function to implement a binary search tree",
            "explain quantum computing in detail",
        ];
        
        for query in complex_queries {
            let result = system.process(query, &[]).await;
            assert!(result.is_err(), "Should not match complex query: {}", query);
        }
    }
    
    #[tokio::test]
    async fn test_context_awareness() {
        let system = System1Enhanced::new();
        
        // Create context with previous message
        let context = vec![
            Message {
                role: "assistant".to_string(),
                content: Some(vec![ContentPart::Text { text: "Do you want me to proceed?".to_string() }]),
                tool_calls: None,
                tool_call_id: None,
            }
        ];
        
        // Simple "yes" should match in context
        let result = system.process("yes", &context).await;
        assert!(result.is_ok());
    }
}
