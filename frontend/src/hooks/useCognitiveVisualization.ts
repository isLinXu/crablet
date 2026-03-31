// useCognitiveVisualization Hook
// P0-2: Hook for managing cognitive layer visualization state

import { useState, useCallback, useEffect, useRef } from 'react';
import type { 
  CognitiveLayer, 
  CognitiveRouteDecision, 
  CognitiveLayerStatus 
} from '../components/cognitive/CognitiveLayerVisualizer';

interface UseCognitiveVisualizationOptions {
  // 自动刷新间隔 (ms)
  refreshInterval?: number;
  // 是否启用实时更新
  realTimeEnabled?: boolean;
  // 最大历史记录数
  maxHistorySize?: number;
}

interface UseCognitiveVisualizationReturn {
  // 当前决策
  currentDecision: CognitiveRouteDecision | null;
  
  // 决策历史
  decisionHistory: CognitiveRouteDecision[];
  
  // 认知层状态
  layerStatuses: CognitiveLayerStatus[];
  
  // 是否正在处理
  isProcessing: boolean;
  
  // 手动覆盖认知层
  overrideLayer: (layer: CognitiveLayer, reason: string) => void;
  
  // 请求详情
  requestDetails: (decisionId: string) => void;
  
  // 清除历史
  clearHistory: () => void;
  
  // 刷新状态
  refresh: () => Promise<void>;
}

// 模拟数据生成器（实际使用时替换为真实 API 调用）
const generateMockDecision = (): CognitiveRouteDecision => {
  const layers: CognitiveLayer[] = ['system1', 'system2', 'system3'];
  const intents = ['Greeting', 'Help', 'Status', 'Analysis', 'Research', 'MultiStep'];
  const reasons = [
    '简单命令，直接匹配到 Trie',
    '复杂度低于阈值，选择快速路径',
    '多步骤任务，启用 ReAct 引擎',
    '深度研究需求，启动 Swarm 多智能体',
  ];
  
  const selectedLayer = layers[Math.floor(Math.random() * layers.length)];
  
  return {
    timestamp: Date.now(),
    input: '用户输入内容...',
    complexity: Math.random(),
    intent: intents[Math.floor(Math.random() * intents.length)],
    selectedLayer,
    routingReason: reasons[Math.floor(Math.random() * reasons.length)],
    confidence: 0.7 + Math.random() * 0.3,
    alternativeLayers: layers
      .filter(l => l !== selectedLayer)
      .map(layer => ({
        layer,
        score: Math.random(),
        reason: `备选方案: ${layer}`,
      })),
    processingTimeMs: Math.random() * 100,
  };
};

const generateMockLayerStatuses = (): CognitiveLayerStatus[] => [
  {
    layer: 'system1',
    name: 'System 1 (直觉)',
    description: '快速Trie匹配',
    isActive: Math.random() > 0.5,
    currentLoad: Math.floor(Math.random() * 30),
    avgLatencyMs: 0.5 + Math.random() * 2,
    requestCount: Math.floor(Math.random() * 1000),
    successRate: 95 + Math.random() * 5,
  },
  {
    layer: 'system2',
    name: 'System 2 (分析)',
    description: 'ReAct引擎',
    isActive: Math.random() > 0.5,
    currentLoad: Math.floor(Math.random() * 70),
    avgLatencyMs: 500 + Math.random() * 2000,
    requestCount: Math.floor(Math.random() * 500),
    successRate: 90 + Math.random() * 10,
  },
  {
    layer: 'system3',
    name: 'System 3 (Swarm)',
    description: '多智能体协作',
    isActive: Math.random() > 0.7,
    currentLoad: Math.floor(Math.random() * 50),
    avgLatencyMs: 5000 + Math.random() * 10000,
    requestCount: Math.floor(Math.random() * 100),
    successRate: 85 + Math.random() * 15,
  },
];

export function useCognitiveVisualization(
  options: UseCognitiveVisualizationOptions = {}
): UseCognitiveVisualizationReturn {
  const {
    refreshInterval = 5000,
    realTimeEnabled = true,
    maxHistorySize = 50,
  } = options;

  // State
  const [currentDecision, setCurrentDecision] = useState<CognitiveRouteDecision | null>(null);
  const [decisionHistory, setDecisionHistory] = useState<CognitiveRouteDecision[]>([]);
  const [layerStatuses, setLayerStatuses] = useState<CognitiveLayerStatus[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);

  // Refs
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // 手动覆盖认知层
  const overrideLayer = useCallback((layer: CognitiveLayer, reason: string) => {
    const overrideDecision: CognitiveRouteDecision = {
      timestamp: Date.now(),
      input: `手动覆盖: ${reason}`,
      complexity: 0,
      intent: 'ManualOverride',
      selectedLayer: layer,
      routingReason: `用户手动选择: ${LAYER_CONFIG[layer].name}`,
      confidence: 1.0,
      alternativeLayers: [],
      processingTimeMs: 0,
    };
    
    setCurrentDecision(overrideDecision);
    setDecisionHistory(prev => {
      const newHistory = [overrideDecision, ...prev];
      return newHistory.slice(0, maxHistorySize);
    });
  }, [maxHistorySize]);

  // 请求详情
  const requestDetails = useCallback((decisionId: string) => {
    // TODO: 调用后端 API 获取详细决策信息
  }, []);

  // 清除历史
  const clearHistory = useCallback(() => {
    setDecisionHistory([]);
  }, []);

  // 刷新状态
  const refresh = useCallback(async () => {
    setIsProcessing(true);
    try {
      // TODO: 替换为真实 API 调用
      // const response = await fetch('/api/v1/cognitive/status');
      // const data = await response.json();
      
      // 模拟数据
      const newDecision = generateMockDecision();
      const newStatuses = generateMockLayerStatuses();
      
      setCurrentDecision(newDecision);
      setDecisionHistory(prev => {
        const updated = [newDecision, ...prev];
        return updated.slice(0, maxHistorySize);
      });
      setLayerStatuses(newStatuses);
    } catch (error) {
      console.error('[CognitiveVisualization] Refresh failed:', error);
    } finally {
      setIsProcessing(false);
    }
  }, [maxHistorySize]);

  // 自动刷新
  useEffect(() => {
    if (!realTimeEnabled) return;

    // 初始刷新
    refresh();

    // 设置定时刷新
    intervalRef.current = setInterval(refresh, refreshInterval);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, [realTimeEnabled, refreshInterval, refresh]);

  return {
    currentDecision,
    decisionHistory,
    layerStatuses,
    isProcessing,
    overrideLayer,
    requestDetails,
    clearHistory,
    refresh,
  };
}

// Layer 配置映射（与 Visualizer 组件共享）
const LAYER_CONFIG: Record<CognitiveLayer, { name: string }> = {
  system1: { name: 'System 1 (直觉)' },
  system2: { name: 'System 2 (分析)' },
  system3: { name: 'System 3 (Swarm)' },
  unknown: { name: '未知' },
};

export default useCognitiveVisualization;
