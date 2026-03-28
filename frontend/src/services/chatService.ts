import { api } from '@/services/api';
import { isAxiosError } from 'axios';
import type { ChatSession, Message } from '@/types/domain';

interface SendChatResponse {
  response: unknown;
  traces?: Array<{
    step?: number;
    thought?: string;
    action?: string | null;
    action_input?: string | null;
    observation?: string | null;
  }>;
  cognitive_layer?: 'system1' | 'system2' | 'system3' | 'unknown';
  session_id?: string;
  error?: string;
}

interface SendImageResponse {
  images?: string[];
  session_id?: string;
  model?: string;
  error?: string;
}

interface RouteSelection {
  provider_id: string;
  vendor: string;
  model: string;
  version: string;
  reason: string;
  priority: 'speed' | 'quality' | 'balanced';
  question_type: string;
  api_base_url: string;
  api_key: string;
  model_type: 'chat' | 'image';
}

const shouldRetryWithLegacyPath = (error: unknown) =>
  isAxiosError(error) &&
  [401, 404, 405].includes(error.response?.status ?? 0);

export const chatService = {
  sendMessage: async (
    message: string,
    sessionId?: string,
    route?: RouteSelection
  ) => {
    const payload = {
      message,
      session_id: sessionId,
      route,
    };
    try {
      return await api.post<SendChatResponse>('/v1/chat', payload);
    } catch (error: unknown) {
      if (shouldRetryWithLegacyPath(error)) {
        return api.post<SendChatResponse>('/api/v1/chat', payload);
      }
      throw error;
    }
  },
  generateImage: async (
    prompt: string,
    sessionId?: string,
    n?: number,
    route?: RouteSelection
  ) => {
    const payload = {
      prompt,
      session_id: sessionId,
      n,
      route,
    };
    try {
      return await api.post<SendImageResponse>('/v1/images', payload);
    } catch (error: unknown) {
      if (shouldRetryWithLegacyPath(error)) {
        return api.post<SendImageResponse>('/api/v1/images', payload);
      }
      throw error;
    }
  },
  getSessions: () => api.get<ChatSession[]>('/v1/sessions'),
  getSessionHistory: (sessionId: string) => api.get<Message[]>(`/v1/sessions/${sessionId}/history`),
  deleteSession: (sessionId: string) => api.delete<void>(`/v1/sessions/${sessionId}`),
};
