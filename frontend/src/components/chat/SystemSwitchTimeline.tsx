import React from 'react';
import { GitBranch, ArrowRightLeft } from 'lucide-react';
import { cn, cognitiveLayerLabels, getConfidenceColor, getConfidenceBgColor } from './thinkingConstants';
import type { SystemSwitch } from './thinkingTypes';

// 系统切换时间线
export const SystemSwitchTimeline: React.FC<{ switches: SystemSwitch[] }> = ({ switches }) => {
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
