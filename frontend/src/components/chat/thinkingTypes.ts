// Shared thinking types for AgentThinkingVisualization and EnhancedThinkingVisualization

export type CognitiveLayer = 'system1' | 'system2' | 'system3' | 'unknown';

export type AgentParadigm =
  | 'single-turn'
  | 'react'
  | 'reflexion'
  | 'plan-and-execute'
  | 'swarm'
  | 'unknown';

export type DecisionStepType =
  | 'routing'
  | 'system'
  | 'paradigm'
  | 'agent'
  | 'reasoning'
  | 'search'
  | 'code'
  | 'insight'
  | 'reflection'
  | 'planning'
  | 'tool-call'
  | 'context'
  | 'state-change'
  | 'confidence'
  | 'intent';

export interface DecisionStepDetails {
  provider?: string;
  model?: string;
  vendor?: string;
  reason?: string;
  complexityScore?: number;
  systemPrompt?: string;
  triggerCondition?: string;
  fromParadigm?: string;
  toParadigm?: string;
  paradigmReason?: string;
  agentName?: string;
  agentType?: string;
  params?: Record<string, unknown>;
  thought?: string;
  action?: string;
  observation?: string;
  toolName?: string;
  toolDuration?: number;
  toolInput?: unknown;
  toolOutput?: unknown;
  tokenCount?: number;
  contextWindow?: number;
  memoryAccessed?: boolean;
  previousState?: string;
  currentState?: string;
  stateDiff?: unknown;
  confidenceScore?: number;
  confidenceReason?: string;
  alternatives?: string[];
  [key: string]: unknown;
}

export interface DecisionStep {
  id: string;
  type: DecisionStepType;
  title: string;
  content: string;
  details?: DecisionStepDetails;
  metadata?: Record<string, unknown>;
  timestamp: number;
  duration?: number;
  confidence?: number;
}

export interface SystemSwitch {
  id: string;
  from: CognitiveLayer;
  to: CognitiveLayer;
  reason: string;
  trigger: string;
  timestamp: number;
  confidence: number;
}

export interface ParadigmSwitch {
  id: string;
  from: AgentParadigm;
  to: AgentParadigm;
  reason: string;
  trigger: string;
  timestamp: number;
}

export interface StackFrame {
  id: string;
  function: string;
  args: Record<string, unknown>;
  startTime: number;
  endTime?: number;
  result?: Record<string, unknown>;
  status: 'running' | 'completed' | 'error';
}

export interface ThinkingProcess {
  steps: DecisionStep[];
  systemSwitches: SystemSwitch[];
  paradigmSwitches: ParadigmSwitch[];
  callStack: StackFrame[];
  currentLayer: CognitiveLayer;
  currentParadigm: AgentParadigm;
  startTime: number;
  endTime?: number;
  totalDuration?: number;
  confidence: number;
}
