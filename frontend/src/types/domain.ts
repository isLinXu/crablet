export type ContentPart =
  | { type: 'text'; text: string }
  | { type: 'image_url'; image_url: { url: string } };

export interface Message {
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: string | ContentPart[];
  timestamp?: string;
  id?: string;
}

export interface ChatSession {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export interface Skill {
  name: string;
  description: string;
  version: string;
  enabled: boolean;
}

export interface KnowledgeDocument {
  id: string;
  source: string;
  type: string;
  timestamp: string;
  content_preview: string;
}

export interface RegistrySkillItem {
  name: string;
  description: string;
  version: string;
  url: string;
  display_name?: string;
  author?: string;
  rating?: number;
  downloads?: number;
}

export interface SkillsShTopItem {
  source: string;
  skill_id: string;
  name: string;
  installs: number;
}

export interface BatchTestResult {
  name: string;
  installed: boolean;
  enabled: boolean;
  passed: boolean;
}

// 语义搜索结果
export interface SemanticSearchResult {
  skill_name: string;
  description: string;
  version: string;
  similarity_score: number;
  match_type: 'semantic' | 'keyword' | 'hybrid';
  tags: string[];
  author: string;
  category: string;
}

// 技能执行日志
export interface SkillExecutionLog {
  skill_name: string;
  timestamp: string;
  success: boolean;
  output: string;
  error?: string;
  execution_time_ms: number;
}

// 技能运行结果
export interface SkillRunResult {
  skill_name: string;
  success: boolean;
  output: string;
  execution_time_ms: number;
  timestamp: string;
}

export interface ApiKeyInfo {
  id: string;
  name: string;
  created_at: string;
  last_used_at?: string | null;
  revoked: boolean;
}

export interface RoutingSettings {
  enable_adaptive_routing: boolean;
  system2_threshold: number;
  system3_threshold: number;
  bandit_exploration: number;
  enable_hierarchical_reasoning: boolean;
  deliberate_threshold: number;
  meta_reasoning_threshold: number;
  mcts_simulations: number;
  mcts_exploration_weight: number;
  graph_rag_entity_mode: 'rule' | 'phrase' | 'hybrid';
}

export interface RoutingChoiceMetrics {
  choice: string;
  count: number;
  avg_reward: number;
  avg_latency_ms: number;
}

export interface RoutingEvaluationReport {
  total_feedback: number;
  avg_reward: number;
  avg_latency_ms: number;
  avg_quality_score: number;
  recent_window: number;
  by_choice: RoutingChoiceMetrics[];
  hierarchical?: {
    enabled: boolean;
    deliberate_threshold: number;
    meta_reasoning_threshold: number;
    mcts_simulations: number;
    mcts_exploration_weight: number;
  };
  hierarchical_stats?: {
    total_requests: number;
    deliberate_activations: number;
    meta_activations: number;
    strategy_switches: number;
    bfs_runs: number;
    dfs_runs: number;
    mcts_runs: number;
  };
}

export interface McpOverview {
  status: string;
  mcp_tools: number;
  resources: number;
  prompts: number;
  resource_items: Array<{ uri: string; name?: string; description?: string }>;
  prompt_items: Array<{ name: string; description?: string }>;
}

export interface DashboardStats {
  status: string;
  skills_count: number;
  active_tasks: number;
  system_load: string;
  skills: SkillManifest[];
}

export interface SkillManifest {
  name: string;
  version: string;
  description: string;
  author?: string;
  license?: string;
}

export interface TaskGraph {
  nodes: Record<string, TaskNode>;
  status?: 'Active' | 'Paused' | 'Completed' | 'Failed';
  id?: string;
}

export interface TaskNode {
  id: string;
  agent_role: string;
  prompt: string;
  dependencies: string[];
  status: TaskStatus;
  result?: string;
}

export type TaskStatus =
  | 'Pending'
  | { Running: { started_at: number } }
  | { Paused: { paused_at: number } }
  | { Completed: { duration: number } }
  | { Failed: { error: string; retries: number } };

export interface Pagination {
  page: number;
  limit: number;
  total: number;
  total_pages: number;
}

export interface SwarmTasksResponse {
  graphs: TaskGraph[];
  pagination?: Pagination;
}

export interface AgentInfo {
  name: string;
  description: string;
  capabilities: string[];
}

export interface SwarmStatsData {
  total_tasks: number;
  active: number;
  completed: number;
  failed: number;
  success_rate: number;
  avg_duration_sec: number;
}

export interface HitlReview {
  review_id: string;
  graph_id: string;
  task_id: string;
  agent_output: string;
  review_type: string | { type?: string; value?: string };
  deadline: string;
  options: string[];
}
