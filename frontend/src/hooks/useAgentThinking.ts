import { useState, useCallback, useRef } from 'react';
import type {
  ThinkingProcess,
  DecisionStep,
  SystemSwitch,
  ParadigmSwitch,
  StackFrame,
  CognitiveLayer,
  AgentParadigm,
} from '@/components/chat/AgentThinkingVisualization';

interface UseAgentThinkingOptions {
  sessionId: string | null;
  model: string;
  vendor: string;
  // 手动模式配置
  manualMode?: boolean;
  defaultLayer?: CognitiveLayer;
  defaultParadigm?: AgentParadigm;
}

export function useAgentThinking(options: UseAgentThinkingOptions) {
  const { manualMode = false, defaultLayer = 'unknown', defaultParadigm = 'unknown' } = options;
  
  // 手动模式状态
  const [isManualMode, setIsManualMode] = useState(manualMode);
  const [manualLayer, setManualLayer] = useState<CognitiveLayer>(defaultLayer);
  const [manualParadigm, setManualParadigm] = useState<AgentParadigm>(defaultParadigm);
  
  const [process, setProcess] = useState<ThinkingProcess>(() => ({
    steps: [],
    systemSwitches: [],
    paradigmSwitches: [],
    callStack: [],
    currentLayer: defaultLayer,
    currentParadigm: defaultParadigm,
    startTime: Date.now(),
    confidence: 0,
  }));
  const [isThinking, setIsThinking] = useState(false);
  
  // 使用 ref 来跟踪当前状态，避免闭包问题
  const processRef = useRef(process);
  const updateProcess = useCallback((updater: (prev: ThinkingProcess) => ThinkingProcess) => {
    setProcess(prev => {
      const next = updater(prev);
      processRef.current = next;
      return next;
    });
  }, []);

  // 开始思考
  const startThinking = useCallback(() => {
    setIsThinking(true);
    updateProcess(prev => ({
      ...prev,
      startTime: Date.now(),
      steps: [],
      systemSwitches: [],
      paradigmSwitches: [],
      callStack: [],
      currentLayer: 'unknown',
      currentParadigm: 'unknown',
      confidence: 0,
    }));
  }, [updateProcess]);

  // 结束思考
  const endThinking = useCallback(() => {
    setIsThinking(false);
    updateProcess(prev => ({
      ...prev,
      endTime: Date.now(),
      totalDuration: Date.now() - prev.startTime,
    }));
  }, [updateProcess]);

  // 添加决策步骤
  const addStep = useCallback((step: Omit<DecisionStep, 'id' | 'timestamp'>) => {
    const newStep: DecisionStep = {
      ...step,
      id: `step-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      timestamp: Date.now(),
    };
    
    updateProcess(prev => ({
      ...prev,
      steps: [...prev.steps, newStep],
    }));
    
    return newStep.id;
  }, [updateProcess]);

  // 更新步骤
  const updateStep = useCallback((stepId: string, updates: Partial<DecisionStep>) => {
    updateProcess(prev => ({
      ...prev,
      steps: prev.steps.map(s => s.id === stepId ? { ...s, ...updates } : s),
    }));
  }, [updateProcess]);

  // 完成步骤（设置 duration）
  const completeStep = useCallback((stepId: string) => {
    updateProcess(prev => {
      const step = prev.steps.find(s => s.id === stepId);
      if (!step) return prev;
      
      return {
        ...prev,
        steps: prev.steps.map(s => 
          s.id === stepId 
            ? { ...s, duration: Date.now() - s.timestamp }
            : s
        ),
      };
    });
  }, [updateProcess]);

  // 切换认知层
  const switchLayer = useCallback((to: CognitiveLayer, reason: string, trigger: string, confidence: number) => {
    updateProcess(prev => {
      if (prev.currentLayer === to) return prev;
      
      const switchRecord: SystemSwitch = {
        id: `switch-${Date.now()}`,
        from: prev.currentLayer,
        to,
        reason,
        trigger,
        timestamp: Date.now(),
        confidence,
      };
      
      return {
        ...prev,
        currentLayer: to,
        systemSwitches: [...prev.systemSwitches, switchRecord],
      };
    });
  }, [updateProcess]);

  // 切换范式
  const switchParadigm = useCallback((to: AgentParadigm, reason: string, trigger: string) => {
    updateProcess(prev => {
      if (prev.currentParadigm === to) return prev;
      
      const switchRecord: ParadigmSwitch = {
        id: `paradigm-${Date.now()}`,
        from: prev.currentParadigm,
        to,
        reason,
        trigger,
        timestamp: Date.now(),
      };
      
      return {
        ...prev,
        currentParadigm: to,
        paradigmSwitches: [...prev.paradigmSwitches, switchRecord],
      };
    });
  }, [updateProcess]);

  // 压入调用栈
  const pushStack = useCallback((functionName: string, args: Record<string, unknown>) => {
    const frame: StackFrame = {
      id: `frame-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      function: functionName,
      args,
      startTime: Date.now(),
      status: 'running',
    };
    
    updateProcess(prev => ({
      ...prev,
      callStack: [frame, ...prev.callStack],
    }));
    
    return frame.id;
  }, [updateProcess]);

  // 弹出调用栈
  const popStack = useCallback((frameId: string, result?: Record<string, unknown>) => {
    updateProcess(prev => ({
      ...prev,
      callStack: prev.callStack.map(f => 
        f.id === frameId 
          ? { ...f, endTime: Date.now(), result, status: 'completed' as const }
          : f
      ),
    }));
  }, [updateProcess]);

  // 设置置信度
  const setConfidence = useCallback((confidence: number) => {
    updateProcess(prev => ({
      ...prev,
      confidence,
    }));
  }, [updateProcess]);

  // 快捷方法：添加路由选择步骤
  const addRoutingStep = useCallback((provider: string, model: string, vendor: string, reason: string, complexityScore?: number) => {
    return addStep({
      type: 'routing',
      title: '路由选择',
      content: `选择模型提供商: ${vendor} / ${model}`,
      details: {
        provider,
        model,
        vendor,
        reason,
        complexityScore,
      },
    });
  }, [addStep]);

  // 快捷方法：添加系统选择步骤
  const addSystemStep = useCallback((layer: CognitiveLayer, systemPrompt: string, triggerCondition: string) => {
    const layerNames: Record<CognitiveLayer, string> = {
      system1: 'System 1 - 快速直觉',
      system2: 'System 2 - 深度分析',
      system3: 'System 3 - 元认知反思',
      unknown: '未知系统',
    };
    
    return addStep({
      type: 'system',
      title: '系统选择',
      content: `选择 ${layerNames[layer]}`,
      details: {
        systemPrompt,
        triggerCondition,
      },
      metadata: { layer },
    });
  }, [addStep]);

  // 快捷方法：添加范式选择步骤
  const addParadigmStep = useCallback((paradigm: AgentParadigm, reason: string) => {
    const paradigmNames: Record<AgentParadigm, string> = {
      'single-turn': 'Single-Turn 单轮对话',
      'react': 'ReAct 推理-行动循环',
      'reflexion': 'Reflexion 自我反思',
      'plan-and-execute': 'Plan & Execute 规划-执行',
      'swarm': 'Swarm 多代理协作',
      'unknown': '未知范式',
    };
    
    return addStep({
      type: 'paradigm',
      title: '范式选择',
      content: `采用 ${paradigmNames[paradigm]}`,
      details: {
        paradigmReason: reason,
      },
      metadata: { paradigm },
    });
  }, [addStep]);

  // 快捷方法：添加推理步骤
  const addReasoningStep = useCallback((thought: string, action?: string, observation?: string) => {
    return addStep({
      type: 'reasoning',
      title: '推理思考',
      content: thought,
      details: {
        thought,
        action,
        observation,
      },
    });
  }, [addStep]);

  // 快捷方法：添加工具调用步骤
  const addToolCallStep = useCallback((toolName: string, input: unknown) => {
    return addStep({
      type: 'tool-call',
      title: '工具调用',
      content: `调用 ${toolName}`,
      details: {
        toolName,
        toolInput: input,
      },
    });
  }, [addStep]);

  // 快捷方法：完成工具调用
  const completeToolCall = useCallback((stepId: string, output: unknown, duration: number) => {
    updateStep(stepId, {
      details: {
        ...processRef.current.steps.find(s => s.id === stepId)?.details,
        toolOutput: output,
        toolDuration: duration,
      },
    });
    completeStep(stepId);
  }, [updateStep, completeStep]);

  // 快捷方法：添加置信度评估步骤
  const addConfidenceStep = useCallback((score: number, reason: string, alternatives?: string[]) => {
    setConfidence(score);
    return addStep({
      type: 'confidence',
      title: '置信度评估',
      content: `置信度评估: ${(score * 100).toFixed(1)}%`,
      confidence: score,
      details: {
        confidenceScore: score,
        confidenceReason: reason,
        alternatives,
      },
    });
  }, [addStep, setConfidence]);

  // 切换手动模式
  const toggleManualMode = useCallback((enabled: boolean) => {
    setIsManualMode(enabled);
    if (enabled) {
      // 启用手动模式时，应用手动选择的层和范式
      updateProcess(prev => ({
        ...prev,
        currentLayer: manualLayer,
        currentParadigm: manualParadigm,
      }));
    }
  }, [manualLayer, manualParadigm, updateProcess]);

  // 设置手动选择的层
  const setManualLayerSelected = useCallback((layer: CognitiveLayer) => {
    setManualLayer(layer);
    if (isManualMode) {
      // 直接更新 process 中的 currentLayer
      updateProcess(prev => {
        if (prev.currentLayer === layer) return prev;
        
        const switchRecord: SystemSwitch = {
          id: `switch-${Date.now()}`,
          from: prev.currentLayer,
          to: layer,
          reason: '手动选择',
          trigger: 'manual-override',
          timestamp: Date.now(),
          confidence: 1.0,
        };
        
        return {
          ...prev,
          currentLayer: layer,
          systemSwitches: [...prev.systemSwitches, switchRecord],
        };
      });
    }
  }, [isManualMode, updateProcess]);

  // 设置手动选择的范式
  const setManualParadigmSelected = useCallback((paradigm: AgentParadigm) => {
    setManualParadigm(paradigm);
    if (isManualMode) {
      // 直接更新 process 中的 currentParadigm
      updateProcess(prev => {
        if (prev.currentParadigm === paradigm) return prev;
        
        const switchRecord: ParadigmSwitch = {
          id: `paradigm-${Date.now()}`,
          from: prev.currentParadigm,
          to: paradigm,
          reason: '手动选择',
          trigger: 'manual-override',
          timestamp: Date.now(),
        };
        
        return {
          ...prev,
          currentParadigm: paradigm,
          paradigmSwitches: [...prev.paradigmSwitches, switchRecord],
        };
      });
    }
  }, [isManualMode, updateProcess]);

  // 重置
  const reset = useCallback(() => {
    setProcess({
      steps: [],
      systemSwitches: [],
      paradigmSwitches: [],
      callStack: [],
      currentLayer: isManualMode ? manualLayer : 'unknown',
      currentParadigm: isManualMode ? manualParadigm : 'unknown',
      startTime: Date.now(),
      confidence: 0,
    });
    processRef.current = {
      steps: [],
      systemSwitches: [],
      paradigmSwitches: [],
      callStack: [],
      currentLayer: isManualMode ? manualLayer : 'unknown',
      currentParadigm: isManualMode ? manualParadigm : 'unknown',
      startTime: Date.now(),
      confidence: 0,
    };
    setIsThinking(false);
  }, [isManualMode, manualLayer, manualParadigm]);

  return {
    process,
    isThinking,
    isManualMode,
    manualLayer,
    manualParadigm,
    startThinking,
    endThinking,
    addStep,
    updateStep,
    completeStep,
    switchLayer,
    switchParadigm,
    pushStack,
    popStack,
    setConfidence,
    addRoutingStep,
    addSystemStep,
    addParadigmStep,
    addReasoningStep,
    addToolCallStep,
    completeToolCall,
    addConfidenceStep,
    toggleManualMode,
    setManualLayerSelected,
    setManualParadigmSelected,
    reset,
  };
}
