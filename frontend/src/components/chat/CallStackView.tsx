import React, { useState } from 'react';
import { Layers } from 'lucide-react';
import { cn } from './thinkingConstants';
import type { StackFrame } from './thinkingTypes';

// 调用栈视图
export const CallStackView: React.FC<{ frames: StackFrame[] }> = ({ frames }) => {
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
