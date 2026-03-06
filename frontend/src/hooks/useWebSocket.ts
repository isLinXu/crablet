import { useEffect, useRef, useCallback, useMemo, useState } from 'react';
import { useChatStore } from '../store/chatStore';
import type { CitationItem } from '../store/chatStore';
import { getWsUrl } from '../utils/constants';
import { parseWsEvent, type WsNormalizedEvent } from '../types/ws';
import { chatService } from '@/services/chatService';
import { inferCognitiveLayer } from '@/utils/cognitive';
import { useModelStore } from '@/store/modelStore';

const MAX_RECONNECT_ATTEMPTS = 5;
const BASE_RECONNECT_DELAY = 1000;
const PING_INTERVAL = 30000; // 30s heartbeat

export const useWebSocket = () => {
  const ws = useRef<WebSocket | null>(null);
  const reconnectAttempts = useRef(0);
  const isConnecting = useRef(false);
  const pingIntervalRef = useRef<NodeJS.Timeout | null>(null);
  const [events, setEvents] = useState<WsNormalizedEvent[]>([]);
  const isConnected = useChatStore((state) => state.isConnected);
  const resolveForPrompt = useModelStore((state) => state.resolveForPrompt);
  
  const { 
    setConnected, 
    addMessage, 
    appendTrace, 
    updateLastMessage, 
    appendSwarmEvent,
    setThinking,
    setCurrentCognitiveLayer,
    sessionId,
    setSessionId
  } = useChatStore();

  const cleanup = useCallback(() => {
    if (pingIntervalRef.current) {
      clearInterval(pingIntervalRef.current);
      pingIntervalRef.current = null;
    }
    if (ws.current) {
      ws.current.close();
      ws.current = null;
    }
  }, []);

  const connect = useCallback(() => {
    if (isConnecting.current || ws.current?.readyState === WebSocket.OPEN) return;

    isConnecting.current = true;
    const socket = new WebSocket(getWsUrl());

    socket.onopen = () => {
      console.log('WebSocket Connected');
      setConnected(true);
      reconnectAttempts.current = 0;
      isConnecting.current = false;
      ws.current = socket;

      // Start Heartbeat
      if (pingIntervalRef.current) clearInterval(pingIntervalRef.current);
      pingIntervalRef.current = setInterval(() => {
        if (socket.readyState === WebSocket.OPEN) {
          socket.send(JSON.stringify({ type: 'ping' }));
        }
      }, PING_INTERVAL);
    };

    socket.onclose = () => {
      console.log('WebSocket Disconnected');
      setConnected(false);
      isConnecting.current = false;
      ws.current = null;
      if (pingIntervalRef.current) clearInterval(pingIntervalRef.current);

      if (reconnectAttempts.current < MAX_RECONNECT_ATTEMPTS) {
        const delay = Math.min(
          BASE_RECONNECT_DELAY * Math.pow(2, reconnectAttempts.current), 
          30000
        );
        console.log(`Reconnecting in ${delay}ms... (Attempt ${reconnectAttempts.current + 1})`);
        reconnectAttempts.current += 1;
        setTimeout(() => connect(), delay);
      } else {
        console.error('Max reconnect attempts reached');
        // Optionally show a toast or global error here
      }
    };

    socket.onerror = (error) => {
      console.error('WebSocket Error:', error);
      // onerror usually precedes onclose, so we let onclose handle reconnection
    };

    socket.onmessage = (event) => {
      const raw = typeof event.data === 'string' ? event.data : '';
      const parsed = parseWsEvent(raw);
      setEvents((prev) => [...prev.slice(-499), parsed]);
      if (parsed.kind === 'pong' || parsed.kind === 'unknown') return;
      if (parsed.kind === 'user_input') {
        addMessage({
          role: 'user',
          content: parsed.content,
          timestamp: new Date().toISOString()
        });
        addMessage({
          role: 'assistant',
          content: '',
          timestamp: new Date().toISOString(),
          traceSteps: [],
          swarmEvents: []
        });
      } else if (parsed.kind === 'thought') {
        appendTrace({
          thought: parsed.thought,
          action: '',
          input: '',
          observation: ''
        });
      } else if (parsed.kind === 'tool_start') {
        appendTrace({
          thought: '',
          action: parsed.tool,
          input: parsed.args,
          observation: ''
        });
      } else if (parsed.kind === 'tool_finish') {
        appendTrace({
          thought: '',
          action: '',
          input: '',
          observation: parsed.output
        });
      } else if (parsed.kind === 'swarm_activity') {
        appendSwarmEvent({
          taskId: parsed.taskId,
          from: parsed.from,
          to: parsed.to,
          type: parsed.messageType,
          content: parsed.content,
          timestamp: Date.now()
        });
      } else if (parsed.kind === 'graph_rag_mode_changed') {
        appendTrace({
          thought: `GraphRAG 实体抽取模式切换：${parsed.fromMode} → ${parsed.toMode}`,
          action: 'graph_rag_mode_changed',
          input: parsed.fromMode,
          observation: parsed.toMode
        });
      } else if (parsed.kind === 'cognitive_layer') {
        setCurrentCognitiveLayer(parsed.layer);
      } else if (parsed.kind === 'response') {
        updateLastMessage(parsed.content);
      } else if (parsed.kind === 'error') {
        console.error('Backend Error:', parsed.message);
      }
    };
    }, [setConnected, addMessage, appendTrace, updateLastMessage, appendSwarmEvent, setCurrentCognitiveLayer]);

  useEffect(() => {
    connect();
    return cleanup;
  }, [connect, cleanup]);

  const sendMessage = useCallback(async (content: string, citations?: CitationItem[]) => {
    const activeSessionId = sessionId ?? `session-${Date.now()}`;
    if (!sessionId) {
      setSessionId(activeSessionId);
    }

    addMessage({
      role: 'user',
      content,
      timestamp: new Date().toISOString()
    });
    setThinking(true);
    try {
      const questionType = /(image|图片|ocr|图像|audio|视频|video|多模态)/i.test(content)
        ? 'multimodal'
        : /(code|代码|debug|bug|refactor|typescript|rust|python|java)/i.test(content)
        ? 'coding'
        : /(analyze|分析|reason|推理|总结|对比)/i.test(content)
        ? 'analysis'
        : 'general';
      const p = localStorage.getItem('crablet-model-priority');
      const priority = p === 'speed' || p === 'quality' || p === 'balanced' ? p : 'balanced';
      const resolved = resolveForPrompt(activeSessionId, content, priority);
      const routePayload = {
        provider_id: resolved.providerId,
        vendor: resolved.vendor,
        model: resolved.model,
        version: resolved.version,
        reason: resolved.reason,
        priority: priority as 'speed' | 'quality' | 'balanced',
        question_type: questionType,
        api_base_url: resolved.apiBaseUrl,
        api_key: resolved.apiKey,
        model_type: resolved.modelType,
      };
      const imageModelByName = /image|qwen-image|doubao-image|sdxl|stable-diffusion|flux/i.test(resolved.model);
      if (resolved.modelType === 'image' || imageModelByName) {
        const fromCn = content.match(/([1-9]\d*)\s*[张幅个]\s*(图|图片|图像)/i);
        const fromEn = content.match(/\b([1-9]\d*)\s*(images|pics|pictures)\b/i);
        const parsedN = Number(fromCn?.[1] || fromEn?.[1] || '1');
        const imageCount = Math.max(1, Math.min(4, Number.isFinite(parsedN) ? parsedN : 1));
        const img = await chatService.generateImage(content, activeSessionId, imageCount, routePayload);
        if (img?.error) throw new Error(img.error);
        if (img?.session_id) setSessionId(img.session_id);
        const images = (img?.images || []).filter(Boolean);
        if (images.length === 0) throw new Error('图像生成返回为空');
        addMessage({
          role: 'assistant',
          content: images.map((url) => ({ type: 'image_url', image_url: { url } })),
          timestamp: new Date().toISOString(),
          cognitiveLayer: 'system2',
          citations: citations || []
        });
        setCurrentCognitiveLayer('system2');
        return;
      }
      const data = await chatService.sendMessage(content, activeSessionId, routePayload);
      
      if (data?.error) {
        throw new Error(data.error);
      }

      if (data?.session_id) {
        setSessionId(data.session_id);
      }
      let assistantContent = 'No response';
      let traceSteps: Array<{ thought: string; action: string; input: string; observation: string }> = [];
      let cognitiveLayer: 'system1' | 'system2' | 'system3' | 'unknown' = data?.cognitive_layer || 'unknown';
      if (typeof data?.response === 'string') {
        assistantContent = data.response;
        if (Array.isArray(data?.traces)) {
          traceSteps = data.traces.map((s: any) => ({
            thought: String(s?.thought ?? ''),
            action: String(s?.action ?? ''),
            input: String(s?.action_input ?? s?.input ?? ''),
            observation: String(s?.observation ?? '')
          }));
        }
      } else if (Array.isArray(data?.response) && typeof data.response[0] === 'string') {
        assistantContent = data.response[0];
        const maybeSteps = data.response[1];
        if (Array.isArray(maybeSteps)) {
          traceSteps = maybeSteps.map((s: any) => ({
            thought: String(s?.thought ?? ''),
            action: String(s?.action ?? ''),
            input: String(s?.action_input ?? s?.input ?? ''),
            observation: String(s?.observation ?? '')
          }));
          for (const step of traceSteps) {
            const layer = inferCognitiveLayer(step);
            if (layer !== 'unknown') {
              cognitiveLayer = layer;
              break;
            }
          }
        }
      } else if (data?.response != null) {
        assistantContent = JSON.stringify(data.response);
      }
      if (cognitiveLayer === 'unknown') {
        for (const step of traceSteps) {
          const layer = inferCognitiveLayer(step);
          if (layer !== 'unknown') {
            cognitiveLayer = layer;
            break;
          }
        }
      }
      setCurrentCognitiveLayer(cognitiveLayer);
      addMessage({
        role: 'assistant',
        content: assistantContent,
        timestamp: new Date().toISOString(),
        traceSteps,
        cognitiveLayer,
        citations: citations || []
      });
    } catch (error: any) {
      const errorMessage = error.message || '发送失败，请稍后重试。';
      addMessage({
        role: 'assistant',
        content: errorMessage,
        timestamp: new Date().toISOString()
      });
      console.error('Send message failed:', error);
    } finally {
      setThinking(false);
      if (!ws.current && !isConnecting.current) connect();
    }
  }, [addMessage, setThinking, sessionId, setSessionId, connect, resolveForPrompt]);

  const chatResponses = useMemo(
    () => events.filter((event) => event.kind === 'response'),
    [events]
  );
  const swarmEvents = useMemo(
    () => events.filter((event) => event.kind === 'swarm_activity'),
    [events]
  );
  const systemLogs = useMemo(
    () => events.filter((event) => event.kind === 'thought' || event.kind === 'tool_start' || event.kind === 'tool_finish' || event.kind === 'error' || event.kind === 'cognitive_layer'),
    [events]
  );

  return { sendMessage, isConnected, events, chatResponses, swarmEvents, systemLogs };
};
