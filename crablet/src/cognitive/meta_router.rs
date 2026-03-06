use crate::cognitive::classifier::Intent;
use crate::types::Message;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct InputFeatures {
    pub input_length: usize,
    pub token_count: usize,
    pub has_code: bool,
    pub has_question: bool,
    pub has_research_keyword: bool,
    pub context_turns: usize,
    pub complexity_bucket: u8,
    pub intent_embedding: [f32; 16],
}

#[derive(Clone, Debug)]
pub struct RoutingFeedback {
    pub route_key: String,
    pub choice: SystemChoice,
    pub latency_ms: u64,
    pub quality_score: f32,
    pub reward: f32,
}

#[derive(Clone, Debug)]
pub enum SystemChoice {
    System1,
    System2,
    System3,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChoiceMetrics {
    pub choice: String,
    pub count: u64,
    pub avg_reward: f32,
    pub avg_latency_ms: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct RoutingEvaluationReport {
    pub total_feedback: u64,
    pub avg_reward: f32,
    pub avg_latency_ms: f32,
    pub avg_quality_score: f32,
    pub recent_window: usize,
    pub by_choice: Vec<ChoiceMetrics>,
}

#[derive(Clone, Debug)]
struct ArmStats {
    count: u64,
    reward_sum: f32,
}

impl ArmStats {
    fn mean(&self) -> f32 {
        if self.count == 0 {
            0.5
        } else {
            self.reward_sum / self.count as f32
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContextualBandit {
    contexts: HashMap<u64, [ArmStats; 3]>,
    global: [ArmStats; 3],
    exploration: f32,
}

impl ContextualBandit {
    pub fn new(exploration: f32) -> Self {
        let init = || ArmStats {
            count: 0,
            reward_sum: 0.0,
        };
        Self {
            contexts: HashMap::new(),
            global: [init(), init(), init()],
            exploration,
        }
    }

    pub fn set_exploration(&mut self, exploration: f32) {
        self.exploration = exploration.clamp(0.05, 2.0);
    }

    pub fn select(&mut self, features: &InputFeatures, prior_means: [f32; 3], prior_strengths: [f32; 3]) -> SystemChoice {
        let key = bucket_key(features);
        let stats = self.contexts.entry(key).or_insert_with(|| {
            [
                ArmStats {
                    count: 0,
                    reward_sum: 0.0,
                },
                ArmStats {
                    count: 0,
                    reward_sum: 0.0,
                },
                ArmStats {
                    count: 0,
                    reward_sum: 0.0,
                },
            ]
        });
        let total_n = stats.iter().map(|s| s.count).sum::<u64>() as f32 + 1.0 + prior_strengths.iter().sum::<f32>();
        let mut best_arm = 1usize;
        let mut best_score = f32::MIN;
        for i in 0..3 {
            let c_stats = &stats[i];
            let g_stats = &self.global[i];
            let local_mean = blended_mean(c_stats, prior_means[i], prior_strengths[i]);
            let global_mean = g_stats.mean();
            let blended = if c_stats.count == 0 {
                0.4 * local_mean + 0.6 * global_mean
            } else {
                0.7 * local_mean + 0.3 * global_mean
            };
            let effective_n = c_stats.count as f32 + prior_strengths[i] + 1.0;
            let bonus = self.exploration * (total_n.ln() / effective_n).sqrt();
            let score = blended + bonus;
            if score > best_score {
                best_score = score;
                best_arm = i;
            }
        }
        match best_arm {
            0 => SystemChoice::System1,
            2 => SystemChoice::System3,
            _ => SystemChoice::System2,
        }
    }

    pub fn update(&mut self, features: &InputFeatures, choice: &SystemChoice, reward: f32) {
        let arm = match choice {
            SystemChoice::System1 => 0usize,
            SystemChoice::System2 => 1usize,
            SystemChoice::System3 => 2usize,
        };
        let key = bucket_key(features);
        let stats = self.contexts.entry(key).or_insert_with(|| {
            [
                ArmStats {
                    count: 0,
                    reward_sum: 0.0,
                },
                ArmStats {
                    count: 0,
                    reward_sum: 0.0,
                },
                ArmStats {
                    count: 0,
                    reward_sum: 0.0,
                },
            ]
        });
        let r = reward.clamp(0.0, 1.0);
        stats[arm].count += 1;
        stats[arm].reward_sum += r;
        self.global[arm].count += 1;
        self.global[arm].reward_sum += r;
    }
}

#[derive(Clone, Debug)]
struct PendingRoute {
    features: InputFeatures,
    choice: SystemChoice,
}

#[derive(Clone, Debug)]
pub struct MetaCognitiveRouter {
    bandit: ContextualBandit,
    feedback_buffer: VecDeque<RoutingFeedback>,
    pending: HashMap<String, PendingRoute>,
    max_feedback: usize,
}

impl Default for MetaCognitiveRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl MetaCognitiveRouter {
    pub fn new() -> Self {
        Self {
            bandit: ContextualBandit::new(0.55),
            feedback_buffer: VecDeque::with_capacity(512),
            pending: HashMap::new(),
            max_feedback: 512,
        }
    }

    pub fn set_exploration(&mut self, exploration: f32) {
        self.bandit.set_exploration(exploration);
    }

    pub fn route(&mut self, input: &str, context: &[Message], complexity_score: f32, intent: Intent) -> (SystemChoice, InputFeatures) {
        let features = self.extract_features(input, context, complexity_score);
        let choice = if matches!(intent, Intent::DeepResearch) || features.has_research_keyword {
            SystemChoice::System3
        } else if matches!(intent, Intent::Greeting | Intent::Help | Intent::Status) && !features.has_code {
            SystemChoice::System1
        } else {
            let (prior_means, prior_strengths) = cold_start_priors(&intent, &features);
            self.bandit.select(&features, prior_means, prior_strengths)
        };
        (choice, features)
    }

    pub fn begin_route(&mut self, session_id: &str, input: &str, choice: SystemChoice, features: InputFeatures) {
        let key = route_key(session_id, input);
        self.pending.insert(
            key,
            PendingRoute {
                features,
                choice,
            },
        );
    }

    pub fn record_feedback(&mut self, session_id: &str, input: &str, latency: Duration, quality_score: f32) {
        let key = route_key(session_id, input);
        let Some(pending) = self.pending.remove(&key) else {
            return;
        };
        let reward = quality_score.clamp(0.0, 1.0) * (1.0 - latency.as_secs_f32() / 30.0).max(0.0);
        self.bandit.update(&pending.features, &pending.choice, reward);
        if self.feedback_buffer.len() >= self.max_feedback {
            self.feedback_buffer.pop_front();
        }
        self.feedback_buffer.push_back(RoutingFeedback {
            route_key: key,
            choice: pending.choice,
            latency_ms: latency.as_millis() as u64,
            quality_score: quality_score.clamp(0.0, 1.0),
            reward,
        });
    }

    pub fn evaluation_report(&self, recent_window: usize) -> RoutingEvaluationReport {
        let window = recent_window.max(1);
        let start = self.feedback_buffer.len().saturating_sub(window);
        let slice = self.feedback_buffer.iter().skip(start);
        let mut total_feedback = 0u64;
        let mut reward_sum = 0.0f32;
        let mut latency_sum = 0.0f32;
        let mut quality_sum = 0.0f32;
        let mut choice_stats: HashMap<&'static str, (u64, f32, f32)> = HashMap::new();
        for f in slice {
            total_feedback += 1;
            reward_sum += f.reward;
            latency_sum += f.latency_ms as f32;
            quality_sum += f.quality_score;
            let key = choice_label(&f.choice);
            let entry = choice_stats.entry(key).or_insert((0, 0.0, 0.0));
            entry.0 += 1;
            entry.1 += f.reward;
            entry.2 += f.latency_ms as f32;
        }
        let avg_reward = if total_feedback == 0 { 0.0 } else { reward_sum / total_feedback as f32 };
        let avg_latency_ms = if total_feedback == 0 { 0.0 } else { latency_sum / total_feedback as f32 };
        let avg_quality_score = if total_feedback == 0 { 0.0 } else { quality_sum / total_feedback as f32 };
        let mut by_choice = vec![];
        for key in ["system1", "system2", "system3"] {
            let (count, rsum, lsum) = choice_stats.get(key).copied().unwrap_or((0, 0.0, 0.0));
            by_choice.push(ChoiceMetrics {
                choice: key.to_string(),
                count,
                avg_reward: if count == 0 { 0.0 } else { rsum / count as f32 },
                avg_latency_ms: if count == 0 { 0.0 } else { lsum / count as f32 },
            });
        }
        RoutingEvaluationReport {
            total_feedback,
            avg_reward,
            avg_latency_ms,
            avg_quality_score,
            recent_window: window,
            by_choice,
        }
    }

    fn extract_features(&self, input: &str, context: &[Message], complexity_score: f32) -> InputFeatures {
        let lower = input.to_lowercase();
        let token_count = input.split_whitespace().count().max(1);
        let has_code = input.contains("```")
            || lower.contains("fn ")
            || lower.contains("class ")
            || lower.contains("import ")
            || lower.contains("SELECT ");
        let has_question = input.contains('?') || input.contains('？');
        let has_research_keyword = ["research", "调研", "深入", "比较", "分析", "方案", "benchmark"]
            .iter()
            .any(|k| lower.contains(k));
        let complexity_bucket = (complexity_score.clamp(0.0, 1.0) * 10.0).round() as u8;
        let intent_embedding = embed_intent(&lower);
        InputFeatures {
            input_length: input.len(),
            token_count,
            has_code,
            has_question,
            has_research_keyword,
            context_turns: context.len(),
            complexity_bucket,
            intent_embedding,
        }
    }
}

fn bucket_key(features: &InputFeatures) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (features.input_length / 32).hash(&mut hasher);
    (features.token_count / 8).hash(&mut hasher);
    features.has_code.hash(&mut hasher);
    features.has_question.hash(&mut hasher);
    features.has_research_keyword.hash(&mut hasher);
    (features.context_turns / 4).hash(&mut hasher);
    features.complexity_bucket.hash(&mut hasher);
    for v in features.intent_embedding {
        ((v * 8.0).round() as i8).hash(&mut hasher);
    }
    hasher.finish()
}

fn blended_mean(stats: &ArmStats, prior_mean: f32, prior_strength: f32) -> f32 {
    let n = stats.count as f32 + prior_strength;
    if n <= 0.0 {
        0.5
    } else {
        (stats.reward_sum + prior_mean * prior_strength) / n
    }
}

fn cold_start_priors(intent: &Intent, features: &InputFeatures) -> ([f32; 3], [f32; 3]) {
    let mut means = [0.35, 0.55, 0.45];
    let mut strengths = [1.5, 2.5, 2.0];
    match intent {
        Intent::Greeting | Intent::Help | Intent::Status => {
            means = [0.85, 0.3, 0.2];
            strengths = [6.0, 1.5, 1.0];
        }
        Intent::DeepResearch => {
            means = [0.15, 0.45, 0.9];
            strengths = [1.0, 2.0, 7.0];
        }
        Intent::MultiStep | Intent::Analysis => {
            means = [0.2, 0.72, 0.62];
            strengths = [1.0, 5.5, 3.5];
        }
        Intent::Coding => {
            means = [0.2, 0.76, 0.58];
            strengths = [1.0, 6.0, 2.5];
        }
        Intent::Creative | Intent::General | Intent::Math => {}
    }
    if features.has_code {
        means[1] += 0.05;
        means[2] += 0.03;
    }
    if features.complexity_bucket >= 8 {
        means[2] += 0.06;
        strengths[2] += 1.0;
    }
    if features.context_turns <= 1 {
        strengths[1] += 0.5;
    }
    (means, strengths)
}

fn choice_label(choice: &SystemChoice) -> &'static str {
    match choice {
        SystemChoice::System1 => "system1",
        SystemChoice::System2 => "system2",
        SystemChoice::System3 => "system3",
    }
}

fn route_key(session_id: &str, input: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    session_id.hash(&mut hasher);
    input.hash(&mut hasher);
    format!("{}-{}", session_id, hasher.finish())
}

fn embed_intent(input: &str) -> [f32; 16] {
    let mut vec = [0.0f32; 16];
    let mut total = 0.0f32;
    for token in input.split_whitespace() {
        let idx = hash_to_dim(token, 16);
        vec[idx] += 1.0;
        total += 1.0;
    }
    if total == 0.0 {
        return vec;
    }
    for v in &mut vec {
        *v /= total;
    }
    vec
}

fn hash_to_dim(token: &str, dim: usize) -> usize {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut hasher);
    (hasher.finish() as usize) % dim
}
