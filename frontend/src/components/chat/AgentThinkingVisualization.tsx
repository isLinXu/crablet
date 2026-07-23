import React, { useState, useMemo } from 'react';
import {
  Brain,
  ChevronDown,
  ChevronUp,
  GitBranch,
  Layers,
  History,
  ArrowRightLeft,
  Workflow,
} from 'lucide-react';
import {
  cn,
  stepIcons,
  stepLabels,
  stepColors,
  cognitiveLayerLabels,
  paradigmLabels,
  getConfidenceColor,
  getConfidenceBgColor,
} from './thinkingConstants';
import { StepDetailPanel } from './StepDetailPanel';
import { SystemSwitchTimeline } from './SystemSwitchTimeline';
import { CallStackView } from './CallStackView';

import type { CognitiveLayer, AgentParadigm, DecisionStepType, DecisionStepDetails, DecisionStep, SystemSwitch, ParadigmSwitch, StackFrame, ThinkingProcess } from './thinkingTypes';
export type { CognitiveLayer, AgentParadigm, DecisionStepType, DecisionStepDetails, DecisionStep, SystemSwitch, ParadigmSwitch, StackFrame, ThinkingProcess } from './thinkingTypes';

// Re-export sub-components for convenience
export { StepDetailPanel } from './StepDetailPanel';
export { SystemSwitchTimeline } from './SystemSwitchTimeline';
export { CallStackView } from './CallStackView';
export { ManualControlPanel } from './ManualControlPanel';

interface AgentThinkingVisualizationProps {
  process: ThinkingProcess;
  isThinking: boolean;
  className?: string;
}

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
                          {(() => {
                            const layer = step.metadata?.layer;
                            if (typeof layer !== 'string') return null;
                            return (
                            <span className={cn(
                              "text-[10px] px-1.5 py-0.5 rounded",
                              layer === 'system1' ? 'bg-yellow-500/20 text-yellow-400' :
                              layer === 'system2' ? 'bg-blue-500/20 text-blue-400' :
                              'bg-purple-500/20 text-purple-400'
                            )}>
                              {cognitiveLayerLabels[layer as CognitiveLayer].label}
                            </span>
                            );
                          })()}
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
