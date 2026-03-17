import React, { useState } from 'react';
import { ChevronDown, ChevronUp, Brain, Sparkles, Lightbulb, Search, Code, Route, Settings, Bot, Layers, MessageSquare } from 'lucide-react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export interface ThinkingStep {
  id: string;
  type: 'reasoning' | 'search' | 'code' | 'insight' | 'routing' | 'system' | 'agent' | 'paradigm';
  content: string;
  timestamp: number;
  duration?: number;
  details?: {
    provider?: string;
    model?: string;
    vendor?: string;
    reason?: string;
    systemPrompt?: string;
    agentName?: string;
    agentType?: string;
    fromParadigm?: string;
    toParadigm?: string;
    params?: Record<string, any>;
  };
}

interface ThinkingProcessProps {
  steps: ThinkingStep[];
  isThinking: boolean;
  className?: string;
}

const stepIcons = {
  reasoning: Brain,
  search: Search,
  code: Code,
  insight: Lightbulb,
  routing: Route,
  system: Settings,
  agent: Bot,
  paradigm: Layers,
};

const stepLabels = {
  reasoning: '推理中',
  search: '检索知识',
  code: '代码分析',
  insight: '洞察',
  routing: '路由选择',
  system: '系统提示',
  agent: '代理执行',
  paradigm: '范式切换',
};

const stepColors = {
  reasoning: 'text-blue-400 bg-blue-400/10 border-blue-400/20',
  search: 'text-amber-400 bg-amber-400/10 border-amber-400/20',
  code: 'text-emerald-400 bg-emerald-400/10 border-emerald-400/20',
  insight: 'text-purple-400 bg-purple-400/10 border-purple-400/20',
  routing: 'text-cyan-400 bg-cyan-400/10 border-cyan-400/20',
  system: 'text-zinc-400 bg-zinc-400/10 border-zinc-400/20',
  agent: 'text-rose-400 bg-rose-400/10 border-rose-400/20',
  paradigm: 'text-indigo-400 bg-indigo-400/10 border-indigo-400/20',
};

// 渲染步骤详情
const StepDetails: React.FC<{ step: ThinkingStep }> = ({ step }) => {
  const { details } = step;
  if (!details) return null;

  return (
    <div className="mt-2 space-y-1.5 text-xs">
      {/* 路由选择详情 */}
      {step.type === 'routing' && details.provider && (
        <div className="p-2 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-1">
          <div className="flex items-center gap-2">
            <span className="text-zinc-500">提供商:</span>
            <span className="font-medium text-zinc-700 dark:text-zinc-300">{details.vendor}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-zinc-500">模型:</span>
            <span className="font-medium text-zinc-700 dark:text-zinc-300">{details.model}</span>
          </div>
          {details.reason && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500">原因:</span>
              <span className="text-zinc-600 dark:text-zinc-400">{details.reason}</span>
            </div>
          )}
        </div>
      )}

      {/* System 提示详情 */}
      {step.type === 'system' && details.systemPrompt && (
        <div className="p-2 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50">
          <div className="text-zinc-500 mb-1">System Prompt:</div>
          <div className="font-mono text-zinc-600 dark:text-zinc-400 line-clamp-3">
            {details.systemPrompt}
          </div>
        </div>
      )}

      {/* Agent 执行详情 */}
      {step.type === 'agent' && details.agentName && (
        <div className="p-2 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-1">
          <div className="flex items-center gap-2">
            <span className="text-zinc-500">代理:</span>
            <span className="font-medium text-zinc-700 dark:text-zinc-300">{details.agentName}</span>
          </div>
          {details.agentType && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500">类型:</span>
              <span className="text-zinc-600 dark:text-zinc-400">{details.agentType}</span>
            </div>
          )}
          {details.params && Object.keys(details.params).length > 0 && (
            <div className="mt-1">
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

      {/* 范式切换详情 */}
      {step.type === 'paradigm' && (
        <div className="p-2 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50">
          <div className="flex items-center gap-2">
            {details.fromParadigm && (
              <>
                <span className="px-2 py-0.5 bg-zinc-200 dark:bg-zinc-700 rounded text-zinc-600 dark:text-zinc-400">
                  {details.fromParadigm}
                </span>
                <span className="text-zinc-400">→</span>
              </>
            )}
            {details.toParadigm && (
              <span className="px-2 py-0.5 bg-indigo-500/20 text-indigo-400 rounded">
                {details.toParadigm}
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

export const ThinkingProcess: React.FC<ThinkingProcessProps> = ({
  steps,
  isThinking,
  className,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);
  const [expandedSteps, setExpandedSteps] = useState<Set<string>>(new Set());

  if (steps.length === 0 && !isThinking) return null;

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
  const stepCounts = steps.reduce((acc, step) => {
    acc[step.type] = (acc[step.type] || 0) + 1;
    return acc;
  }, {} as Record<string, number>);

  return (
    <div className={cn("rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white/50 dark:bg-zinc-900/50 overflow-hidden", className)}>
      {/* 头部 - 始终显示 */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center justify-between px-4 py-3 hover:bg-zinc-50 dark:hover:bg-zinc-800/50 transition-colors"
      >
        <div className="flex items-center gap-3">
          <div className="relative">
            <Sparkles className="w-4 h-4 text-amber-400" />
            {isThinking && (
              <span className="absolute -top-0.5 -right-0.5 w-2 h-2 bg-amber-400 rounded-full animate-ping" />
            )}
          </div>
          <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300">
            {isThinking ? '思考中...' : '思考过程'}
          </span>
          <span className="text-xs text-zinc-400">
            {steps.length} 个步骤
          </span>
          {/* 步骤类型标签 */}
          <div className="hidden sm:flex items-center gap-1">
            {Object.entries(stepCounts).slice(0, 3).map(([type, count]) => {
              const Icon = stepIcons[type as keyof typeof stepIcons];
              return (
                <span key={type} className="flex items-center gap-0.5 px-1.5 py-0.5 bg-zinc-100 dark:bg-zinc-800 rounded text-[10px] text-zinc-500">
                  <Icon className="w-3 h-3" />
                  {count}
                </span>
              );
            })}
          </div>
        </div>
        <div className="flex items-center gap-2">
          {isThinking && (
            <span className="flex items-center gap-1.5 text-xs text-zinc-400">
              <span className="w-1.5 h-1.5 bg-amber-400 rounded-full animate-pulse" />
              进行中
            </span>
          )}
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-zinc-400" />
          ) : (
            <ChevronDown className="w-4 h-4 text-zinc-400" />
          )}
        </div>
      </button>

      {/* 展开的步骤列表 */}
      {isExpanded && (
        <div className="border-t border-zinc-200 dark:border-zinc-800 animate-in slide-in-from-top-2 duration-200">
          <div className="p-4 space-y-3 max-h-80 overflow-y-auto">
            {steps.length === 0 ? (
              <div className="flex items-center justify-center py-8 text-zinc-400">
                <span className="text-sm">暂无思考记录</span>
              </div>
            ) : (
              steps.map((step, index) => {
                const Icon = stepIcons[step.type];
                const isDetailExpanded = expandedSteps.has(step.id);
                const hasDetails = step.details && Object.keys(step.details).length > 0;

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
                        stepColors[step.type]
                      )}>
                        <Icon className="w-4 h-4" />
                      </div>
                      {index < steps.length - 1 && (
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
                        <StepDetails step={step} />
                      )}
                    </div>
                  </div>
                );
              })
            )}
            
            {/* 思考中指示器 */}
            {isThinking && (
              <div className="flex gap-3 animate-pulse">
                <div className="w-8 h-8 rounded-lg flex items-center justify-center border border-amber-400/30 bg-amber-400/10">
                  <div className="flex gap-0.5">
                    <span className="w-1.5 h-1.5 bg-amber-400 rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
                    <span className="w-1.5 h-1.5 bg-amber-400 rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
                    <span className="w-1.5 h-1.5 bg-amber-400 rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
                  </div>
                </div>
                <div className="flex-1">
                  <span className="text-xs text-zinc-400">继续思考中...</span>
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

export default ThinkingProcess;
