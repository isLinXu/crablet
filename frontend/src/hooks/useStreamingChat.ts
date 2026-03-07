import { useCallback } from 'react';
import { useChatStore } from '../store/chatStore';
import type { CitationItem } from '../store/chatStore';
import { LOCAL_STORAGE_KEYS, getApiBaseUrl } from '../utils/constants';
import { useModelStore } from '@/store/modelStore';

type StreamEvent = {
  type: string;
  content?: string | null;
  payload?: any;
  session_id?: string;
};

const buildHeaders = () => {
  const token = localStorage.getItem(LOCAL_STORAGE_KEYS.AUTH_TOKEN);
  const apiKey = localStorage.getItem(LOCAL_STORAGE_KEYS.API_KEY);
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

  const sendMessage = useCallback(async (content: string, citations?: CitationItem[]) => {
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
      const payload = {
        message: content,
        session_id: activeSessionId,
        route: {
          provider_id: resolved.providerId,
          vendor: resolved.vendor,
          model: resolved.model,
          version: resolved.version,
          reason: resolved.reason,
          priority,
          question_type: 'general',
          api_base_url: resolved.apiBaseUrl,
          api_key: resolved.apiKey,
          model_type: resolved.modelType,
        },
      };
      const baseUrl = getApiBaseUrl().replace(/\/+$/, '');
      const candidates = [
        `http://localhost:18789/api/v1/chat/stream`,
        `http://127.0.0.1:18789/api/v1/chat/stream`
      ]; // Force Gateway
      if (baseUrl !== 'http://127.0.0.1:18789/api' && baseUrl !== 'http://localhost:18789/api') {
        candidates.push(`${baseUrl}/v1/chat/stream`);
      }
      
      let response: Response | null = null;
      let lastError: any = null;
      const candidatesList = [...new Set(candidates)];
      
      for (const url of candidatesList) {
        try {
          // Add logging to debug
          console.log(`[Chat] Trying stream URL: ${url}`);
          const res = await fetch(url, {
            method: 'POST',
            headers: buildHeaders(),
            body: JSON.stringify(payload),
          });

          // If OK, use this response
          if (res.ok) {
            response = res;
            break;
          }
          
          // If not OK, but it's a "soft" error (404/405/401/5xx), try next candidate
          // Note: 401 might need re-auth, but we treat it as "try next" here
          if ([404, 405, 401, 502, 503].includes(res.status)) {
            console.warn(`[Chat] Request to ${url} failed with ${res.status}, trying next...`);
            lastError = new Error(`HTTP ${res.status}`);
            continue;
          }
          
          // Other errors (e.g. 400 Bad Request) -> stop trying
          response = res;
          break;
        } catch (e) {
          console.warn(`[Chat] Request to ${url} failed with network error`, e);
          lastError = e;
          // Continue to next candidate on network error (e.g. CORS failure)
        }
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
          if (event.type === 'delta') {
            const chunk = typeof event.content === 'string' ? event.content : '';
            full += chunk;
            updateLastMessage(full);
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
      if (full) updateLastMessage(full);
      setCurrentCognitiveLayer('system2');
    } catch (error: any) {
      updateLastMessage(error?.message || '流式发送失败，请稍后重试');
    } finally {
      setThinking(false);
    }
  }, [addMessage, appendTrace, resolveForPrompt, sessionId, setCurrentCognitiveLayer, setSessionId, setThinking, updateLastMessage]);

  return { sendMessage };
}
