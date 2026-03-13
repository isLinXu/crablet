//! 任务复杂度分析器
//!
//! 通过多维度特征分析，智能判断任务复杂度：
//! - 文本长度和结构
//! - 领域专业度
//! - 推理深度需求
//! - 创造性要求

use anyhow::Result;
use regex::Regex;
use serde::{Serialize, Deserialize};
use std::collections::HashSet;

use crate::types::{Message, ContentPart};

/// 复杂度级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Complexity {
    /// 简单: 日常对话、问候、简单问答
    /// 特征: <50词、无专业术语、单轮可完成
    Simple,
    /// 中等: 一般性分析、多轮对话、轻度推理
    /// 特征: 50-200词、少量专业术语、需上下文
    Medium,
    /// 复杂: 深度分析、复杂推理、创造性任务
    /// 特征: >200词、大量专业术语、多步推理
    Complex,
}

/// 任务特征分析结果
#[derive(Debug, Clone)]
pub struct TaskCharacteristics {
    /// 总词数
    pub word_count: usize,
    /// 句子数
    pub sentence_count: usize,
    /// 专业术语数
    pub technical_terms: usize,
    /// 问句数（表示需要推理）
    pub question_count: usize,
    /// 指令数（如"请分析"、"请解释"）
    pub instruction_count: usize,
    /// 创造性指标（如"创作"、"设计"）
    pub creativity_score: f32,
    /// 领域检测
    pub detected_domains: Vec<String>,
}

/// 复杂度分析器
pub struct ComplexityAnalyzer {
    /// 专业术语词典
    technical_terms: HashSet<String>,
    /// 领域关键词
    domain_keywords: HashSet<String>,
    /// 创造性关键词
    creativity_keywords: HashSet<String>,
}

impl Default for ComplexityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplexityAnalyzer {
    pub fn new() -> Self {
        let mut technical_terms = HashSet::new();
        technical_terms.extend(vec![
            "算法", "模型", "架构", "协议", "接口", "组件",
            "优化", "并发", "异步", "同步", "线程", "进程",
            "神经网络", "深度学习", "机器学习", "人工智能",
            "区块链", "加密", "哈希", "签名", "证书",
            "量子", "纠缠", "叠加", "观测", "坍缩",
            "微分", "积分", "矩阵", "向量", "张量",
            "概率", "统计", "分布", "假设", "检验",
        ].iter().map(|s| s.to_string()));

        let mut domain_keywords = HashSet::new();
        domain_keywords.extend(vec![
            "医学", "法律", "金融", "工程", "科学", "艺术",
            "哲学", "历史", "文学", "音乐", "体育", "教育",
            "计算机", "物理", "化学", "生物", "数学", "经济",
            "心理学", "社会学", "政治学", "人类学", "考古学",
        ].iter().map(|s| s.to_string()));

        let mut creativity_keywords = HashSet::new();
        creativity_keywords.extend(vec![
            "创作", "设计", "发明", "创新", "想象", "构思",
            "艺术", "文学", "诗歌", "故事", "小说", "绘画",
            "音乐", "作曲", "编曲", "演奏", "表演", "导演",
            "创意", "灵感", "独特", "原创", "新颖", "突破",
        ].iter().map(|s| s.to_string()));

        Self {
            technical_terms,
            domain_keywords,
            creativity_keywords,
        }
    }

    /// 分析消息列表的复杂度
    pub fn analyze(&self, messages: &[Message]) -> Result<Complexity> {
        let characteristics = self.extract_characteristics(messages)?;
        let complexity = self.calculate_complexity(&characteristics);
        
        tracing::debug!(
            "Complexity analysis: {:?} (words: {}, terms: {}, questions: {})",
            complexity, characteristics.word_count, 
            characteristics.technical_terms, characteristics.question_count
        );
        
        Ok(complexity)
    }

    /// 提取任务特征
    pub fn extract_characteristics(&self, messages: &[Message]) -> Result<TaskCharacteristics> {
        let mut total_text = String::new();
        let mut word_count = 0;
        let mut sentence_count = 0;
        let mut technical_terms = 0;
        let mut question_count = 0;
        let mut instruction_count = 0;
        let mut creativity_score = 0.0;
        let mut detected_domains = Vec::new();

        // 中文分词正则 (简化版)
        let word_regex = Regex::new(r"[\u4e00-\u9fa5]+|[a-zA-Z]+|\d+").unwrap();
        // 句子结束符
        let sentence_regex = Regex::new(r"[。！？.!?]+").unwrap();
        // 问句检测
        let question_regex = Regex::new(r"[？?]|(?:什么|为什么|如何|怎么|哪里|谁|何时|多少|是否|能否|可以吗)").unwrap();
        // 指令检测
        let instruction_regex = Regex::new(r"(?:请|帮我|给我|需要|要求|请分析|请解释|请总结|请列出|请描述|请比较|请评估)").unwrap();

        for message in messages {
            if let Some(content) = &message.content {
                for part in content {
                    if let ContentPart::Text { text } = part {
                        total_text.push_str(text);
                        total_text.push(' ');
                        
                        // 词数统计
                        let words: Vec<_> = word_regex.find_iter(text).collect();
                        word_count += words.len();
                        
                        // 句子数统计
                        let sentences: Vec<_> = sentence_regex.find_iter(text).collect();
                        sentence_count += sentences.len().max(1); // 至少1句
                        
                        // 专业术语检测
                        for term in &self.technical_terms {
                            if text.to_lowercase().contains(&term.to_lowercase()) {
                                technical_terms += 1;
                            }
                        }
                        
                        // 问句检测
                        let questions: Vec<_> = question_regex.find_iter(text).collect();
                        question_count += questions.len();
                        
                        // 指令检测
                        let instructions: Vec<_> = instruction_regex.find_iter(text).collect();
                        instruction_count += instructions.len();
                        
                        // 创造性评分
                        for keyword in &self.creativity_keywords {
                            if text.contains(keyword) {
                                creativity_score += 0.2;
                            }
                        }
                        creativity_score = if creativity_score > 1.0 { 1.0 } else { creativity_score };
                        
                        // 领域检测
                        for domain in &self.domain_keywords {
                            if text.contains(domain) && !detected_domains.contains(domain) {
                                detected_domains.push(domain.clone());
                            }
                        }
                    }
                }
            }
        }

        Ok(TaskCharacteristics {
            word_count,
            sentence_count,
            technical_terms,
            question_count,
            instruction_count,
            creativity_score,
            detected_domains,
        })
    }

    /// 根据特征计算复杂度
    pub fn calculate_complexity(&self, chars: &TaskCharacteristics) -> Complexity {
        let score = self.calculate_complexity_score(chars);
        
        // 根据总分确定复杂度
        match score {
            s if s < 3.5 => Complexity::Simple,
            s if s < 7.0 => Complexity::Medium,
            _ => Complexity::Complex,
        }
    }

    /// 计算复杂度分数 (0.0 - 10.0+)
    pub fn calculate_complexity_score(&self, chars: &TaskCharacteristics) -> f32 {
        let mut score = 0.0f32;
        
        // 1. 规模评分 (权重: 2.0)
        if chars.word_count > 500 {
            score += 2.0;
        } else if chars.word_count > 200 {
            score += 1.2;
        } else if chars.word_count > 50 {
            score += 0.5;
        }
        
        // 2. 专业性评分 (权重: 3.0)
        let term_density = if chars.word_count > 0 {
            chars.technical_terms as f32 / chars.word_count as f32
        } else {
            0.0
        };
        if term_density > 0.15 {
            score += 3.0;
        } else if term_density > 0.08 {
            score += 1.8;
        } else if term_density > 0.03 {
            score += 0.8;
        }
        
        // 3. 推理深度评分 (权重: 2.5)
        if chars.question_count >= 5 {
            score += 2.5;
        } else if chars.question_count >= 2 {
            score += 1.5;
        } else if chars.question_count >= 1 {
            score += 0.6;
        }
        
        // 4. 结构复杂度评分 (权重: 1.5)
        if chars.sentence_count > 10 {
            score += 1.5;
        } else if chars.sentence_count > 5 {
            score += 0.8;
        }
        
        // 5. 创造性与领域多样性 (权重: 1.0)
        if chars.creativity_score > 0.6 {
            score += 0.6;
        }
        if chars.detected_domains.len() >= 3 {
            score += 0.4;
        } else if !chars.detected_domains.is_empty() {
            score += 0.2;
        }
        
        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_complexity() {
        let analyzer = ComplexityAnalyzer::new();
        
        let messages = vec![Message {
            role: "user".to_string(),
            content: Some(vec![ContentPart::Text { 
                text: "Hello, how are you?".to_string() 
            }]),
            ..Default::default()
        }];
        
        let complexity = analyzer.analyze(&messages).unwrap();
        assert_eq!(complexity, Complexity::Simple);
    }

    #[test]
    fn test_complex_complexity() {
        let analyzer = ComplexityAnalyzer::new();
        
        let messages = vec![Message {
            role: "user".to_string(),
            content: Some(vec![ContentPart::Text { 
                text: r#"
                请详细分析量子计算对现代密码学的影响，包括：
                1. 量子算法（如Shor算法）如何威胁RSA和椭圆曲线加密
                2. 后量子密码学的发展方向，包括格密码、多变量密码等
                3. 当前NIST后量子密码标准的进展和评估
                4. 企业和政府应该如何准备迁移到后量子密码系统
                请提供具体的数学原理说明、实际案例分析和时间线预测。
                "#.to_string() 
            }]),
            ..Default::default()
        }];
        
        let complexity = analyzer.analyze(&messages).unwrap();
        assert_ne!(complexity, Complexity::Simple);
    }
}
