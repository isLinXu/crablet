import { useCallback } from 'react';
import { useChatStore } from '../store/chatStore';
import type { CitationItem } from '../store/chatStore';
import { LOCAL_STORAGE_KEYS, resolveApiUrl } from '../utils/constants';
import { getSecureItem } from '../utils/secureStorage';
import { useModelStore } from '@/store/modelStore';

type StreamEvent = {
  type: string;
  content?: string | null;
  status?: string;
  payload?: {
    status?: string;
    total_latency_ms?: number;
    model?: string;
    cognitive_layer?: string;
    step?: {
      thought?: unknown;
      action?: unknown;
      action_input?: unknown;
      observation?: unknown;
    };
    layer?: unknown;
  };
  session_id?: string;
};

// 节流函数：限制函数执行频率
function throttle<TArgs extends unknown[]>(func: (...args: TArgs) => void, limit: number) {
  let inThrottle = false;
  let lastArgs: TArgs | null = null;
  return (...args: TArgs) => {
    if (!inThrottle) {
      func(...args);
      inThrottle = true;
      setTimeout(() => {
        inThrottle = false;
        if (lastArgs) {
          func(...lastArgs);
          lastArgs = null;
        }
      }, limit);
    } else {
      lastArgs = args;
    }
  };
}

const getErrorMessage = (error: unknown) =>
  error instanceof Error ? error.message : '流式发送失败，请稍后重试';

const buildHeaders = () => {
  const token = getSecureItem(LOCAL_STORAGE_KEYS.AUTH_TOKEN);
  const apiKey = getSecureItem(LOCAL_STORAGE_KEYS.API_KEY);
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    Accept: 'text/event-stream',
  };
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  } else if (apiKey) {
    headers.Authorization = `Bearer ${apiKey}`;
  }
  return headers;
};

const parseSseBlocks = (buffer: string) => {
  const events: string[] = [];
  let rest = buffer;
  while (true) {
    const idx = rest.indexOf('\n\n');
    if (idx < 0) break;
    const block = rest.slice(0, idx);
    rest = rest.slice(idx + 2);
    const dataLines = block
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line.startsWith('data:'))
      .map((line) => line.slice(5).trim());
    if (dataLines.length > 0) {
      events.push(dataLines.join(''));
    }
  }
  return { events, rest };
};

let activeChatController: AbortController | null = null;

export function useStreamingChat() {
  const {
    addMessage,
    appendTrace,
    updateLastMessage,
    setThinking,
    setCurrentCognitiveLayer,
    sessionId,
    setSessionId,
  } = useChatStore();
  const resolveForPrompt = useModelStore((state) => state.resolveForPrompt);
  const providers = useModelStore((state) => state.providers);

  const isImagePrompt = (prompt: string) =>
    /(draw|绘图|画图|生成图片|生成图像|海报|插画|text-to-image|image generation)/i.test(prompt);

  const sendMessage = useCallback(async (content: string, citations?: CitationItem[]) => {
    activeChatController?.abort();
    const controller = new AbortController();
    activeChatController = controller;
    const activeSessionId = sessionId ?? `session-${Date.now()}`;
    if (!sessionId) setSessionId(activeSessionId);
    addMessage({
      role: 'user',
      content,
      timestamp: new Date().toISOString(),
    });
    addMessage({
      role: 'assistant',
      content: '',
      timestamp: new Date().toISOString(),
      traceSteps: [],
      citations: citations || [],
    });
    setThinking(true);
    try {
      const p = localStorage.getItem(LOCAL_STORAGE_KEYS.MODEL_PRIORITY);
      const priority = p === 'speed' || p === 'quality' || p === 'balanced' ? p : 'balanced';
      const resolved = resolveForPrompt(activeSessionId, content, priority);
      const chatFallback = [...providers]
        .filter((p) => p.enabled && (p.modelType === 'chat' || !p.modelType))
        .sort((a, b) => a.priority - b.priority)[0];
      const forceChat = resolved.modelType === 'image' && !isImagePrompt(content) && !!chatFallback;
      const effective = forceChat ? {
        providerId: chatFallback!.id,
        vendor: chatFallback!.vendor,
        model: chatFallback!.model,
        modelType: 'chat' as const,
        apiBaseUrl: chatFallback!.apiBaseUrl,
        apiKey: chatFallback!.apiKey,
        version: chatFallback!.version,
        reason: 'forced-chat-for-text',
      } : resolved;
      const payload = {
        message: content,
        session_id: activeSessionId,
        route: {
          provider_id: effective.providerId,
          vendor: effective.vendor,
          model: effective.model,
          version: effective.version,
          reason: effective.reason,
          priority,
          question_type: 'general',
          api_base_url: effective.apiBaseUrl,
          api_key: effective.apiKey,
          model_type: effective.modelType,
        },
      };
      const streamUrl = resolveApiUrl('/v1/chat/stream');
      if (import.meta.env.DEV) {
        console.debug(`[Chat] Using stream URL: ${streamUrl}`);
      }
      let response: Response | null = null;
      let lastError: Error | null = null;
      try {
        response = await fetch(streamUrl, {
          method: 'POST',
          headers: buildHeaders(),
          body: JSON.stringify(payload),
          signal: controller.signal,
        });
      } catch (e) {
        console.warn(`[Chat] Request to ${streamUrl} failed with network error`, e);
        lastError = e instanceof Error ? e : new Error('Network Error');
      }

      if (!response || !response.ok || !response.body) {
        const status = response?.status;
        const msg = status ? `HTTP ${status}` : (lastError?.message || 'Network Error');
        throw new Error(`流式请求失败: ${msg}`);
      }
      const reader = response.body.getReader();
      const decoder = new TextDecoder('utf-8');
      let buffer = '';
      let full = '';
      let receivedDone = false;
      
      // 创建节流的更新函数，每 50ms 最多更新一次
      const throttledUpdate = throttle((content: string) => {
        updateLastMessage(content);
      }, 50);
      
      while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });
        const parsed = parseSseBlocks(buffer);
        buffer = parsed.rest;
        for (const raw of parsed.events) {
          let event: StreamEvent | null = null;
          try {
            event = JSON.parse(raw);
          } catch {
            continue;
          }
          if (!event) continue;
          if (event.session_id) setSessionId(event.session_id);
          if (event.type === 'status') {
            updateLastMessage(typeof event.payload?.status === 'string' ? `【${event.payload.status}】` : '模型处理中');
          } else if (event.type === 'metrics') {
            if (import.meta.env.DEV) console.debug('[Chat Metrics]', event.payload);
          } else if (event.type === 'delta') {
            const chunk = typeof event.content === 'string' ? event.content : '';
            full += chunk;
            // 使用节流函数更新，减少渲染频率
            throttledUpdate(full);
          } else if (event.type === 'trace') {
            const step = event.payload?.step;
            if (step) {
              appendTrace({
                thought: String(step.thought ?? ''),
                action: String(step.action ?? ''),
                input: String(step.action_input ?? ''),
                observation: String(step.observation ?? ''),
              });
            }
          } else if (event.type === 'cognitive_layer') {
            const layer = event.payload?.layer;
            if (layer === 'system1' || layer === 'system2' || layer === 'system3' || layer === 'unknown') {
              setCurrentCognitiveLayer(layer);
            }
          } else if (event.type === 'error') {
            throw new Error(event.content || '流式处理出错');
          } else if (event.type === 'done') {
            receivedDone = true;
          }
        }
      }
      if (buffer.trim()) {
        const parsed = parseSseBlocks(`${buffer}\n\n`);
        for (const raw of parsed.events) {
          try {
            const event = JSON.parse(raw) as StreamEvent;
            if (event.type === 'delta') {
              const chunk = typeof event.content === 'string' ? event.content : '';
              full += chunk;
            }
          } catch {
            continue;
          }
        }
      }
      // 确保最终内容被更新
      if (full) {
        updateLastMessage(full);
      } else if (receivedDone) {
        updateLastMessage('模型未返回文本结果，请切换为聊天模型后重试。');
      }
      // 不再强制设置为 system2，而是保持从流中接收到的认知层
      // 如果从未收到认知层事件，则基于输入内容推断
      // 注意：实际的认知层应该在流处理过程中已经被设置
    } catch (error: unknown) {
      if (controller.signal.aborted) {
        updateLastMessage('已取消本次模型请求');
      } else {
        updateLastMessage(getErrorMessage(error));
      }
    } finally {
      if (activeChatController === controller) activeChatController = null;
      setThinking(false);
    }
  }, [addMessage, appendTrace, providers, resolveForPrompt, sessionId, setCurrentCognitiveLayer, setSessionId, setThinking, updateLastMessage]);

  const cancelMessage = useCallback(() => {
    activeChatController?.abort();
  }, []);

  return { sendMessage, cancelMessage };
}
