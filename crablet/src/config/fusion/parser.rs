//! Markdown Configuration Parser
//!
//! Parses OpenClaw-style Markdown configuration files into structured data.

use super::*;
use regex::Regex;
use std::collections::HashMap;

/// Parsed Markdown document with frontmatter and content
#[derive(Debug, Clone)]
pub struct MarkdownDocument {
    pub frontmatter: Option<String>,
    pub content: String,
    pub path: PathBuf,
}

impl MarkdownDocument {
    /// Parse a Markdown file with YAML frontmatter
    pub fn parse(content: &str, path: PathBuf) -> Result<Self, ParseError> {
        // Check for frontmatter (delimited by ---)
        let frontmatter_re = Regex::new(r"^---\s*\n(.*?)\n---\s*\n(.*)$").unwrap();
        
        if let Some(captures) = frontmatter_re.captures(content) {
            let frontmatter = captures.get(1).map(|m| m.as_str().to_string());
            let body = captures.get(2).map(|m| m.as_str().to_string())
                .unwrap_or_default();
            
            Ok(Self {
                frontmatter,
                content: body,
                path,
            })
        } else {
            // No frontmatter, treat entire content as body
            Ok(Self {
                frontmatter: None,
                content: content.to_string(),
                path,
            })
        }
    }
    
    /// Parse frontmatter as YAML
    pub fn parse_frontmatter<T: serde::de::DeserializeOwned>(&self) -> Result<Option<T>, ParseError> {
        match &self.frontmatter {
            Some(fm) => {
                let value: T = serde_yaml::from_str(fm)
                    .map_err(|e| ParseError::YamlError(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }
    
    /// Extract structured data from Markdown content
    pub fn extract_sections(&self) -> HashMap<String, String> {
        let mut sections = HashMap::new();
        let section_re = Regex::new(r"##+\s+(.+)\n").unwrap();
        
        let mut current_section = "introduction".to_string();
        let mut current_content = String::new();
        
        for line in self.content.lines() {
            if let Some(captures) = section_re.captures(line) {
                // Save previous section
                if !current_content.is_empty() {
                    sections.insert(
                        current_section.clone(),
                        current_content.trim().to_string()
                    );
                }
                
                // Start new section
                current_section = captures.get(1).unwrap().as_str().to_string();
                current_content = String::new();
            } else {
                current_content.push_str(line);
                current_content.push('\n');
            }
        }
        
        // Save last section
        if !current_content.is_empty() {
            sections.insert(current_section, current_content.trim().to_string());
        }
        
        sections
    }
}

/// Parser for AGENTS.md
pub struct AgentsParser;

impl AgentsParser {
    pub fn parse(doc: &MarkdownDocument) -> Result<AgentConfig, ParseError> {
        let frontmatter: Option<AgentsFrontmatter> = doc.parse_frontmatter()?;
        let sections = doc.extract_sections();
        
        // Build config from frontmatter and sections
        let config = AgentConfig {
            identity: Self::parse_identity(&frontmatter, &sections)?,
            capabilities: Self::parse_capabilities(&frontmatter, &sections)?,
            behavior: Self::parse_behavior(&frontmatter, &sections)?,
        };
        
        Ok(config)
    }
    
    fn parse_identity(
        frontmatter: &Option<AgentsFrontmatter>,
        sections: &HashMap<String, String>
    ) -> Result<IdentityConfig, ParseError> {
        // Try frontmatter first
        if let Some(fm) = frontmatter {
            return Ok(IdentityConfig {
                name: fm.metadata.name.clone(),
                description: fm.metadata.description.clone(),
                role: fm.metadata.role.clone(),
                avatar: fm.metadata.avatar.clone(),
            });
        }
        
        // Fall back to parsing sections
        let identity_section = sections.get("身份")
            .or_else(|| sections.get("Identity"))
            .ok_or_else(|| ParseError::MissingSection("身份/Identity".to_string()))?;
        
        // Parse from Markdown content
        let name_re = Regex::new(r"\*\*名称\*\*[:：]\s*(.+)|\*\*Name\*\*[:：]\s*(.+)").unwrap();
        let name = name_re.captures(identity_section)
            .and_then(|c| c.get(1).or(c.get(2)))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| "Crablet".to_string());
        
        Ok(IdentityConfig {
            name,
            description: "Your intelligent assistant".to_string(),
            role: "assistant".to_string(),
            avatar: None,
        })
    }
    
    fn parse_capabilities(
        frontmatter: &Option<AgentsFrontmatter>,
        _sections: &HashMap<String, String>
    ) -> Result<CapabilitiesConfig, ParseError> {
        if let Some(fm) = frontmatter {
            return Ok(fm.capabilities.clone());
        }
        
        // Default capabilities
        Ok(CapabilitiesConfig {
            rag: RagCapability {
                enabled: true,
                backend: "hybrid".to_string(),
                vector_store: "qdrant".to_string(),
                graph_store: None,
            },
            memory: MemoryCapability {
                layers: 4,
                daily_logs: true,
                consolidation: true,
                cross_session: true,
            },
            cognitive: CognitiveCapability {
                router: "adaptive".to_string(),
                system1: true,
                system2: true,
                system3: true,
            },
            skills: SkillsCapability {
                local: true,
                mcp: true,
                openclaw: true,
                plugin: true,
                hot_reload: true,
            },
            channels: vec!["web".to_string()],
        })
    }
    
    fn parse_behavior(
        _frontmatter: &Option<AgentsFrontmatter>,
        sections: &HashMap<String, String>
    ) -> Result<BehaviorConfig, ParseError> {
        // Parse from 行为准则 section
        let behavior_section = sections.get("行为准则")
            .or_else(|| sections.get("Behavior"));
        
        let proactivity = if let Some(section) = behavior_section {
            if section.contains("主动") || section.contains("active") {
                ProactivityLevel::Active
            } else if section.contains("被动") || section.contains("passive") {
                ProactivityLevel::Passive
            } else {
                ProactivityLevel::Balanced
            }
        } else {
            ProactivityLevel::Balanced
        };
        
        Ok(BehaviorConfig {
            proactivity,
            response_style: ResponseStyle::Balanced,
            safety_level: SafetyLevel::Balanced,
        })
    }
}

/// Frontmatter structure for AGENTS.md
#[derive(Debug, Clone, Deserialize)]
struct AgentsFrontmatter {
    metadata: AgentsMetadata,
    capabilities: CapabilitiesConfig,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentsMetadata {
    name: String,
    description: String,
    role: String,
    avatar: Option<String>,
}

/// Parser for SOUL.md
pub struct SoulParser;

impl SoulParser {
    pub fn parse(doc: &MarkdownDocument) -> Result<SoulConfig, ParseError> {
        let frontmatter: Option<SoulFrontmatter> = doc.parse_frontmatter()?;
        let sections = doc.extract_sections();
        
        Ok(SoulConfig {
            personality: Self::parse_personality(&frontmatter, &sections)?,
            values: Self::parse_values(&frontmatter, &sections)?,
            principles: Self::parse_principles(&frontmatter, &sections)?,
            cognitive_profile: Self::parse_cognitive_profile(&frontmatter, &sections)?,
        })
    }
    
    fn parse_personality(
        frontmatter: &Option<SoulFrontmatter>,
        sections: &HashMap<String, String>
    ) -> Result<PersonalityConfig, ParseError> {
        if let Some(fm) = frontmatter {
            return Ok(fm.personality.clone());
        }
        
        // Parse from content
        let personality_section = sections.get("人格特质")
            .or_else(|| sections.get("Personality"));
        
        let traits = if let Some(section) = personality_section {
            // Extract traits from bullet points
            section.lines()
                .filter(|l| l.trim().starts_with('-') || l.trim().starts_with('*'))
                .map(|l| l.trim().trim_start_matches('-').trim_start_matches('*').trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            vec!["friendly".to_string(), "professional".to_string()]
        };
        
        Ok(PersonalityConfig {
            name: "小螃蟹".to_string(),
            traits,
            communication_style: "adaptive".to_string(),
            thinking_pattern: "analytical".to_string(),
        })
    }
    
    fn parse_values(
        frontmatter: &Option<SoulFrontmatter>,
        sections: &HashMap<String, String>
    ) -> Result<Vec<ValueConfig>, ParseError> {
        if let Some(fm) = frontmatter {
            return Ok(fm.values.clone());
        }
        
        // Parse from 核心价值观 section
        let values_section = sections.get("核心价值观")
            .or_else(|| sections.get("Core Values"));
        
        let values = if let Some(section) = values_section {
            // Extract numbered values
            let value_re = Regex::new(r"(\d+)\.\s*\*\*(.+?)\*\*[:：]?\s*(.*)").unwrap();
            value_re.captures_iter(section)
                .enumerate()
                .map(|(idx, cap)| ValueConfig {
                    name: cap.get(2).unwrap().as_str().to_string(),
                    priority: (10 - idx as u8).min(10).max(1),
                    description: cap.get(3).map(|m| m.as_str().to_string())
                        .unwrap_or_default(),
                })
                .collect()
        } else {
            vec![
                ValueConfig {
                    name: "user_first".to_string(),
                    priority: 10,
                    description: "用户至上".to_string(),
                },
            ]
        };
        
        Ok(values)
    }
    
    fn parse_principles(
        frontmatter: &Option<SoulFrontmatter>,
        sections: &HashMap<String, String>
    ) -> Result<Vec<PrincipleConfig>, ParseError> {
        if let Some(fm) = frontmatter {
            return Ok(fm.principles.clone());
        }
        
        // Parse from 不可变原则 section
        let principles_section = sections.get("不可变原则")
            .or_else(|| sections.get("Immutable Principles"));
        
        let principles = if let Some(section) = principles_section {
            section.lines()
                .filter(|l| l.trim().starts_with('-') || l.trim().starts_with('*'))
                .map(|l| {
                    let content = l.trim().trim_start_matches('-').trim_start_matches('*').trim();
                    PrincipleConfig {
                        name: content.split(':').next().unwrap_or(content).to_string(),
                        description: content.to_string(),
                        immutable: true,
                    }
                })
                .collect()
        } else {
            vec![
                PrincipleConfig {
                    name: "do_no_harm".to_string(),
                    description: "绝不伤害".to_string(),
                    immutable: true,
                },
            ]
        };
        
        Ok(principles)
    }
    
    fn parse_cognitive_profile(
        frontmatter: &Option<SoulFrontmatter>,
        _sections: &HashMap<String, String>
    ) -> Result<CognitiveProfileConfig, ParseError> {
        if let Some(fm) = frontmatter {
            return Ok(fm.cognitive_profile.clone());
        }
        
        // Default cognitive profile
        Ok(CognitiveProfileConfig {
            system1: System1Profile {
                enabled: true,
                intent_trie: "builtin".to_string(),
                fuzzy_matching: true,
                openclaw_prompts: true,
            },
            system2: System2Profile {
                enabled: true,
                react_engine: "enhanced".to_string(),
                middleware_chain: vec!["safety".to_string(), "rag".to_string()],
            },
            system3: System3Profile {
                enabled: true,
                swarm_coordinator: "default".to_string(),
                max_agents: 100,
            },
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SoulFrontmatter {
    personality: PersonalityConfig,
    values: Vec<ValueConfig>,
    principles: Vec<PrincipleConfig>,
    cognitive_profile: CognitiveProfileConfig,
}

/// Parser for USER.md
pub struct UserParser;

impl UserParser {
    pub fn parse(doc: &MarkdownDocument) -> Result<UserConfig, ParseError> {
        let frontmatter: Option<UserFrontmatter> = doc.parse_frontmatter()?;
        let sections = doc.extract_sections();
        
        Ok(UserConfig {
            profile: Self::parse_profile(&frontmatter, &sections)?,
            preferences: Self::parse_preferences(&frontmatter, &sections)?,
            privacy: Self::parse_privacy(&frontmatter, &sections)?,
        })
    }
    
    fn parse_profile(
        frontmatter: &Option<UserFrontmatter>,
        _sections: &HashMap<String, String>
    ) -> Result<UserProfileConfig, ParseError> {
        if let Some(fm) = frontmatter {
            return Ok(fm.profile.clone());
        }
        
        Ok(UserProfileConfig {
            user_id: "default".to_string(),
            name: None,
            role: None,
            expertise: vec![],
            goals: vec![],
        })
    }
    
    fn parse_preferences(
        frontmatter: &Option<UserFrontmatter>,
        _sections: &HashMap<String, String>
    ) -> Result<UserPreferencesConfig, ParseError> {
        if let Some(fm) = frontmatter {
            return Ok(fm.preferences.clone());
        }
        
        Ok(UserPreferencesConfig {
            language: "zh-CN".to_string(),
            response_length: ResponseLength::Moderate,
            format_preferences: FormatPreferencesConfig {
                use_markdown: true,
                use_tables: true,
                use_code_blocks: true,
                use_emoji: false,
            },
            proactive_behavior: ProactiveBehaviorConfig {
                suggest_related: true,
                ask_clarification: true,
                summarize_conversation: false,
                recommend_next: false,
            },
        })
    }
    
    fn parse_privacy(
        frontmatter: &Option<UserFrontmatter>,
        _sections: &HashMap<String, String>
    ) -> Result<PrivacyConfig, ParseError> {
        if let Some(fm) = frontmatter {
            return Ok(fm.privacy.clone());
        }
        
        Ok(PrivacyConfig {
            store_history: true,
            learn_preferences: true,
            share_anonymous_data: false,
            cross_session_identification: false,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct UserFrontmatter {
    profile: UserProfileConfig,
    preferences: UserPreferencesConfig,
    privacy: PrivacyConfig,
}

/// Main configuration parser that combines all parsers
pub struct FusionConfigParser;

impl FusionConfigParser {
    /// Parse all Markdown configuration files from workspace
    pub async fn parse_workspace(workspace_path: &PathBuf) -> Result<FusionConfig, ParseError> {
        // Read all config files
        let agents_content = tokio::fs::read_to_string(workspace_path.join("AGENTS.md")).await
            .map_err(|e| ParseError::IoError(e.to_string()))?;
        let soul_content = tokio::fs::read_to_string(workspace_path.join("SOUL.md")).await
            .map_err(|e| ParseError::IoError(e.to_string()))?;
        let user_content = tokio::fs::read_to_string(workspace_path.join("USER.md")).await
            .map_err(|e| ParseError::IoError(e.to_string()))?;
        
        // Parse each document
        let agents_doc = MarkdownDocument::parse(&agents_content, workspace_path.join("AGENTS.md"))?;
        let soul_doc = MarkdownDocument::parse(&soul_content, workspace_path.join("SOUL.md"))?;
        let user_doc = MarkdownDocument::parse(&user_content, workspace_path.join("USER.md"))?;
        
        // Parse individual configs
        let agent = AgentsParser::parse(&agents_doc)?;
        let soul = SoulParser::parse(&soul_doc)?;
        let user = UserParser::parse(&user_doc)?;
        
        // Build fusion config
        let config = FusionConfig {
            metadata: ConfigMetadata {
                name: agent.identity.name.clone(),
                version: "2.0.0".to_string(),
                edition: "fusion".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
            agent,
            soul,
            user,
            memory: MemoryConfig {
                working: WorkingMemoryConfig {
                    capacity_messages: 20,
                    max_tokens: 8000,
                    compression_strategy: "hybrid".to_string(),
                },
                episodic: EpisodicMemoryConfig {
                    backend: "sqlite".to_string(),
                    database_url: "sqlite://./data/episodic.db".to_string(),
                    retention_days: 365,
                },
                semantic: SemanticMemoryConfig {
                    backend: "hybrid".to_string(),
                    vector_store: VectorStoreConfig {
                        provider: "qdrant".to_string(),
                        url: "http://localhost:6333".to_string(),
                        dimension: 1536,
                        distance: "cosine".to_string(),
                    },
                    graph_store: None,
                },
                daily_logs: DailyLogsConfig {
                    enabled: true,
                    log_dir: workspace_path.join("memory"),
                    format: "markdown".to_string(),
                    retention_days: 90,
                    auto_extract_memories: true,
                },
            },
            tools: ToolsConfig {
                registry: ToolRegistryConfig {
                    auto_load: true,
                    hot_reload: true,
                    scan_interval_secs: 30,
                    skill_dirs: vec![workspace_path.join("skills")],
                },
                permissions: ToolPermissionsConfig {
                    default_allow: vec!["read".to_string()],
                    default_deny: vec!["execute".to_string()],
                    require_confirmation: vec!["write".to_string()],
                },
                orchestration: ToolOrchestrationConfig {
                    max_parallel: 5,
                    timeout_secs: 30,
                    retry_attempts: 3,
                    enable_chaining: true,
                    enable_composition: true,
                },
            },
            heartbeat: HeartbeatConfig {
                enabled: true,
                timezone: "Asia/Shanghai".to_string(),
                tasks: HeartbeatTasksConfig {
                    daily: vec![],
                    weekly: vec![],
                    monthly: vec![],
                },
                health_checks: HealthChecksConfig {
                    enabled: true,
                    interval_secs: 60,
                    checks: vec![],
                },
            },
            engine: EngineConfig {
                performance: PerformanceConfig {
                    max_concurrent_sessions: 1000,
                    message_queue_size: 10000,
                    worker_threads: 8,
                    enable_zero_copy: true,
                },
                safety: SafetyConfig {
                    oracle_enabled: true,
                    sandbox_type: "docker".to_string(),
                    rate_limiting: RateLimitingConfig {
                        enabled: true,
                        requests_per_minute: 60,
                        tokens_per_day: 1000000,
                    },
                    content_filter: ContentFilterConfig {
                        enabled: true,
                        filter_level: "moderate".to_string(),
                        custom_rules: vec![],
                    },
                },
                observability: ObservabilityConfig {
                    logging: LoggingConfig {
                        level: "info".to_string(),
                        format: "json".to_string(),
                        output: "stdout".to_string(),
                    },
                    metrics: MetricsConfig {
                        enabled: true,
                        endpoint: None,
                        interval_secs: 15,
                    },
                    tracing: TracingConfig {
                        enabled: true,
                        jaeger_endpoint: None,
                        sampling_rate: 0.1,
                    },
                },
            },
        };
        
        Ok(config)
    }
}

#[derive(Debug)]
pub enum ParseError {
    IoError(String),
    YamlError(String),
    MissingSection(String),
    InvalidFormat(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::IoError(msg) => write!(f, "IO error: {}", msg),
            ParseError::YamlError(msg) => write!(f, "YAML error: {}", msg),
            ParseError::MissingSection(section) => write!(f, "Missing section: {}", section),
            ParseError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
        }
    }
}

impl std::error::Error for ParseError {}
