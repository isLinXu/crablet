import { useEffect } from 'react';
import { useAgentThinking } from '@/hooks/useAgentThinking';
import type { ExtendedMessage, TraceStep } from '@/store/chatStore';
import { inferCognitiveLayer, type CognitiveLayer } from '@/utils/cognitive';
import type { AgentParadigm } from '../components/chat/EnhancedThinkingVisualization';

interface ResolvedModel {
  providerId: string;
  model: string;
  vendor: string;
  reason: string;
}

interface UseChatThinkingParams {
  isThinking: boolean;
  messages: ExtendedMessage[];
  resolvedModel: ResolvedModel | null;
  currentLayer: CognitiveLayer;
  sessionId: string | null;
}

interface UseChatThinkingReturn {
  thinkingProcess: ReturnType<typeof useAgentThinking>['process'];
  isAgentThinking: boolean;
  isManualMode: boolean;
  manualLayer: CognitiveLayer;
  manualParadigm: AgentParadigm;
  toggleManualMode: (v: boolean) => void;
  setManualLayerSelected: (v: CognitiveLayer) => void;
  setManualParadigmSelected: (v: AgentParadigm) => void;
}

function inferLayerFromMessage(msg: string): CognitiveLayer {
  const lower = msg.toLowerCase();
  const greetings = ['你好', '您好', '嗨', 'hello', 'hi', 'hey', '早上好', '下午好', '晚上好', '在吗', '在么'];
  if (greetings.some(g => lower.trim() === g || lower.startsWith(g + ' '))) {
    return 'system1';
  }
  const personaPatterns = [
    '你是谁', '你是什么', '你叫什么', '介绍一下', '你是干嘛的', '你是做什么的',
    '你的身份', '你的角色', '你是ai吗', '你是人工智能吗', '你是机器人吗',
    'who are you', 'what are you', 'your name', 'introduce yourself', 'tell me about yourself'
  ];
  if (personaPatterns.some(p => lower.includes(p))) {
    return 'system1';
  }
  const chatPatterns = [
    '你好吗', '最近怎么样', '很高兴认识你', '谢谢', '多谢', '哈哈', '呵呵', '嘿嘿',
    'how are you', 'what\'s up', 'how\'s it going', 'nice to meet you', 'thank you', 'thanks'
  ];
  if (chatPatterns.some(p => lower.trim() === p || lower.startsWith(p))) {
    return 'system1';
  }
  const personalPatterns = [
    '你多大了', '你几岁了', '你喜欢什么', '你的爱好', '你喜欢', 'how old are you',
    'where are you from', 'what do you like', 'your favorite'
  ];
  if (personalPatterns.some(p => lower.includes(p))) {
    return 'system1';
  }
  if (lower.includes('help') || lower.includes('帮助') || lower.includes('怎么用') || lower.includes('如何使用')) {
    return 'system1';
  }
  return 'system2';
}

const systemPrompts: Record<string, string> = {
  system1: '快速直觉响应模式 - 适用于简单直接的问题',
  system2: '深度分析推理模式 - 适用于需要逻辑思考的问题',
  system3: '元认知反思模式 - 适用于复杂的多步骤任务',
};

export function useChatThinking({
  isThinking,
  messages,
  resolvedModel,
  currentLayer,
  sessionId,
}: UseChatThinkingParams): UseChatThinkingReturn {
  const {
    process: thinkingProcess,
    isThinking: isAgentThinking,
    isManualMode,
    manualLayer,
    manualParadigm,
    startThinking,
    endThinking,
    addRoutingStep,
    addSystemStep,
    addParadigmStep,
    addReasoningStep,
    switchLayer,
    switchParadigm,
    pushStack,
    popStack,
    toggleManualMode,
    setManualLayerSelected,
    setManualParadigmSelected,
  } = useAgentThinking({
    sessionId,
    model: resolvedModel?.model || 'unknown',
    vendor: resolvedModel?.vendor || 'unknown',
  });

  useEffect(() => {
    if (isThinking && messages.length > 0) {
      const lastMessage = messages[messages.length - 1];
      if (lastMessage?.role === 'assistant') {
        startThinking();

        if (resolvedModel) {
          addRoutingStep(
            resolvedModel.providerId,
            resolvedModel.model,
            resolvedModel.vendor,
            resolvedModel.reason,
            0.5
          );
        }

        const lastUserMsg = messages[messages.length - 2]?.content as string || '';
        const effectiveLayer = isManualMode
          ? manualLayer
          : (currentLayer !== 'unknown' ? currentLayer : inferLayerFromMessage(lastUserMsg));

        addSystemStep(
          effectiveLayer,
          systemPrompts[effectiveLayer] || '默认系统提示',
          isManualMode ? '手动选择' : (currentLayer !== 'unknown' ? '基于问题复杂度自动选择' : '使用默认系统')
        );

        switchLayer(
          effectiveLayer,
          isManualMode ? `手动切换至 ${effectiveLayer}` : (currentLayer !== 'unknown' ? `自动切换至 ${effectiveLayer}` : `默认使用 ${effectiveLayer}`),
          isManualMode ? 'manual-override' : 'complexity-analysis',
          0.85
        );

        const paradigm: AgentParadigm = isManualMode ? manualParadigm : (
          effectiveLayer === 'system1' ? 'single-turn' :
          effectiveLayer === 'system2' ? 'react' :
          effectiveLayer === 'system3' ? 'reflexion' : 'react'
        );
        addParadigmStep(
          paradigm,
          isManualMode ? `手动选择 ${paradigm} 范式` : `基于 ${effectiveLayer} 认知层选择对应范式`
        );
        switchParadigm(paradigm, isManualMode ? '手动选择范式' : `切换至 ${paradigm} 范式`, isManualMode ? 'manual-override' : 'layer-paradigm-mapping');

        const processMessageFrame = pushStack('processMessage', {
          sessionId,
          messageLength: lastMessage.content?.length || 0,
          layer: effectiveLayer,
          paradigm,
          isManualMode,
          manualLayer,
          manualParadigm
        });

        const inferenceFrame = pushStack('inference.generate', {
          model: resolvedModel?.model,
          provider: resolvedModel?.providerId,
          temperature: 0.7,
          maxTokens: 2048
        });

        if (lastMessage.traceSteps) {
          lastMessage.traceSteps.forEach((trace) => {
            addReasoningStep(
              trace.thought,
              trace.action,
              trace.observation
            );
          });
        }

        popStack(inferenceFrame, { tokens: lastMessage.content?.length || 0, status: 'success' });
        popStack(processMessageFrame, { completed: true, totalSteps: lastMessage.traceSteps?.length || 0 });
      }
    } else if (!isThinking && isAgentThinking) {
      endThinking();
    }
  }, [isThinking, messages, resolvedModel, currentLayer, isManualMode, manualLayer, manualParadigm, sessionId, startThinking, endThinking, addRoutingStep, addSystemStep, addParadigmStep, addReasoningStep, switchLayer, switchParadigm, isAgentThinking, pushStack, popStack]);

  return {
    thinkingProcess,
    isAgentThinking,
    isManualMode,
    manualLayer,
    manualParadigm,
    toggleManualMode,
    setManualLayerSelected,
    setManualParadigmSelected,
  };
}
