// Three-Layer Cognitive Architecture Visualizer
// P0-2: Real-time visualization of cognitive routing decisions
// Provides transparency into "why" the AI chose a particular thinking approach

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import {
  Brain, Zap, Search, Code, Lightbulb, Route, Settings, Bot, Layers,
  ChevronRight, Activity, Gauge, Clock, Network, Target, Eye, Pause, Play,
  SkipForward, RotateCcw, Info, AlertCircle, CheckCircle, XCircle
} from 'lucide-react';

// 认知层类型
export type CognitiveLayer = 'system1' | 'system2' | 'system3' | 'unknown';

export interface CognitiveRouteDecision {
  timestamp: number;
  input: string;
  complexity: number;  // 0-1
  intent: string;
  selectedLayer: CognitiveLayer;
  routingReason: string;
  confidence: number;   // 0-1
  alternativeLayers: Array<{
    layer: CognitiveLayer;
    score: number;
    reason: string;
  }>;
  processingTimeMs: number;
}

export interface CognitiveLayerStatus {
  layer: CognitiveLayer;
  name: string;
  description: string;
  isActive: boolean;
  currentLoad: number;  // 0-100
  avgLatencyMs: number;
  requestCount: number;
  successRate: number;  // 0-100
}

export interface CognitiveVisualizationProps {
  // 当前路由决策
  currentDecision?: CognitiveRouteDecision;
  
  // 历史决策
  decisionHistory?: CognitiveRouteDecision[];
  
  // 认知层状态
  layerStatuses?: CognitiveLayerStatus[];
  
  // 用户干预回调
  onOverrideLayer?: (layer: CognitiveLayer, reason: string) => void;
  onRequestDetails?: (decisionId: string) => void;
}

// 认知层配置
const LAYER_CONFIG: Record<CognitiveLayer, {
  name: string;
  icon: React.ReactNode;
  color: string;
  description: string;
  bestFor: string[];
  avgLatency: string;
}> = {
  system1: {
    name: 'System 1 (直觉)',
    icon: <Zap className="w-4 h-4" />,
    color: 'text-green-500',
    description: '快速、自动、无意识的直觉思考。使用 Trie 匹配和模糊算法。',
    bestFor: ['问候', '简单命令', '状态查询', '帮助请求'],
    avgLatency: '<10ms',
  },
  system2: {
    name: 'System 2 (分析)',
    icon: <Brain className="w-4 h-4" />,
    color: 'text-blue-500',
    description: '慢速、逻辑，分析思考。使用 ReAct 引擎和工具调用。',
    bestFor: ['复杂问题', '代码生成', '数据分析', '多步骤任务'],
    avgLatency: '2-10s',
  },
  system3: {
    name: 'System 3 (Swarm)',
    icon: <Network className="w-4 h-4" />,
    color: 'text-purple-500',
    description: '多智能体协作。多个 Agent 并行工作，聚合结果。',
    bestFor: ['深度研究', '复杂分析', '创意生成', '跨领域任务'],
    avgLatency: '10s+',
  },
  unknown: {
    name: '未知',
    icon: <AlertCircle className="w-4 h-4" />,
    color: 'text-gray-500',
    description: '未确定认知层',
    bestFor: [],
    avgLatency: '-',
  },
};

export const CognitiveLayerVisualizer: React.FC<CognitiveVisualizationProps> = ({
  currentDecision,
  decisionHistory = [],
  layerStatuses = [],
  onOverrideLayer,
  onRequestDetails,
}) => {
  const [isExpanded, setIsExpanded] = useState(true);
  const [showDetails, setShowDetails] = useState(false);
  const [selectedHistoryIndex, setSelectedHistoryIndex] = useState<number | null>(null);

  // 计算总体认知负载
  const totalLoad = useMemo(() => {
    if (layerStatuses.length === 0) return { system1: 0, system2: 0, system3: 0, overall: 0 };
    
    const loads = {
      system1: layerStatuses.find(s => s.layer === 'system1')?.currentLoad ?? 0,
      system2: layerStatuses.find(s => s.layer === 'system2')?.currentLoad ?? 0,
      system3: layerStatuses.find(s => s.layer === 'system3')?.currentLoad ?? 0,
    };
    
    return {
      ...loads,
      overall: Math.round((loads.system1 + loads.system2 + loads.system3) / 3),
    };
  }, [layerStatuses]);

  // 渲染层选择器
  const renderLayerSelector = (layer: CognitiveLayer, status?: CognitiveLayerStatus) => {
    const config = LAYER_CONFIG[layer];
    const isSelected = currentDecision?.selectedLayer === layer;
    const isActive = status?.isActive ?? false;

    return (
      <div
        key={layer}
        className={`
          relative p-4 rounded-lg border-2 transition-all cursor-pointer
          ${isSelected
            ? `${config.color.replace('text-', 'border-')} bg-opacity-10`
            : 'border-gray-200 dark:border-gray-700 hover:border-gray-300'}
          ${isActive ? 'ring-2 ring-yellow-400 ring-opacity-50' : ''}
        `}
        onClick={() => onOverrideLayer?.(layer, 'Manual override')}
      >
        {/* Layer Header */}
        <div className="flex items-center gap-2 mb-2">
          <div className={`${config.color}`}>
            {config.icon}
          </div>
          <span className={`font-medium ${config.color}`}>{config.name}</span>
          {isSelected && (
            <span className="ml-auto px-2 py-0.5 text-xs bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300 rounded">
              已选择
            </span>
          )}
          {isActive && !isSelected && (
            <span className="ml-auto px-2 py-0.5 text-xs bg-yellow-100 dark:bg-yellow-900 text-yellow-700 dark:text-yellow-300 rounded">
              活跃
            </span>
          )}
        </div>

        {/* Load Bar */}
        <div className="mb-2">
          <div className="flex justify-between text-xs text-gray-500 mb-1">
            <span>负载</span>
            <span>{status?.currentLoad ?? 0}%</span>
          </div>
          <div className="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
            <div
              className={`h-full ${config.color.replace('text-', 'bg-')} transition-all`}
              style={{ width: `${status?.currentLoad ?? 0}%` }}
            />
          </div>
        </div>

        {/* Stats */}
        <div className="grid grid-cols-2 gap-2 text-xs text-gray-600 dark:text-gray-400">
          <div>
            <span className="opacity-60">延迟:</span>
            <span className="ml-1 font-mono">{status?.avgLatencyMs.toFixed(1) ?? config.avgLatency}</span>
          </div>
          <div>
            <span className="opacity-60">成功率:</span>
            <span className="ml-1">{status?.successRate.toFixed(0) ?? 100}%</span>
          </div>
        </div>
      </div>
    );
  };

  // 渲染路由决策详情
  const renderDecisionDetails = () => {
    if (!currentDecision) return null;

    const selectedConfig = LAYER_CONFIG[currentDecision.selectedLayer];

    return (
      <div className="mt-4 p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
        <h4 className="font-medium mb-3 flex items-center gap-2">
          <Route className="w-4 h-4" />
          路由决策详情
        </h4>

        {/* Input Preview */}
        <div className="mb-3">
          <div className="text-xs text-gray-500 mb-1">用户输入:</div>
          <div className="p-2 bg-white dark:bg-gray-900 rounded text-sm truncate">
            {currentDecision.input}
          </div>
        </div>

        {/* Metrics */}
        <div className="grid grid-cols-3 gap-3 mb-3">
          <div className="text-center">
            <div className="text-xs text-gray-500">复杂度</div>
            <div className="text-lg font-bold text-blue-500">
              {(currentDecision.complexity * 100).toFixed(0)}%
            </div>
          </div>
          <div className="text-center">
            <div className="text-xs text-gray-500">意图</div>
            <div className="text-lg font-bold text-purple-500">
              {currentDecision.intent}
            </div>
          </div>
          <div className="text-center">
            <div className="text-xs text-gray-500">置信度</div>
            <div className="text-lg font-bold text-green-500">
              {(currentDecision.confidence * 100).toFixed(0)}%
            </div>
          </div>
        </div>

        {/* Selected Layer Reason */}
        <div className="p-3 bg-white dark:bg-gray-900 rounded-lg mb-3">
          <div className="flex items-center gap-2 mb-2">
            {selectedConfig.icon}
            <span className={`font-medium ${selectedConfig.color}`}>
              选择理由
            </span>
          </div>
          <p className="text-sm text-gray-600 dark:text-gray-400">
            {currentDecision.routingReason}
          </p>
          <div className="mt-2 text-xs text-gray-500">
            处理时间: {currentDecision.processingTimeMs.toFixed(2)}ms
          </div>
        </div>

        {/* Alternative Layers */}
        {currentDecision.alternativeLayers.length > 0 && (
          <div>
            <div className="text-xs text-gray-500 mb-2">备选方案:</div>
            <div className="space-y-2">
              {currentDecision.alternativeLayers.map((alt, i) => {
                const altConfig = LAYER_CONFIG[alt.layer];
                return (
                  <div
                    key={i}
                    className="flex items-center justify-between p-2 bg-white dark:bg-gray-900 rounded"
                  >
                    <div className="flex items-center gap-2">
                      <div className={`${altConfig.color}`}>{altConfig.icon}</div>
                      <span className="text-sm">{altConfig.name}</span>
                    </div>
                    <div className="text-right">
                      <div className="text-sm font-medium">{(alt.score * 100).toFixed(0)}%</div>
                      <div className="text-xs text-gray-500">{alt.reason}</div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* User Override Button */}
        <button
          className="mt-4 w-full py-2 px-4 bg-blue-500 hover:bg-blue-600 text-white rounded-lg
                     flex items-center justify-center gap-2 transition-colors"
          onClick={() => setShowDetails(!showDetails)}
        >
          <Eye className="w-4 h-4" />
          {showDetails ? '隐藏详情' : '查看完整决策过程'}
        </button>
      </div>
    );
  };

  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 overflow-hidden">
      {/* Header */}
      <div
        className="p-4 bg-gradient-to-r from-blue-500 to-purple-500 text-white cursor-pointer"
        onClick={() => setIsExpanded(!isExpanded)}
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Layers className="w-6 h-6" />
            <div>
              <h3 className="font-bold text-lg">三层认知架构</h3>
              <p className="text-sm text-white text-opacity-80">
                实时可视化 AI 思考路径
              </p>
            </div>
          </div>
          <div className="flex items-center gap-4">
            {/* Overall Load */}
            <div className="text-center">
              <div className="text-xs text-white text-opacity-80">总负载</div>
              <div className="text-2xl font-bold">{totalLoad.overall}%</div>
            </div>
            {/* Status Indicator */}
            <div className="flex items-center gap-2">
              {currentDecision ? (
                <>
                  <div className="w-3 h-3 bg-green-400 rounded-full animate-pulse" />
                  <span className="text-sm">处理中</span>
                </>
              ) : (
                <>
                  <div className="w-3 h-3 bg-gray-400 rounded-full" />
                  <span className="text-sm">空闲</span>
                </>
              )}
            </div>
            <ChevronRight className={`w-5 h-5 transition-transform ${isExpanded ? 'rotate-90' : ''}`} />
          </div>
        </div>
      </div>

      {/* Content */}
      {isExpanded && (
        <div className="p-4">
          {/* Cognitive Layer Status */}
          <div className="mb-4">
            <h4 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-3 flex items-center gap-2">
              <Activity className="w-4 h-4" />
              认知层状态
            </h4>
            <div className="grid grid-cols-3 gap-3">
              {(['system1', 'system2', 'system3'] as CognitiveLayer[]).map(layer => {
                const status = layerStatuses.find(s => s.layer === layer);
                return renderLayerSelector(layer, status);
              })}
            </div>
          </div>

          {/* Current Decision */}
          {currentDecision && (
            <div className="mb-4">
              <h4 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-3 flex items-center gap-2">
                <Target className="w-4 h-4" />
                当前决策
              </h4>
              {renderDecisionDetails()}
            </div>
          )}

          {/* Decision History */}
          {decisionHistory.length > 0 && (
            <div>
              <h4 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-3 flex items-center gap-2">
                <Clock className="w-4 h-4" />
                决策历史 ({decisionHistory.length})
              </h4>
              <div className="space-y-2 max-h-60 overflow-y-auto">
                {decisionHistory.slice(-10).reverse().map((decision, i) => {
                  const config = LAYER_CONFIG[decision.selectedLayer];
                  const isSelected = selectedHistoryIndex === i;
                  return (
                    <div
                      key={i}
                      className={`
                        p-3 rounded-lg border cursor-pointer transition-all
                        ${isSelected
                          ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
                          : 'border-gray-200 dark:border-gray-700 hover:border-gray-300'}
                      `}
                      onClick={() => setSelectedHistoryIndex(isSelected ? null : i)}
                    >
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <div className={`${config.color}`}>{config.icon}</div>
                          <span className="text-sm font-medium">{config.name}</span>
                        </div>
                        <div className="text-xs text-gray-500">
                          {decision.complexity.toFixed(2)} 复杂度
                        </div>
                      </div>
                      <p className="text-xs text-gray-500 mt-1 truncate">
                        {decision.input}
                      </p>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Legend */}
          <div className="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
            <div className="flex flex-wrap gap-4 text-xs text-gray-500">
              <div className="flex items-center gap-1">
                <div className="w-2 h-2 bg-green-400 rounded-full" />
                <span>活跃 (正在处理)</span>
              </div>
              <div className="flex items-center gap-1">
                <div className="w-2 h-2 bg-blue-500 rounded-full" />
                <span>已选择 (当前路由)</span>
              </div>
              <div className="flex items-center gap-1">
                <Info className="w-3 h-3" />
                <span>点击层可手动覆盖路由</span>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default CognitiveLayerVisualizer;
