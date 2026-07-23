import {
  Zap,
  Search,
  Code,
  Lightbulb,
  Route,
  Settings,
  Bot,
  Layers,
  Brain,
  Eye,
  Terminal,
  Database,
  ArrowRightLeft,
  Target,
  Gauge,
  Activity,
} from 'lucide-react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import type { CognitiveLayer, AgentParadigm, DecisionStepType } from './thinkingTypes';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// 图标映射
export const stepIcons: Record<DecisionStepType, React.ElementType> = {
  routing: Route,
  system: Settings,
  paradigm: Layers,
  agent: Bot,
  reasoning: Brain,
  search: Search,
  code: Code,
  insight: Lightbulb,
  reflection: Eye,
  planning: Target,
  'tool-call': Terminal,
  context: Database,
  'state-change': ArrowRightLeft,
  confidence: Gauge,
  intent: Target,
};

// 标签映射
export const stepLabels: Record<DecisionStepType, string> = {
  routing: '路由选择',
  system: '系统选择',
  paradigm: '范式切换',
  agent: '代理执行',
  reasoning: '推理思考',
  search: '知识检索',
  code: '代码分析',
  insight: '洞察发现',
  reflection: '反思验证',
  planning: '任务规划',
  'tool-call': '工具调用',
  context: '上下文管理',
  'state-change': '状态变更',
  confidence: '置信度评估',
  intent: '意图识别',
};

// 颜色映射
export const stepColors: Record<DecisionStepType, string> = {
  routing: 'text-cyan-400 bg-cyan-400/10 border-cyan-400/20',
  system: 'text-indigo-400 bg-indigo-400/10 border-indigo-400/20',
  paradigm: 'text-purple-400 bg-purple-400/10 border-purple-400/20',
  agent: 'text-rose-400 bg-rose-400/10 border-rose-400/20',
  reasoning: 'text-blue-400 bg-blue-400/10 border-blue-400/20',
  search: 'text-amber-400 bg-amber-400/10 border-amber-400/20',
  code: 'text-emerald-400 bg-emerald-400/10 border-emerald-400/20',
  insight: 'text-pink-400 bg-pink-400/10 border-pink-400/20',
  reflection: 'text-teal-400 bg-teal-400/10 border-teal-400/20',
  planning: 'text-orange-400 bg-orange-400/10 border-orange-400/20',
  'tool-call': 'text-lime-400 bg-lime-400/10 border-lime-400/20',
  context: 'text-sky-400 bg-sky-400/10 border-sky-400/20',
  'state-change': 'text-violet-400 bg-violet-400/10 border-violet-400/20',
  confidence: 'text-yellow-400 bg-yellow-400/10 border-yellow-400/20',
  intent: 'text-fuchsia-400 bg-fuchsia-400/10 border-fuchsia-400/20',
};

// 认知层标签
export const cognitiveLayerLabels: Record<CognitiveLayer, { label: string; desc: string; icon: React.ElementType }> = {
  system1: { label: 'System 1', desc: '快速直觉', icon: Zap },
  system2: { label: 'System 2', desc: '深度分析', icon: Brain },
  system3: { label: 'System 3', desc: '元认知反思', icon: Eye },
  unknown: { label: 'Unknown', desc: '未分类', icon: Activity },
};

// 范式标签
export const paradigmLabels: Record<AgentParadigm, { label: string; desc: string }> = {
  'single-turn': { label: 'Single-Turn', desc: '单轮对话' },
  'react': { label: 'ReAct', desc: '推理-行动循环' },
  'reflexion': { label: 'Reflexion', desc: '自我反思' },
  'plan-and-execute': { label: 'Plan & Execute', desc: '规划-执行' },
  'swarm': { label: 'Swarm', desc: '多代理协作' },
  'unknown': { label: 'Unknown', desc: '未分类' },
};

// 置信度颜色
export const getConfidenceColor = (score: number): string => {
  if (score >= 0.8) return 'text-emerald-400';
  if (score >= 0.6) return 'text-yellow-400';
  if (score >= 0.4) return 'text-orange-400';
  return 'text-rose-400';
};

// 置信度背景色
export const getConfidenceBgColor = (score: number): string => {
  if (score >= 0.8) return 'bg-emerald-400/20';
  if (score >= 0.6) return 'bg-yellow-400/20';
  if (score >= 0.4) return 'bg-orange-400/20';
  return 'bg-rose-400/20';
};
