import React, { useState, useMemo } from 'react';
import {
  Brain,
  Zap,
  Search,
  Code,
  Lightbulb,
  Route,
  Settings,
  Bot,
  Layers,
  MessageSquare,
  GitBranch,
  Clock,
  Activity,
  BarChart3,
  ChevronDown,
  ChevronUp,
  Terminal,
  Cpu,
  Database,
  ArrowRightLeft,
  Target,
  Sparkles,
  Eye,
  FileText,
  Workflow,
  Gauge,
  History
} from 'lucide-react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// 认知层类型
export type CognitiveLayer = 'system1' | 'system2' | 'system3' | 'unknown';

// Agent 范式类型
export type AgentParadigm = 'single-turn' | 'react' | 'reflexion' | 'plan-and-execute' | 'swarm' | 'unknown';

// 决策步骤类型
export type DecisionStepType = 
  | 'routing'      // 路由选择
  | 'system'       // 系统选择
  | 'paradigm'     // 范式选择
  | 'agent'        // Agent 执行
  | 'reasoning'    // 推理
  | 'search'       // 检索
  | 'code'         // 代码
  | 'insight'      // 洞察
  | 'reflection'   // 反思
  | 'planning'     // 规划
  | 'tool-call'    // 工具调用
  | 'context'      // 上下文管理
  | 'state-change' // 状态变更
  | 'confidence';  // 置信度评估

// 决策步骤详情
export interface DecisionStepDetails {
  // 路由选择
  provider?: string;
  model?: string;
  vendor?: string;
  reason?: string;
  
  // 系统选择
  systemPrompt?: string;
  triggerCondition?: string;
  complexityScore?: number;
  
  // 范式选择
  fromParadigm?: string;
  toParadigm?: string;
  paradigmReason?: string;
  
  // Agent 执行
  agentName?: string;
  agentType?: string;
  params?: Record<string, any>;
  
  // 推理
  thought?: string;
  observation?: string;
  action?: string;
  actionInput?: string;
  
  // 工具调用
  toolName?: string;
  toolInput?: any;
  toolOutput?: any;
  toolDuration?: number;
  
  // 上下文
  contextWindow?: number;
  tokenCount?: number;
  memoryAccessed?: boolean;
  
  // 状态
  previousState?: string;
  currentState?: string;
  stateDiff?: any;
  
  // 置信度
  confidenceScore?: number;
  confidenceReason?: string;
  alternatives?: string[];
  
  // 性能
  duration?: number;
  latency?: number;
  throughput?: number;
}

// 决策步骤
export interface DecisionStep {
  id: string;
  type: DecisionStepType;
  title: string;
  content: string;
  timestamp: number;
  duration?: number;
  confidence?: number;
  details?: DecisionStepDetails;
  subSteps?: DecisionStep[];
  metadata?: {
    layer?: CognitiveLayer;
    paradigm?: AgentParadigm;
    iteration?: number;
    depth?: number;
  };
}

// 系统切换记录
export interface SystemSwitch {
  id: string;
  from: CognitiveLayer;
  to: CognitiveLayer;
  reason: string;
  trigger: string;
  timestamp: number;
  confidence: number;
}

// 范式切换记录
export interface ParadigmSwitch {
  id: string;
  from: AgentParadigm;
  to: AgentParadigm;
  reason: string;
  trigger: string;
  timestamp: number;
}

// 调用栈帧
export interface StackFrame {
  id: string;
  function: string;
  args: any;
  result?: any;
  startTime: number;
  endTime?: number;
  status: 'running' | 'completed' | 'error';
}

// 完整思考过程
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

interface AgentThinkingVisualizationProps {
  process: ThinkingProcess;
  isThinking: boolean;
  className?: string;
}

// 图标映射
const stepIcons: Record<DecisionStepType, React.ElementType> = {
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
};

// 标签映射
const stepLabels: Record<DecisionStepType, string> = {
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
};

// 认知层标签
const cognitiveLayerLabels: Record<CognitiveLayer, { label: string; desc: string; icon: React.ElementType }> = {
  system1: { label: 'System 1', desc: '快速直觉', icon: Zap },
  system2: { label: 'System 2', desc: '深度分析', icon: Brain },
  system3: { label: 'System 3', desc: '元认知反思', icon: Eye },
  unknown: { label: 'Unknown', desc: '未分类', icon: Activity },
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

// 置信度背景色
const getConfidenceBgColor = (score: number): string => {
  if (score >= 0.8) return 'bg-emerald-400/20';
  if (score >= 0.6) return 'bg-yellow-400/20';
  if (score >= 0.4) return 'bg-orange-400/20';
  return 'bg-rose-400/20';
};

// 渲染步骤详情
const StepDetailPanel: React.FC<{ step: DecisionStep }> = ({ step }) => {
  const { details, type } = step;
  if (!details) return null;

  return (
    <div className="mt-3 space-y-3 text-xs">
      {/* 路由选择详情 */}
      {type === 'routing' && details.provider && (
        <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-2">
          <div className="flex items-center gap-2">
            <span className="text-zinc-500 w-16">提供商:</span>
            <span className="font-medium text-zinc-700 dark:text-zinc-300">{details.vendor}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-zinc-500 w-16">模型:</span>
            <span className="font-medium text-zinc-700 dark:text-zinc-300">{details.model}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-zinc-500 w-16">原因:</span>
            <span className="text-zinc-600 dark:text-zinc-400">{details.reason}</span>
          </div>
          {details.complexityScore !== undefined && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500 w-16">复杂度:</span>
              <div className="flex-1 flex items-center gap-2">
                <div className="flex-1 h-1.5 bg-zinc-200 dark:bg-zinc-700 rounded-full overflow-hidden">
                  <div 
                    className="h-full bg-blue-500 rounded-full"
                    style={{ width: `${details.complexityScore * 100}%` }}
                  />
                </div>
                <span className="text-zinc-600 dark:text-zinc-400">{(details.complexityScore * 100).toFixed(0)}%</span>
              </div>
            </div>
          )}
        </div>
      )}

      {/* 系统选择详情 */}
      {type === 'system' && (
        <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-2">
          {details.systemPrompt && (
            <div>
              <div className="text-zinc-500 mb-1">System Prompt:</div>
              <div className="font-mono text-zinc-600 dark:text-zinc-400 bg-zinc-200/50 dark:bg-zinc-700/50 p-2 rounded">
                {details.systemPrompt}
              </div>
            </div>
          )}
          {details.triggerCondition && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500">触发条件:</span>
              <span className="text-zinc-600 dark:text-zinc-400">{details.triggerCondition}</span>
            </div>
          )}
        </div>
      )}

      {/* 范式切换详情 */}
      {type === 'paradigm' && (
        <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50">
          <div className="flex items-center gap-3">
            {details.fromParadigm && (
              <>
                <span className="px-2 py-1 bg-zinc-200 dark:bg-zinc-700 rounded text-zinc-600 dark:text-zinc-400">
                  {paradigmLabels[details.fromParadigm as AgentParadigm]?.label || details.fromParadigm}
                </span>
                <ArrowRightLeft className="w-4 h-4 text-zinc-400" />
              </>
            )}
            {details.toParadigm && (
              <span className="px-2 py-1 bg-purple-500/20 text-purple-400 rounded">
                {paradigmLabels[details.toParadigm as AgentParadigm]?.label || details.toParadigm}
              </span>
            )}
          </div>
          {details.paradigmReason && (
            <div className="mt-2 text-zinc-600 dark:text-zinc-400">
              原因: {details.paradigmReason}
            </div>
          )}
        </div>
      )}

      {/* Agent 执行详情 */}
      {type === 'agent' && details.agentName && (
        <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-2">
          <div className="flex items-center gap-2">
            <span className="text-zinc-500 w-16">代理:</span>
            <span className="font-medium text-zinc-700 dark:text-zinc-300">{details.agentName}</span>
          </div>
          {details.agentType && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500 w-16">类型:</span>
              <span className="text-zinc-600 dark:text-zinc-400">{details.agentType}</span>
            </div>
          )}
          {details.params && Object.keys(details.params).length > 0 && (
            <div>
              <span className="text-zinc-500">参数:</span>
              <div className="mt-1 flex flex-wrap gap-1">
                {Object.entries(details.params).map(([key, value]) => (
                  <span key={key} className="px-1.5 py-0.5 bg-zinc-200 dark:bg-zinc-700 rounded text-zinc-600 dark:text-zinc-400">
                    {key}: {String(value)}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* 推理详情 */}
      {type === 'reasoning' && (
        <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-2">
          {details.thought && (
            <div>
              <div className="text-zinc-500 mb-1">思考:</div>
              <div className="text-zinc-700 dark:text-zinc-300">{details.thought}</div>
            </div>
          )}
          {details.action && (
            <div>
              <div className="text-zinc-500 mb-1">行动:</div>
              <div className="text-zinc-700 dark:text-zinc-300">{details.action}</div>
            </div>
          )}
          {details.observation && (
            <div>
              <div className="text-zinc-500 mb-1">观察:</div>
              <div className="text-zinc-600 dark:text-zinc-400">{details.observation}</div>
            </div>
          )}
        </div>
      )}

      {/* 工具调用详情 */}
      {type === 'tool-call' && details.toolName && (
        <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-2">
          <div className="flex items-center gap-2">
            <span className="text-zinc-500 w-16">工具:</span>
            <span className="font-medium text-zinc-700 dark:text-zinc-300">{details.toolName}</span>
          </div>
          {details.toolDuration && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500 w-16">耗时:</span>
              <span className="text-zinc-600 dark:text-zinc-400">{details.toolDuration}ms</span>
            </div>
          )}
          {details.toolInput && (
            <div>
              <div className="text-zinc-500 mb-1">输入:</div>
              <pre className="text-xs bg-zinc-200/50 dark:bg-zinc-700/50 p-2 rounded overflow-x-auto">
                {JSON.stringify(details.toolInput, null, 2)}
              </pre>
            </div>
          )}
          {details.toolOutput && (
            <div>
              <div className="text-zinc-500 mb-1">输出:</div>
              <pre className="text-xs bg-zinc-200/50 dark:bg-zinc-700/50 p-2 rounded overflow-x-auto">
                {JSON.stringify(details.toolOutput, null, 2)}
              </pre>
            </div>
          )}
        </div>
      )}

      {/* 上下文详情 */}
      {type === 'context' && (
        <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-2">
          {details.tokenCount !== undefined && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500 w-20">Token 数:</span>
              <span className="text-zinc-700 dark:text-zinc-300">{details.tokenCount}</span>
            </div>
          )}
          {details.contextWindow !== undefined && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500 w-20">上下文窗口:</span>
              <span className="text-zinc-700 dark:text-zinc-300">{details.contextWindow}</span>
            </div>
          )}
          {details.memoryAccessed !== undefined && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500 w-20">访问记忆:</span>
              <span className={details.memoryAccessed ? 'text-emerald-400' : 'text-zinc-400'}>
                {details.memoryAccessed ? '是' : '否'}
              </span>
            </div>
          )}
        </div>
      )}

      {/* 状态变更详情 */}
      {type === 'state-change' && (
        <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-2">
          {details.previousState && details.currentState && (
            <div className="flex items-center gap-2">
              <span className="px-2 py-1 bg-zinc-200 dark:bg-zinc-700 rounded text-zinc-600 dark:text-zinc-400">
                {details.previousState}
              </span>
              <ArrowRightLeft className="w-4 h-4 text-zinc-400" />
              <span className="px-2 py-1 bg-violet-500/20 text-violet-400 rounded">
                {details.currentState}
              </span>
            </div>
          )}
          {details.stateDiff && (
            <div>
              <div className="text-zinc-500 mb-1">变更详情:</div>
              <pre className="text-xs bg-zinc-200/50 dark:bg-zinc-700/50 p-2 rounded overflow-x-auto">
                {JSON.stringify(details.stateDiff, null, 2)}
              </pre>
            </div>
          )}
        </div>
      )}

      {/* 置信度详情 */}
      {type === 'confidence' && details.confidenceScore !== undefined && (
        <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-2">
          <div className="flex items-center gap-2">
            <span className="text-zinc-500 w-16">置信度:</span>
            <div className="flex-1 flex items-center gap-2">
              <div className="flex-1 h-2 bg-zinc-200 dark:bg-zinc-700 rounded-full overflow-hidden">
                <div 
                  className={cn("h-full rounded-full", getConfidenceBgColor(details.confidenceScore))}
                  style={{ width: `${details.confidenceScore * 100}%` }}
                />
              </div>
              <span className={cn("font-medium", getConfidenceColor(details.confidenceScore))}>
                {(details.confidenceScore * 100).toFixed(1)}%
              </span>
            </div>
          </div>
          {details.confidenceReason && (
            <div className="text-zinc-600 dark:text-zinc-400">
              评估依据: {details.confidenceReason}
            </div>
          )}
          {details.alternatives && details.alternatives.length > 0 && (
            <div>
              <div className="text-zinc-500 mb-1">备选方案:</div>
              <div className="flex flex-wrap gap-1">
                {details.alternatives.map((alt, idx) => (
                  <span key={idx} className="px-1.5 py-0.5 bg-zinc-200 dark:bg-zinc-700 rounded text-zinc-600 dark:text-zinc-400">
                    {alt}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

// 系统切换时间线
const SystemSwitchTimeline: React.FC<{ switches: SystemSwitch[] }> = ({ switches }) => {
  if (switches.length === 0) return null;

  return (
    <div className="mt-4 p-3 rounded-lg bg-zinc-100/30 dark:bg-zinc-800/30 border border-zinc-200 dark:border-zinc-700">
      <div className="flex items-center gap-2 mb-3">
        <GitBranch className="w-4 h-4 text-zinc-500" />
        <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300">系统切换记录</span>
      </div>
      <div className="space-y-2">
        {switches.map((sw, index) => {
          const fromInfo = cognitiveLayerLabels[sw.from];
          const toInfo = cognitiveLayerLabels[sw.to];
          return (
            <div key={sw.id} className="flex items-center gap-3 text-xs">
              <span className="text-zinc-400 w-6">#{index + 1}</span>
              <div className="flex items-center gap-2 flex-1">
                <span className={cn("px-2 py-0.5 rounded", fromInfo.label === 'System 1' ? 'bg-yellow-500/20 text-yellow-400' : fromInfo.label === 'System 2' ? 'bg-blue-500/20 text-blue-400' : 'bg-purple-500/20 text-purple-400')}>
                  {fromInfo.label}
                </span>
                <ArrowRightLeft className="w-3 h-3 text-zinc-400" />
                <span className={cn("px-2 py-0.5 rounded", toInfo.label === 'System 1' ? 'bg-yellow-500/20 text-yellow-400' : toInfo.label === 'System 2' ? 'bg-blue-500/20 text-blue-400' : 'bg-purple-500/20 text-purple-400')}>
                  {toInfo.label}
                </span>
              </div>
              <span className="text-zinc-500">{sw.reason}</span>
              <span className={cn("px-1.5 py-0.5 rounded", getConfidenceBgColor(sw.confidence), getConfidenceColor(sw.confidence))}>
                {(sw.confidence * 100).toFixed(0)}%
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
};

// 调用栈视图
const CallStackView: React.FC<{ frames: StackFrame[] }> = ({ frames }) => {
  const [expandedFrames, setExpandedFrames] = useState<Set<string>>(new Set());

  if (frames.length === 0) return null;

  const toggleFrame = (frameId: string) => {
    setExpandedFrames(prev => {
      const newSet = new Set(prev);
      if (newSet.has(frameId)) {
        newSet.delete(frameId);
      } else {
        newSet.add(frameId);
      }
      return newSet;
    });
  };

  return (
    <div className="mt-4 p-3 rounded-lg bg-zinc-100/30 dark:bg-zinc-800/30 border border-zinc-200 dark:border-zinc-700">
      <div className="flex items-center gap-2 mb-3">
        <Layers className="w-4 h-4 text-zinc-500" />
        <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300">调用栈</span>
        <span className="text-xs text-zinc-400">({frames.length} 层)</span>
      </div>
      <div className="space-y-2">
        {frames.map((frame, index) => {
          const isExpanded = expandedFrames.has(frame.id);
          const hasDetails = frame.args || frame.result;
          
          return (
            <div 
              key={frame.id} 
              className={cn(
                "text-xs rounded border",
                frame.status === 'running' ? 'bg-blue-500/10 border-blue-500/20' : 
                frame.status === 'error' ? 'bg-rose-500/10 border-rose-500/20' :
                'bg-zinc-200/50 dark:bg-zinc-700/50 border-zinc-200 dark:border-zinc-700'
              )}
            >
              {/* 头部信息 */}
              <div 
                className={cn(
                  "flex items-center gap-2 p-2 cursor-pointer hover:bg-zinc-100 dark:hover:bg-zinc-700/50 transition-colors",
                  isExpanded && "border-b border-zinc-200 dark:border-zinc-700"
                )}
                onClick={() => hasDetails && toggleFrame(frame.id)}
              >
                <span className="text-zinc-400 w-6 font-mono">#{frames.length - index}</span>
                <span className="font-mono text-zinc-700 dark:text-zinc-300 font-medium">{frame.function}</span>
                
                {/* 状态指示器 */}
                {frame.status === 'running' && (
                  <span className="ml-auto flex items-center gap-1 text-blue-400">
                    <span className="w-1.5 h-1.5 bg-blue-400 rounded-full animate-pulse" />
                    运行中
                  </span>
                )}
                {frame.status === 'error' && (
                  <span className="ml-auto text-rose-400">错误</span>
                )}
                {frame.status === 'completed' && frame.endTime && (
                  <span className="ml-auto text-zinc-500">
                    {frame.endTime - frame.startTime}ms
                  </span>
                )}
                
                {/* 展开指示器 */}
                {hasDetails && (
                  <span className="text-zinc-400">
                    {isExpanded ? '▼' : '▶'}
                  </span>
                )}
              </div>
              
              {/* 详细信息 */}
              {isExpanded && hasDetails && (
                <div className="p-2 space-y-2 bg-zinc-50 dark:bg-zinc-800/50">
                  {/* 参数 */}
                  {frame.args && (
                    <div>
                      <div className="text-zinc-500 mb-1 text-[10px] uppercase tracking-wider">参数</div>
                      <pre className="text-[10px] bg-zinc-100 dark:bg-zinc-900 p-2 rounded overflow-x-auto text-zinc-600 dark:text-zinc-400">
                        {JSON.stringify(frame.args, null, 2)}
                      </pre>
                    </div>
                  )}
                  
                  {/* 返回值 */}
                  {frame.result && (
                    <div>
                      <div className="text-zinc-500 mb-1 text-[10px] uppercase tracking-wider">返回值</div>
                      <pre className="text-[10px] bg-zinc-100 dark:bg-zinc-900 p-2 rounded overflow-x-auto text-zinc-600 dark:text-zinc-400">
                        {JSON.stringify(frame.result, null, 2)}
                      </pre>
                    </div>
                  )}
                  
                  {/* 时间信息 */}
                  <div className="flex items-center gap-4 text-[10px] text-zinc-500">
                    <span>开始: {new Date(frame.startTime).toLocaleTimeString('zh-CN')}</span>
                    {frame.endTime && (
                      <span>结束: {new Date(frame.endTime).toLocaleTimeString('zh-CN')}</span>
                    )}
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};

// 手动控制面板组件
const ManualControlPanel: React.FC<{
  enabled: boolean;
  onEnabledChange: (enabled: boolean) => void;
  selectedLayer: CognitiveLayer;
  onLayerChange: (layer: CognitiveLayer) => void;
  selectedParadigm: AgentParadigm;
  onParadigmChange: (paradigm: AgentParadigm) => void;
}> = ({
  enabled,
  onEnabledChange,
  selectedLayer,
  onLayerChange,
  selectedParadigm,
  onParadigmChange,
}) => {
  return (
    <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 border border-zinc-200 dark:border-zinc-700 mb-3">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <Settings className="w-4 h-4 text-zinc-500" />
          <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300">手动控制</span>
        </div>
        <button
          onClick={() => onEnabledChange(!enabled)}
          className={cn(
            "relative inline-flex h-5 w-9 items-center rounded-full transition-colors",
            enabled ? "bg-blue-500" : "bg-zinc-300 dark:bg-zinc-600"
          )}
        >
          <span
            className={cn(
              "inline-block h-3 w-3 transform rounded-full bg-white transition-transform",
              enabled ? "translate-x-5" : "translate-x-1"
            )}
          />
        </button>
      </div>
      
      {enabled && (
        <div className="space-y-3 animate-in fade-in slide-in-from-top-2 duration-200">
          {/* 思考系统选择 */}
          <div>
            <div className="text-xs text-zinc-500 mb-2">思考系统</div>
            <div className="flex gap-2">
              {(['system1', 'system2', 'system3'] as CognitiveLayer[]).map((layer) => {
                const info = cognitiveLayerLabels[layer];
                const Icon = info.icon;
                return (
                  <button
                    key={layer}
                    onClick={() => onLayerChange(layer)}
                    className={cn(
                      "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs transition-all",
                      selectedLayer === layer
                        ? layer === 'system1'
                          ? "bg-yellow-500/20 text-yellow-600 dark:text-yellow-400 border border-yellow-500/30"
                          : layer === 'system2'
                          ? "bg-blue-500/20 text-blue-600 dark:text-blue-400 border border-blue-500/30"
                          : "bg-purple-500/20 text-purple-600 dark:text-purple-400 border border-purple-500/30"
                        : "bg-zinc-200 dark:bg-zinc-700 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-300 dark:hover:bg-zinc-600"
                    )}
                  >
                    <Icon className="w-3.5 h-3.5" />
                    <span>{info.label}</span>
                  </button>
                );
              })}
            </div>
          </div>
          
          {/* Agent范式选择 */}
          <div>
            <div className="text-xs text-zinc-500 mb-2">Agent范式</div>
            <div className="flex flex-wrap gap-2">
              {(['single-turn', 'react', 'reflexion', 'plan-and-execute', 'swarm'] as AgentParadigm[]).map((paradigm) => {
                const info = paradigmLabels[paradigm];
                return (
                  <button
                    key={paradigm}
                    onClick={() => onParadigmChange(paradigm)}
                    className={cn(
                      "px-3 py-1.5 rounded-lg text-xs transition-all",
                      selectedParadigm === paradigm
                        ? "bg-indigo-500/20 text-indigo-600 dark:text-indigo-400 border border-indigo-500/30"
                        : "bg-zinc-200 dark:bg-zinc-700 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-300 dark:hover:bg-zinc-600"
                    )}
                  >
                    {info.label}
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

// 主组件
export const AgentThinkingVisualization: React.FC<AgentThinkingVisualizationProps> = ({
  process,
  isThinking,
  className,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);
  const [expandedSteps, setExpandedSteps] = useState<Set<string>>(new Set());
  const [activeTab, setActiveTab] = useState<'steps' | 'switches' | 'stack'>('steps');

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

  // 按类型分组统计
  const stepCounts = useMemo(() => {
    return process.steps.reduce((acc, step) => {
      acc[step.type] = (acc[step.type] || 0) + 1;
      return acc;
    }, {} as Record<string, number>);
  }, [process.steps]);

  // 计算总耗时
  const totalDuration = process.totalDuration || 
    (process.endTime ? process.endTime - process.startTime : Date.now() - process.startTime);

  // 当前系统信息
  const currentLayerInfo = cognitiveLayerLabels[process.currentLayer];
  const currentParadigmInfo = paradigmLabels[process.currentParadigm];

  if (process.steps.length === 0 && !isThinking) return null;

  return (
    <div className={cn(
      "rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white/50 dark:bg-zinc-900/50 overflow-hidden",
      className
    )}>
      {/* 头部 - 始终显示 */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center justify-between px-4 py-3 hover:bg-zinc-50 dark:hover:bg-zinc-800/50 transition-colors"
      >
        <div className="flex items-center gap-3">
          <div className="relative">
            <Brain className="w-5 h-5 text-indigo-400" />
            {isThinking && (
              <span className="absolute -top-0.5 -right-0.5 w-2 h-2 bg-indigo-400 rounded-full animate-ping" />
            )}
          </div>
          <div className="flex flex-col items-start">
            <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300">
              {isThinking ? 'Agent 思考中...' : '思考过程'}
            </span>
            <div className="flex items-center gap-2 mt-0.5">
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
            </div>
          </div>
        </div>
        
        <div className="flex items-center gap-3">
          {/* 当前系统/范式指示器 */}
          <div className="hidden sm:flex items-center gap-2">
            <span className={cn(
              "px-2 py-0.5 rounded text-[10px]",
              process.currentLayer === 'system1' ? 'bg-yellow-500/20 text-yellow-400' :
              process.currentLayer === 'system2' ? 'bg-blue-500/20 text-blue-400' :
              process.currentLayer === 'system3' ? 'bg-purple-500/20 text-purple-400' :
              'bg-zinc-500/20 text-zinc-400'
            )}>
              {currentLayerInfo.label}
            </span>
            <span className="px-2 py-0.5 rounded text-[10px] bg-zinc-100 dark:bg-zinc-800 text-zinc-500">
              {currentParadigmInfo.label}
            </span>
          </div>
          
          {/* 步骤类型统计 */}
          <div className="hidden md:flex items-center gap-1">
            {Object.entries(stepCounts).slice(0, 3).map(([type, count]) => {
              const Icon = stepIcons[type as DecisionStepType];
              return (
                <span key={type} className="flex items-center gap-0.5 px-1.5 py-0.5 bg-zinc-100 dark:bg-zinc-800 rounded text-[10px] text-zinc-500">
                  <Icon className="w-3 h-3" />
                  {count}
                </span>
              );
            })}
          </div>
          
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-zinc-400" />
          ) : (
            <ChevronDown className="w-4 h-4 text-zinc-400" />
          )}
        </div>
      </button>

      {/* 展开的内容 */}
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
              思考系统
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
          </div>

          {/* 步骤列表 */}
          {activeTab === 'steps' && (
            <div className="p-4 space-y-3 max-h-96 overflow-y-auto">
              {process.steps.length === 0 ? (
                <div className="flex items-center justify-center py-8 text-zinc-400">
                  <span className="text-sm">暂无思考记录</span>
                </div>
              ) : (
                process.steps.map((step, index) => {
                  const Icon = stepIcons[step.type];
                  const isDetailExpanded = expandedSteps.has(step.id);
                  const hasDetails = step.details && Object.keys(step.details).length > 0;
                  const stepColor = stepColors[step.type];

                  return (
                    <div
                      key={step.id}
                      className="flex gap-3 animate-in fade-in slide-in-from-left-4 duration-300"
                      style={{ animationDelay: `${index * 50}ms` }}
                    >
                      {/* 时间线 */}
                      <div className="flex flex-col items-center">
                        <div className={cn(
                          "w-8 h-8 rounded-lg flex items-center justify-center border",
                          stepColor
                        )}>
                          <Icon className="w-4 h-4" />
                        </div>
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
                              hour: '2-digit', 
                              minute: '2-digit', 
                              second: '2-digit' 
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
                          {step.metadata?.layer && (
                            <span className={cn(
                              "text-[10px] px-1.5 py-0.5 rounded",
                              step.metadata.layer === 'system1' ? 'bg-yellow-500/20 text-yellow-400' :
                              step.metadata.layer === 'system2' ? 'bg-blue-500/20 text-blue-400' :
                              'bg-purple-500/20 text-purple-400'
                            )}>
                              {cognitiveLayerLabels[step.metadata.layer].label}
                            </span>
                          )}
                          {hasDetails && (
                            <button
                              onClick={() => toggleStepDetail(step.id)}
                              className="text-[10px] text-blue-400 hover:text-blue-500 transition-colors"
                            >
                              {isDetailExpanded ? '收起' : '详情'}
                            </button>
                          )}
                        </div>
                        <p className="text-sm text-zinc-700 dark:text-zinc-300 leading-relaxed">
                          {step.content}
                        </p>
                        
                        {/* 展开详情 */}
                        {isDetailExpanded && hasDetails && (
                          <StepDetailPanel step={step} />
                        )}
                      </div>
                    </div>
                  );
                })
              )}
              
              {/* 思考中指示器 */}
              {isThinking && (
                <div className="flex gap-3 animate-pulse">
                  <div className="w-8 h-8 rounded-lg flex items-center justify-center border border-indigo-400/30 bg-indigo-400/10">
                    <div className="flex gap-0.5">
                      <span className="w-1.5 h-1.5 bg-indigo-400 rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
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

          {/* 思考系统视图 */}
          {activeTab === 'switches' && (
            <div className="p-4 max-h-96 overflow-y-auto">
              <SystemSwitchTimeline switches={process.systemSwitches} />
              
              {/* Agent范式 */}
              {process.paradigmSwitches.length > 0 && (
                <div className="mt-4 p-3 rounded-lg bg-zinc-100/30 dark:bg-zinc-800/30 border border-zinc-200 dark:border-zinc-700">
                  <div className="flex items-center gap-2 mb-3">
                    <Workflow className="w-4 h-4 text-zinc-500" />
                    <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300">Agent范式记录</span>
                  </div>
                  <div className="space-y-2">
                    {process.paradigmSwitches.map((sw, index) => (
                      <div key={sw.id} className="flex items-center gap-3 text-xs">
                        <span className="text-zinc-400 w-6">#{index + 1}</span>
                        <div className="flex items-center gap-2 flex-1">
                          <span className="px-2 py-0.5 bg-zinc-200 dark:bg-zinc-700 rounded text-zinc-600 dark:text-zinc-400">
                            {paradigmLabels[sw.from].label}
                          </span>
                          <ArrowRightLeft className="w-3 h-3 text-zinc-400" />
                          <span className="px-2 py-0.5 bg-purple-500/20 text-purple-400 rounded">
                            {paradigmLabels[sw.to].label}
                          </span>
                        </div>
                        <span className="text-zinc-500">{sw.reason}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}

          {/* 调用栈视图 */}
          {activeTab === 'stack' && (
            <div className="p-4 max-h-96 overflow-y-auto">
              <CallStackView frames={process.callStack} />
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default AgentThinkingVisualization;
