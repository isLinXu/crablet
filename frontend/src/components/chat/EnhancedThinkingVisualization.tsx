import React, { useState, useMemo, useEffect, useRef } from 'react';
import {
  Brain, Zap, Search, Code, Lightbulb, Route, Settings, Bot, Layers,
  MessageSquare, GitBranch, Clock, Activity, BarChart3, ChevronDown, ChevronUp,
  Terminal, Cpu, Database, ArrowRightLeft, Target, Sparkles, Eye, FileText,
  Workflow, Gauge, History, Network, Share2, Pause, Play, RotateCcw,
  GitFork, MessageCircle, Wand2, Type, Table, Image as ImageIcon,
  Maximize2, Minimize2, Download, Bookmark, Filter, AlertCircle
} from 'lucide-react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

// 导入认知增强组件
import { ThoughtGraphViewer, ThoughtGraph, ThoughtNode, ThoughtEdge } from '../cognitive/ThoughtGraphViewer';
import { ThinkingStream, ThinkingToken } from '../cognitive/ThinkingStream';
import { SmartSuggestions, Suggestion } from '../cognitive/SmartSuggestions';
import { ActionableSmartSuggestions } from '../cognitive/ActionableSmartSuggestions';
import { ThinkingIntervention, InterventionRequest } from '../cognitive/ThinkingIntervention';
import { MultimodalThinking, ContentBlock } from '../cognitive/MultimodalThinking';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

import type { CognitiveLayer, AgentParadigm, DecisionStepDetails, DecisionStep, SystemSwitch, ParadigmSwitch, StackFrame, ThinkingProcess } from './thinkingTypes';
export type { CognitiveLayer, AgentParadigm, DecisionStepDetails, DecisionStep, SystemSwitch, ParadigmSwitch, StackFrame, ThinkingProcess } from './thinkingTypes';

// Enhanced 版特有的决策步骤类型（包含 'intent'）
export type DecisionStepType =
  | 'routing' | 'system' | 'paradigm' | 'agent' | 'reasoning'
  | 'search' | 'code' | 'insight' | 'reflection' | 'planning'
  | 'tool-call' | 'context' | 'state-change' | 'confidence' | 'intent';

// 质量指标
interface QualityMetrics {
  confidence: number;
  informationGain: number;
  backtrackCount: number;
  toolHitRate: number;
  averageStepDuration: number;
  tokenEfficiency: number;
}

// 认知负载
interface CognitiveLoad {
  system1: number;
  system2: number;
  system3: number;
}

interface EnhancedThinkingVisualizationProps {
  process: ThinkingProcess;
  isThinking: boolean;
  className?: string;
  onIntervene?: (request: InterventionRequest) => void;
  onSuggestionClick?: (suggestion: Suggestion) => void;
  onSendMessage?: (message: string) => void; // 新增：发送消息回调
  lastUserMessage?: string; // 新增：最后一条用户消息
  lastAssistantMessage?: string; // 新增：最后一条助手消息
  conversationHistory?: Array<{ role: string; content: string }>; // 新增：对话历史
}

// 图标映射
const stepIcons: Record<DecisionStepType, React.ElementType> = {
  routing: Route, system: Settings, paradigm: Layers, agent: Bot,
  reasoning: Brain, search: Search, code: Code, insight: Lightbulb,
  reflection: Eye, planning: Target, 'tool-call': Terminal,
  context: Database, 'state-change': ArrowRightLeft, confidence: Gauge,
  intent: Target,
};

// 标签映射
const stepLabels: Record<DecisionStepType, string> = {
  routing: '路由选择', system: '系统选择', paradigm: '范式切换', agent: '代理执行',
  reasoning: '推理思考', search: '知识检索', code: '代码分析', insight: '洞察发现',
  reflection: '反思验证', planning: '任务规划', 'tool-call': '工具调用',
  context: '上下文管理', 'state-change': '状态变更', confidence: '置信度评估',
  intent: '意图识别',
};

// 颜色映射
const stepColors: Record<DecisionStepType, string> = {
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
const cognitiveLayerLabels: Record<CognitiveLayer, { label: string; desc: string; icon: React.ElementType; color: string }> = {
  system1: { label: 'System 1', desc: '快速直觉', icon: Zap, color: 'yellow' },
  system2: { label: 'System 2', desc: '深度分析', icon: Brain, color: 'blue' },
  system3: { label: 'System 3', desc: '元认知反思', icon: Eye, color: 'purple' },
  unknown: { label: 'Unknown', desc: '未分类', icon: Activity, color: 'gray' },
};

// 范式标签
const paradigmLabels: Record<AgentParadigm, { label: string; desc: string }> = {
  'single-turn': { label: 'Single-Turn', desc: '单轮对话' },
  'react': { label: 'ReAct', desc: '推理-行动循环' },
  'reflexion': { label: 'Reflexion', desc: '自我反思' },
  'plan-and-execute': { label: 'Plan & Execute', desc: '规划-执行' },
  'swarm': { label: 'Swarm', desc: '多代理协作' },
  'unknown': { label: 'Unknown', desc: '未分类' },
};

// 置信度颜色
const getConfidenceColor = (score: number): string => {
  if (score >= 0.8) return 'text-emerald-400';
  if (score >= 0.6) return 'text-yellow-400';
  if (score >= 0.4) return 'text-orange-400';
  return 'text-rose-400';
};

const getConfidenceBgColor = (score: number): string => {
  if (score >= 0.8) return 'bg-emerald-400/20';
  if (score >= 0.6) return 'bg-yellow-400/20';
  if (score >= 0.4) return 'bg-orange-400/20';
  return 'bg-rose-400/20';
};

// 质量指标面板组件
const QualityMetricsPanel: React.FC<{ metrics: QualityMetrics; isThinking: boolean }> = ({ metrics, isThinking }) => {
  const MetricCard = ({ label, value, max = 100, unit = '%', icon: Icon, color }: any) => (
    <div className="flex items-center gap-2 p-2 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50">
      <div className={cn("w-8 h-8 rounded-lg flex items-center justify-center", color)}>
        <Icon className="w-4 h-4" />
      </div>
      <div className="flex-1 min-w-0">
        <div className="text-[10px] text-zinc-500">{label}</div>
        <div className="flex items-center gap-2">
          <div className="flex-1 h-1.5 bg-zinc-200 dark:bg-zinc-700 rounded-full overflow-hidden">
            <div 
              className={cn("h-full rounded-full transition-all duration-500", color.replace('text-', 'bg-').replace('/10', ''))}
              style={{ width: `${Math.min((value / max) * 100, 100)}%` }}
            />
          </div>
          <span className="text-xs font-medium text-zinc-700 dark:text-zinc-300">
            {typeof value === 'number' ? value.toFixed(1) : value}{unit}
          </span>
        </div>
      </div>
    </div>
  );

  return (
    <div className="grid grid-cols-2 gap-2 p-3 border-b border-zinc-200 dark:border-zinc-800">
      <MetricCard 
        label="置信度" 
        value={metrics.confidence * 100} 
        icon={Gauge} 
        color="text-blue-400 bg-blue-400/10" 
      />
      <MetricCard 
        label="信息增益" 
        value={metrics.informationGain * 100} 
        icon={Sparkles} 
        color="text-purple-400 bg-purple-400/10" 
      />
      <MetricCard 
        label="回溯次数" 
        value={metrics.backtrackCount} 
        max={10} 
        unit="" 
        icon={RotateCcw} 
        color="text-amber-400 bg-amber-400/10" 
      />
      <MetricCard 
        label="工具命中率" 
        value={metrics.toolHitRate * 100} 
        icon={Target} 
        color="text-emerald-400 bg-emerald-400/10" 
      />
    </div>
  );
};

// 认知负载指示器
const CognitiveLoadIndicator: React.FC<{ load: CognitiveLoad }> = ({ load }) => {
  const LoadBar = ({ label, value, color }: { label: string; value: number; color: string }) => (
    <div className="flex items-center gap-2 text-xs">
      <span className="w-20 text-zinc-500">{label}</span>
      <div className="flex-1 h-2 bg-zinc-200 dark:bg-zinc-700 rounded-full overflow-hidden">
        <div 
          className={cn("h-full rounded-full transition-all duration-500", color)}
          style={{ width: `${value}%` }}
        />
      </div>
      <span className="w-8 text-right text-zinc-600 dark:text-zinc-400">{value.toFixed(0)}%</span>
    </div>
  );

  return (
    <div className="p-3 border-b border-zinc-200 dark:border-zinc-800 space-y-2">
      <div className="flex items-center gap-2 text-xs text-zinc-500 mb-2">
        <Activity className="w-3.5 h-3.5" />
        <span>认知负载</span>
      </div>
      <LoadBar label="System 1" value={load.system1} color="bg-yellow-400" />
      <LoadBar label="System 2" value={load.system2} color="bg-blue-400" />
      <LoadBar label="System 3" value={load.system3} color="bg-purple-400" />
    </div>
  );
};

// 主组件
export const EnhancedThinkingVisualization: React.FC<EnhancedThinkingVisualizationProps> = ({
  process,
  isThinking,
  className,
  onIntervene,
  onSuggestionClick,
  onSendMessage,
  lastUserMessage = '',
  lastAssistantMessage = '',
  conversationHistory = [],
}) => {
  const [isExpanded, setIsExpanded] = useState(false); // 默认收起
  const [viewMode, setViewMode] = useState<'list' | 'graph' | 'stream'>('list');
  const [activeTab, setActiveTab] = useState<'steps' | 'switches' | 'stack' | 'metrics'>('steps');
  const [expandedSteps, setExpandedSteps] = useState<Set<string>>(new Set());
  const [isPaused, setIsPaused] = useState(false);
  const [showIntervention, setShowIntervention] = useState(false);
  const [selectedStepId, setSelectedStepId] = useState<string | null>(null);
  const [streamTokens, setStreamTokens] = useState<ThinkingToken[]>([]);
  const streamRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 计算质量指标
  const qualityMetrics: QualityMetrics = useMemo(() => {
    const completedSteps = process.steps.filter(s => s.duration);
    const avgDuration = completedSteps.length > 0 
      ? completedSteps.reduce((sum, s) => sum + (s.duration || 0), 0) / completedSteps.length 
      : 0;
    
    const toolCalls = process.steps.filter(s => s.type === 'tool-call');
    const successfulTools = toolCalls.filter(s => s.confidence && s.confidence > 0.5);
    
    return {
      confidence: process.confidence || 0.5,
      informationGain: Math.min(process.steps.length / 10, 1),
      backtrackCount: process.systemSwitches.length + process.paradigmSwitches.length,
      toolHitRate: toolCalls.length > 0 ? successfulTools.length / toolCalls.length : 0,
      averageStepDuration: avgDuration,
      tokenEfficiency: 0.75,
    };
  }, [process]);

  // 计算认知负载 - 基于当前认知层和步骤类型
  const cognitiveLoad: CognitiveLoad = useMemo(() => {
    // 基于当前认知层设置主要负载
    const currentLayer = process.currentLayer;
    
    // 根据步骤类型分布计算负载
    const stepTypes = process.steps.map(s => s.type);
    const hasReasoning = stepTypes.some(t => t === 'reasoning' || t === 'planning' || t === 'reflection');
    const hasToolCalls = stepTypes.some(t => t === 'tool-call' || t === 'search');
    const hasCode = stepTypes.some(t => t === 'code');
    
    // 基础活跃度计算（基于步骤数量和类型）
    const activityLevel = Math.min(process.steps.length * 5, 40);
    
    // 根据当前认知层分配负载：
    // - 当前激活的系统获得主要负载（60-90%）
    // - 其他系统获得基础负载（5-20%）
    
    let system1Load: number;
    let system2Load: number;
    let system3Load: number;
    
    switch (currentLayer) {
      case 'system1':
        // System 1 激活：快速响应，工具调用、代码执行
        system1Load = Math.min(70 + activityLevel + (hasToolCalls ? 15 : 0) + (hasCode ? 10 : 0), 95);
        system2Load = Math.min(10 + (hasReasoning ? 10 : 0), 25);
        system3Load = Math.min(5 + process.systemSwitches.length * 5, 20);
        break;
      case 'system2':
        // System 2 激活：深度分析，推理、规划
        system1Load = Math.min(15 + (hasToolCalls ? 15 : 0) + (hasCode ? 10 : 0), 35);
        system2Load = Math.min(75 + activityLevel + (hasReasoning ? 15 : 0), 98);
        system3Load = Math.min(5 + process.systemSwitches.length * 5, 20);
        break;
      case 'system3':
        // System 3 激活：元认知，范式切换、系统切换
        system1Load = Math.min(10 + (hasToolCalls ? 10 : 0) + (hasCode ? 5 : 0), 25);
        system2Load = Math.min(15 + (hasReasoning ? 10 : 0), 30);
        system3Load = Math.min(80 + process.systemSwitches.length * 5 + process.paradigmSwitches.length * 5, 98);
        break;
      default:
        // 未知层：均匀分布或基于步骤类型推断
        system1Load = Math.min(25 + (hasToolCalls ? 20 : 0) + (hasCode ? 15 : 0) + activityLevel, 60);
        system2Load = Math.min(25 + (hasReasoning ? 20 : 0) + activityLevel, 60);
        system3Load = Math.min(20 + process.systemSwitches.length * 10 + process.paradigmSwitches.length * 10, 50);
    }
    
    return {
      system1: Math.round(system1Load),
      system2: Math.round(system2Load),
      system3: Math.round(system3Load),
    };
  }, [process]);

  // 转换为思维图谱数据
  const thoughtGraph: ThoughtGraph = useMemo(() => {
    const now = Date.now();
    const nodes: Record<string, ThoughtNode> = {};
    
    process.steps.forEach((step, index) => {
      nodes[step.id] = {
        id: step.id,
        node_type: step.type === 'reasoning' ? 'reasoning' : 
                   step.type === 'tool-call' ? 'tool_call' :
                   step.type === 'search' ? 'observation' :
                   step.type === 'planning' ? 'planning' :
                   step.type === 'reflection' ? 'reflection' : 'decision',
        content: step.content,
        status: 'completed',
        parent_ids: index > 0 ? [process.steps[index - 1].id] : [],
        child_ids: index < process.steps.length - 1 ? [process.steps[index + 1].id] : [],
        alternative_ids: [],
        created_at: step.timestamp,
        completed_at: step.timestamp + (step.duration || 0),
        duration_ms: step.duration,
        confidence: step.confidence,
        depth: step.metadata?.depth || 0,
        metadata: step.metadata || {},
      };
    });

    const edges: ThoughtEdge[] = process.steps.slice(1).map((step, index) => ({
      id: `edge-${index}`,
      source: process.steps[index].id,
      target: step.id,
      edge_type: 'sequential',
      weight: 1,
    }));

    return { 
      id: `graph-${now}`,
      root_id: process.steps[0]?.id || '',
      nodes, 
      edges, 
      created_at: now,
      updated_at: now,
    };
  }, [process]);

  // 生成建议
  const suggestions: Suggestion[] = useMemo(() => {
    if (!isThinking && process.steps.length > 0) {
      return [
        { id: '1', text: '详细解释这一步', type: 'clarification', confidence: 0.9, icon: '💡' },
        { id: '2', text: '优化这个方案', type: 'action', confidence: 0.85, icon: '⚡' },
        { id: '3', text: '探索其他方法', type: 'exploration', confidence: 0.8, icon: '🔍' },
      ];
    }
    return [];
  }, [isThinking, process.steps.length]);

  // 模拟实时思考流
  useEffect(() => {
    if (isThinking && viewMode === 'stream' && !isPaused) {
      const tokens: ThinkingToken[] = process.steps.map(step => ({
        id: step.id,
        text: step.content,
        type: step.type === 'reasoning' ? 'thought' : 
              step.type === 'tool-call' ? 'tool' : 'observation',
        timestamp: step.timestamp,
        isComplete: true,
      }));

      let index = 0;
      streamRef.current = setInterval(() => {
        if (index < tokens.length) {
          setStreamTokens(prev => [...prev, tokens[index]]);
          index++;
        } else {
          if (streamRef.current) clearInterval(streamRef.current);
        }
      }, 100);

      return () => {
        if (streamRef.current) clearInterval(streamRef.current);
      };
    } else {
      setStreamTokens([]);
    }
  }, [isThinking, viewMode, isPaused, process.steps]);

  const toggleStepDetail = (stepId: string) => {
    setExpandedSteps(prev => {
      const newSet = new Set(prev);
      if (newSet.has(stepId)) {
        newSet.delete(stepId);
      } else {
        newSet.add(stepId);
      }
      return newSet;
    });
  };

  const handleIntervene = (request: InterventionRequest) => {
    onIntervene?.(request);
    
    // 将干预转换为实际消息发送
    if (onSendMessage) {
      let message = '';
      switch (request.type) {
        case 'correct':
          message = `[干预-纠正] ${request.userInput || '请重新考虑之前的回答'}`;
          break;
        case 'guide':
          message = `[干预-引导] ${request.userInput || '请按照我的引导继续'}`;
          break;
        case 'skip':
          message = '[干预-跳过] 请跳过当前步骤，继续下一步';
          break;
        case 'abort':
          message = '[干预-中止] 请停止当前思考，直接给出最终答案';
          break;
        case 'branch':
          message = `[干预-分支] 我选择方案: ${request.userInput || '备选方案'}`;
          break;
        default:
          message = `[干预] ${request.message}`;
      }
      onSendMessage(message);
    }
    
    setShowIntervention(false);
  };

  const stepCounts = useMemo(() => {
    return process.steps.reduce((acc, step) => {
      acc[step.type] = (acc[step.type] || 0) + 1;
      return acc;
    }, {} as Record<string, number>);
  }, [process.steps]);

  const totalDuration = process.totalDuration || 
    (process.endTime ? process.endTime - process.startTime : Date.now() - process.startTime);

  const currentLayerInfo = cognitiveLayerLabels[process.currentLayer];
  const currentParadigmInfo = paradigmLabels[process.currentParadigm];

  if (process.steps.length === 0 && !isThinking) return null;

  return (
    <div className={cn(
      "rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white/50 dark:bg-zinc-900/50 overflow-hidden",
      className
    )}>
      {/* 头部 */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-200 dark:border-zinc-800">
        <div className="flex items-center gap-3">
          <div className="relative">
            <Brain className="w-5 h-5 text-indigo-400" />
            {isThinking && (
              <span className="absolute -top-0.5 -right-0.5 w-2 h-2 bg-indigo-400 rounded-full animate-ping" />
            )}
          </div>
          <div className="flex flex-col">
            <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300">
              {isThinking ? 'Agent 思考中...' : '思考过程'}
            </span>
            <div className="flex items-center gap-2 mt-0.5">
              {isExpanded ? (
                // 展开状态：显示详细信息
                <>
                  <span className="text-[10px] text-zinc-400">
                    {process.steps.length} 个步骤 · {totalDuration}ms
                  </span>
                  {process.confidence > 0 && (
                    <span className={cn(
                      "text-[10px] px-1.5 py-0.5 rounded",
                      getConfidenceBgColor(process.confidence),
                      getConfidenceColor(process.confidence)
                    )}>
                      置信度 {(process.confidence * 100).toFixed(0)}%
                    </span>
                  )}
                </>
              ) : (
                // 收起状态：仅显示当前认知层
                <span className="text-[10px] text-zinc-400">
                  {cognitiveLayerLabels[process.currentLayer]?.label || '自动选择'}
                </span>
              )}
            </div>
          </div>
        </div>
        
        <div className="flex items-center gap-2">
          {/* 仅在展开时显示视图切换和控制按钮 */}
          {isExpanded && (
            <>
              {/* 视图切换 */}
              <div className="flex items-center bg-zinc-100 dark:bg-zinc-800 rounded-lg p-0.5">
                <button
                  onClick={() => setViewMode('list')}
                  className={cn(
                    "px-2 py-1 rounded text-xs transition-colors",
                    viewMode === 'list' 
                      ? "bg-white dark:bg-zinc-700 text-zinc-700 dark:text-zinc-300 shadow-sm" 
                      : "text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
                  )}
                >
                  <History className="w-3.5 h-3.5" />
                </button>
                <button
                  onClick={() => setViewMode('graph')}
                  className={cn(
                    "px-2 py-1 rounded text-xs transition-colors",
                    viewMode === 'graph' 
                      ? "bg-white dark:bg-zinc-700 text-zinc-700 dark:text-zinc-300 shadow-sm" 
                      : "text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
                  )}
                >
                  <Network className="w-3.5 h-3.5" />
                </button>
                <button
                  onClick={() => setViewMode('stream')}
                  className={cn(
                    "px-2 py-1 rounded text-xs transition-colors",
                    viewMode === 'stream' 
                      ? "bg-white dark:bg-zinc-700 text-zinc-700 dark:text-zinc-300 shadow-sm" 
                      : "text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
                  )}
                >
                  <Type className="w-3.5 h-3.5" />
                </button>
              </div>

              {/* 暂停/继续 */}
              {isThinking && (
                <button
                  onClick={() => setIsPaused(!isPaused)}
                  className={cn(
                    "p-1.5 rounded-lg transition-colors",
                    isPaused 
                      ? "bg-emerald-500/20 text-emerald-400" 
                      : "bg-zinc-100 dark:bg-zinc-800 text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
                  )}
                >
                  {isPaused ? <Play className="w-3.5 h-3.5" /> : <Pause className="w-3.5 h-3.5" />}
                </button>
              )}

              {/* 干预按钮 */}
              <button
                onClick={() => setShowIntervention(!showIntervention)}
                className={cn(
                  "p-1.5 rounded-lg transition-colors",
                  showIntervention 
                    ? "bg-amber-500/20 text-amber-400" 
                    : "bg-zinc-100 dark:bg-zinc-800 text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
                )}
              >
                <Wand2 className="w-3.5 h-3.5" />
              </button>
            </>
          )}

          {/* 展开/收起按钮 - 始终显示 */}
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="p-1.5 rounded-lg bg-zinc-100 dark:bg-zinc-800 text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300 transition-colors"
          >
            {isExpanded ? <Minimize2 className="w-3.5 h-3.5" /> : <Maximize2 className="w-3.5 h-3.5" />}
          </button>
        </div>
      </div>

      {/* 认知负载指示器 - 仅在展开时显示 */}
      {isExpanded && <CognitiveLoadIndicator load={cognitiveLoad} />}

      {/* 质量指标面板 */}
      {isExpanded && (
        <QualityMetricsPanel metrics={qualityMetrics} isThinking={isThinking} />
      )}

      {/* 干预面板 */}
      {showIntervention && (
        <div className="border-b border-zinc-200 dark:border-zinc-800">
          <ThinkingIntervention
            isActive={showIntervention}
            currentNodeId={selectedStepId || undefined}
            canBacktrack={process.steps.length > 1}
            onIntervene={handleIntervene}
            onConfirmTool={() => {}}
          />
        </div>
      )}

      {/* 主要内容区域 */}
      {isExpanded && (
        <div className="border-t border-zinc-200 dark:border-zinc-800">
          {/* Tab 导航 */}
          <div className="flex border-b border-zinc-200 dark:border-zinc-800">
            <button
              onClick={() => setActiveTab('steps')}
              className={cn(
                "flex items-center gap-1.5 px-4 py-2 text-xs font-medium transition-colors",
                activeTab === 'steps' 
                  ? "text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400" 
                  : "text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
              )}
            >
              <History className="w-3.5 h-3.5" />
              思考步骤
            </button>
            <button
              onClick={() => setActiveTab('switches')}
              className={cn(
                "flex items-center gap-1.5 px-4 py-2 text-xs font-medium transition-colors",
                activeTab === 'switches' 
                  ? "text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400" 
                  : "text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
              )}
            >
              <GitBranch className="w-3.5 h-3.5" />
              系统切换
              {process.systemSwitches.length > 0 && (
                <span className="ml-1 px-1.5 py-0.5 bg-zinc-200 dark:bg-zinc-700 rounded text-[10px]">
                  {process.systemSwitches.length}
                </span>
              )}
            </button>
            <button
              onClick={() => setActiveTab('stack')}
              className={cn(
                "flex items-center gap-1.5 px-4 py-2 text-xs font-medium transition-colors",
                activeTab === 'stack' 
                  ? "text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400" 
                  : "text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
              )}
            >
              <Layers className="w-3.5 h-3.5" />
              调用栈
              {process.callStack.length > 0 && (
                <span className="ml-1 px-1.5 py-0.5 bg-zinc-200 dark:bg-zinc-700 rounded text-[10px]">
                  {process.callStack.length}
                </span>
              )}
            </button>
            <button
              onClick={() => setActiveTab('metrics')}
              className={cn(
                "flex items-center gap-1.5 px-4 py-2 text-xs font-medium transition-colors",
                activeTab === 'metrics' 
                  ? "text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400" 
                  : "text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
              )}
            >
              <BarChart3 className="w-3.5 h-3.5" />
              详细指标
            </button>
          </div>

          {/* 内容区域 */}
          <div className="max-h-[500px] overflow-y-auto">
            {activeTab === 'steps' && (
              <>
                {/* 图谱视图 */}
                {viewMode === 'graph' && (
                  <div className="p-4">
                    <ThoughtGraphViewer
                      graph={thoughtGraph}
                      width={800}
                      height={400}
                      onNodeClick={(node) => setSelectedStepId(node.id)}
                    />
                  </div>
                )}

                {/* 流式视图 */}
                {viewMode === 'stream' && (
                  <div className="p-4">
                    <ThinkingStream
                      tokens={streamTokens}
                      phase={isThinking ? 'reasoning' : 'complete'}
                      metrics={{
                        tokensPerSecond: 10,
                        averageConfidence: qualityMetrics.confidence,
                        backtrackCount: qualityMetrics.backtrackCount,
                        toolHitRate: qualityMetrics.toolHitRate,
                        reasoningDepth: process.steps.length,
                        coherenceScore: 0.8,
                      }}
                    />
                  </div>
                )}

                {/* 列表视图 */}
                {viewMode === 'list' && (
                  <div className="p-4 space-y-3">
                    {process.steps.map((step, index) => {
                      const Icon = stepIcons[step.type];
                      const isDetailExpanded = expandedSteps.has(step.id);
                      const stepColor = stepColors[step.type];

                      return (
                        <div
                          key={step.id}
                          className="flex gap-3 group"
                        >
                          {/* 时间线 */}
                          <div className="flex flex-col items-center">
                            <button
                              onClick={() => setSelectedStepId(step.id)}
                              className={cn(
                                "w-8 h-8 rounded-lg flex items-center justify-center border transition-all",
                                stepColor,
                                selectedStepId === step.id && "ring-2 ring-offset-1 ring-blue-500"
                              )}
                            >
                              <Icon className="w-4 h-4" />
                            </button>
                            {index < process.steps.length - 1 && (
                              <div className="w-px flex-1 bg-zinc-200 dark:bg-zinc-800 my-1" />
                            )}
                          </div>
                          
                          {/* 内容 */}
                          <div className="flex-1 pb-3 min-w-0">
                            <div className="flex items-center gap-2 mb-1 flex-wrap">
                              <span className="text-xs font-medium text-zinc-500">
                                {stepLabels[step.type]}
                              </span>
                              <span className="text-[10px] text-zinc-400">
                                {new Date(step.timestamp).toLocaleTimeString('zh-CN', { 
                                  hour: '2-digit', minute: '2-digit', second: '2-digit' 
                                })}
                              </span>
                              {step.duration && (
                                <span className="text-[10px] text-zinc-400">
                                  {step.duration}ms
                                </span>
                              )}
                              {step.confidence !== undefined && (
                                <span className={cn(
                                  "text-[10px] px-1.5 py-0.5 rounded",
                                  getConfidenceBgColor(step.confidence),
                                  getConfidenceColor(step.confidence)
                                )}>
                                  {(step.confidence * 100).toFixed(0)}%
                                </span>
                              )}
                              {/* 干预按钮 */}
                              <button
                                onClick={() => {
                                  setSelectedStepId(step.id);
                                  setShowIntervention(true);
                                }}
                                className="opacity-0 group-hover:opacity-100 text-[10px] text-amber-400 hover:text-amber-500 transition-opacity"
                              >
                                纠正
                              </button>
                            </div>
                            <p className="text-sm text-zinc-700 dark:text-zinc-300 leading-relaxed">
                              {step.content}
                            </p>
                          </div>
                        </div>
                      );
                    })}
                    
                    {isThinking && (
                      <div className="flex gap-3 animate-pulse">
                        <div className="w-8 h-8 rounded-lg flex items-center justify-center border border-indigo-400/30 bg-indigo-400/10">
                          <div className="flex gap-0.5">
                            <span className="w-1.5 h-1.5 bg-indigo-400 rounded-full animate-bounce" />
                            <span className="w-1.5 h-1.5 bg-indigo-400 rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
                            <span className="w-1.5 h-1.5 bg-indigo-400 rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
                          </div>
                        </div>
                        <div className="flex-1">
                          <span className="text-xs text-zinc-400">继续思考中...</span>
                        </div>
                      </div>
                    )}
                  </div>
                )}
              </>
            )}

            {activeTab === 'switches' && (
              <div className="p-4">
                <div className="space-y-4">
                  {process.systemSwitches.map((sw, index) => (
                    <div key={sw.id} className="flex items-center gap-3 text-sm">
                      <span className="text-zinc-400">#{index + 1}</span>
                      <span className={cn(
                        "px-2 py-1 rounded",
                        sw.from === 'system1' ? 'bg-yellow-500/20 text-yellow-400' :
                        sw.from === 'system2' ? 'bg-blue-500/20 text-blue-400' :
                        'bg-purple-500/20 text-purple-400'
                      )}>
                        {cognitiveLayerLabels[sw.from].label}
                      </span>
                      <ArrowRightLeft className="w-4 h-4 text-zinc-400" />
                      <span className={cn(
                        "px-2 py-1 rounded",
                        sw.to === 'system1' ? 'bg-yellow-500/20 text-yellow-400' :
                        sw.to === 'system2' ? 'bg-blue-500/20 text-blue-400' :
                        'bg-purple-500/20 text-purple-400'
                      )}>
                        {cognitiveLayerLabels[sw.to].label}
                      </span>
                      <span className="text-zinc-500">{sw.reason}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {activeTab === 'stack' && (
              <div className="p-4">
                <div className="space-y-2">
                  {process.callStack.map((frame, index) => (
                    <div 
                      key={frame.id}
                      className={cn(
                        "p-3 rounded-lg border text-sm",
                        frame.status === 'running' ? 'bg-blue-500/10 border-blue-500/20' : 
                        frame.status === 'error' ? 'bg-rose-500/10 border-rose-500/20' :
                        'bg-zinc-100 dark:bg-zinc-800 border-zinc-200 dark:border-zinc-700'
                      )}
                    >
                      <div className="flex items-center justify-between">
                        <span className="font-mono">{frame.function}</span>
                        <span className="text-xs text-zinc-400">
                          {frame.endTime ? `${frame.endTime - frame.startTime}ms` : '运行中'}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {activeTab === 'metrics' && (
              <div className="p-4">
                <div className="grid grid-cols-2 gap-4">
                  <div className="p-4 rounded-lg bg-zinc-100 dark:bg-zinc-800">
                    <div className="text-xs text-zinc-500 mb-1">总耗时</div>
                    <div className="text-2xl font-semibold">{totalDuration}ms</div>
                  </div>
                  <div className="p-4 rounded-lg bg-zinc-100 dark:bg-zinc-800">
                    <div className="text-xs text-zinc-500 mb-1">思考步骤</div>
                    <div className="text-2xl font-semibold">{process.steps.length}</div>
                  </div>
                  <div className="p-4 rounded-lg bg-zinc-100 dark:bg-zinc-800">
                    <div className="text-xs text-zinc-500 mb-1">系统切换</div>
                    <div className="text-2xl font-semibold">{process.systemSwitches.length}</div>
                  </div>
                  <div className="p-4 rounded-lg bg-zinc-100 dark:bg-zinc-800">
                    <div className="text-xs text-zinc-500 mb-1">范式切换</div>
                    <div className="text-2xl font-semibold">{process.paradigmSwitches.length}</div>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      {/* 智能建议和快捷操作 - 仅在展开时显示 */}
      {isExpanded && !isThinking && onSendMessage && (
        <div className="border-t border-zinc-200 dark:border-zinc-800">
          <ActionableSmartSuggestions
            lastUserMessage={lastUserMessage}
            lastAssistantMessage={lastAssistantMessage}
            conversationHistory={conversationHistory}
            onSendMessage={onSendMessage}
            visible={true}
          />
        </div>
      )}
    </div>
  );
};

export default EnhancedThinkingVisualization;
