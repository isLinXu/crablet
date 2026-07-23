import React from 'react';
import { Settings } from 'lucide-react';
import { cn, cognitiveLayerLabels, paradigmLabels } from './thinkingConstants';
import type { CognitiveLayer, AgentParadigm } from './thinkingTypes';

// 手动控制面板组件
export const ManualControlPanel: React.FC<{
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
