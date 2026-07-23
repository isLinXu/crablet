import React from 'react';
import { ArrowRightLeft } from 'lucide-react';
import { cn, paradigmLabels, getConfidenceColor, getConfidenceBgColor } from './thinkingConstants';
import type { DecisionStep, AgentParadigm } from './thinkingTypes';

// 渲染步骤详情
export const StepDetailPanel: React.FC<{ step: DecisionStep }> = ({ step }) => {
  const { details, type } = step;
  if (!details) return null;

  return (
    <div className="mt-3 space-y-3 text-xs">
      {/* 路由选择详情 */}
      {type === 'routing' && details.provider && (
        <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-2">
          <div className="flex items-center gap-2">
            <span className="text-zinc-500 w-16">提供商:</span>
            <span className="font-medium text-zinc-700 dark:text-zinc-300">{String(details.vendor ?? '')}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-zinc-500 w-16">模型:</span>
            <span className="font-medium text-zinc-700 dark:text-zinc-300">{String(details.model ?? '')}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-zinc-500 w-16">原因:</span>
            <span className="text-zinc-600 dark:text-zinc-400">{String(details.reason ?? '')}</span>
          </div>
          {typeof details.complexityScore === 'number' && (
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
                {String(details.systemPrompt)}
              </div>
            </div>
          )}
          {details.triggerCondition && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500">触发条件:</span>
              <span className="text-zinc-600 dark:text-zinc-400">{String(details.triggerCondition)}</span>
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
                  {paradigmLabels[details.fromParadigm as AgentParadigm]?.label || String(details.fromParadigm)}
                </span>
                <ArrowRightLeft className="w-4 h-4 text-zinc-400" />
              </>
            )}
            {details.toParadigm && (
              <span className="px-2 py-1 bg-purple-500/20 text-purple-400 rounded">
                {paradigmLabels[details.toParadigm as AgentParadigm]?.label || String(details.toParadigm)}
              </span>
            )}
          </div>
          {details.paradigmReason && (
            <div className="mt-2 text-zinc-600 dark:text-zinc-400">
              原因: {String(details.paradigmReason)}
            </div>
          )}
        </div>
      )}

      {/* Agent 执行详情 */}
      {type === 'agent' && details.agentName && (
        <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-2">
          <div className="flex items-center gap-2">
            <span className="text-zinc-500 w-16">代理:</span>
            <span className="font-medium text-zinc-700 dark:text-zinc-300">{String(details.agentName)}</span>
          </div>
          {details.agentType && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500 w-16">类型:</span>
              <span className="text-zinc-600 dark:text-zinc-400">{String(details.agentType)}</span>
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
              <div className="text-zinc-700 dark:text-zinc-300">{String(details.thought)}</div>
            </div>
          )}
          {details.action && (
            <div>
              <div className="text-zinc-500 mb-1">行动:</div>
              <div className="text-zinc-700 dark:text-zinc-300">{String(details.action)}</div>
            </div>
          )}
          {details.observation && (
            <div>
              <div className="text-zinc-500 mb-1">观察:</div>
              <div className="text-zinc-600 dark:text-zinc-400">{String(details.observation)}</div>
            </div>
          )}
        </div>
      )}

      {/* 工具调用详情 */}
      {type === 'tool-call' && details.toolName && (
        <div className="p-3 rounded-lg bg-zinc-100/50 dark:bg-zinc-800/50 space-y-2">
          <div className="flex items-center gap-2">
            <span className="text-zinc-500 w-16">工具:</span>
            <span className="font-medium text-zinc-700 dark:text-zinc-300">{String(details.toolName)}</span>
          </div>
          {typeof details.toolDuration === 'number' && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500 w-16">耗时:</span>
              <span className="text-zinc-600 dark:text-zinc-400">{details.toolDuration}ms</span>
            </div>
          )}
          {details.toolInput != null && (
            <div>
              <div className="text-zinc-500 mb-1">输入:</div>
              <pre className="text-xs bg-zinc-200/50 dark:bg-zinc-700/50 p-2 rounded overflow-x-auto">
                {JSON.stringify(details.toolInput, null, 2)}
              </pre>
            </div>
          )}
          {details.toolOutput != null && (
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
              <span className="text-zinc-700 dark:text-zinc-300">{String(details.tokenCount)}</span>
            </div>
          )}
          {details.contextWindow !== undefined && (
            <div className="flex items-center gap-2">
              <span className="text-zinc-500 w-20">上下文窗口:</span>
              <span className="text-zinc-700 dark:text-zinc-300">{String(details.contextWindow)}</span>
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
                {String(details.previousState)}
              </span>
              <ArrowRightLeft className="w-4 h-4 text-zinc-400" />
              <span className="px-2 py-1 bg-violet-500/20 text-violet-400 rounded">
                {String(details.currentState)}
              </span>
            </div>
          )}
          {details.stateDiff != null && (
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
      {type === 'confidence' && typeof details.confidenceScore === 'number' && (
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
              评估依据: {String(details.confidenceReason)}
            </div>
          )}
          {Array.isArray(details.alternatives) && details.alternatives.length > 0 && (
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
