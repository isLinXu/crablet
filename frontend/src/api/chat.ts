import type { Message, ChatSession, ApiResponse } from './types';
import { chatService } from '@/services/chatService';
import client from './client';

export const chatApi = {
  sendMessage: async (message: string, sessionId?: string): Promise<ApiResponse<{ response: unknown; session_id?: string; error?: string }>> => {
    const data = await chatService.sendMessage(message, sessionId);
    return { data, status: 'ok' };
  },
  getHistory: async (sessionId: string): Promise<ApiResponse<Message[]>> => {
    const data = await chatService.getSessionHistory(sessionId);
    return { data, status: 'ok' };
  },
  getSessions: async (): Promise<ApiResponse<ChatSession[]>> => {
    const data = await chatService.getSessions();
    return { data, status: 'ok' };
  },
  createSession: async (title?: string): Promise<ApiResponse<ChatSession>> => {
    const response = await client.post<ChatSession>('/sessions', { title });
    return { data: response.data, status: 'ok' };
  },
  deleteSession: async (sessionId: string): Promise<ApiResponse<void>> => {
    await chatService.deleteSession(sessionId);
    return { data: undefined, status: 'ok' };
  },
};
