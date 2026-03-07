import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { Message } from '@/types/domain';
import { inferCognitiveLayer, type CognitiveLayer } from '@/utils/cognitive';

export interface TraceStep {
  thought: string;
  action: string;
  input: string;
  observation: string;
}

export interface SwarmEvent {
  taskId: string;
  from: string;
  to: string;
  type: string;
  content: string;
  timestamp: number;
}

export interface CitationItem {
  source: string;
  score: number;
  snippet: string;
}

export interface ExtendedMessage extends Message {
  traceSteps?: TraceStep[];
  swarmEvents?: SwarmEvent[];
  cognitiveLayer?: CognitiveLayer;
  citations?: CitationItem[];
}

export interface ChatSessionItem {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
}

interface ChatState {
  messages: ExtendedMessage[];
  isConnected: boolean;
  isThinking: boolean;
  currentCognitiveLayer: CognitiveLayer;
  sessionId: string | null;
  sessions: ChatSessionItem[];
  sessionMessages: Record<string, ExtendedMessage[]>;
  
  addMessage: (msg: ExtendedMessage) => void;
  setConnected: (status: boolean) => void;
  setThinking: (status: boolean) => void;
  setCurrentCognitiveLayer: (layer: CognitiveLayer) => void;
  setSessionId: (id: string | null) => void;
  createSession: (title?: string) => string;
  renameSession: (id: string, title: string) => void;
  deleteSessions: (ids: string[]) => void;
  getMessagesBySession: (id: string) => ExtendedMessage[];
  bootstrapSessions: () => void;
  
  appendTrace: (step: TraceStep) => void;
  appendSwarmEvent: (event: SwarmEvent) => void;
  updateLastMessage: (content: string) => void;
  clearMessages: () => void;
  deleteMessage: (messageId: string) => void;
  editMessage: (messageId: string, newContent: string) => void;
}

const nowIso = () => new Date().toISOString();
const genSessionId = () => `session-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
const genMessageId = () => `msg-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

const inferSessionTitle = (msgs: ExtendedMessage[]) => {
  const userMsg = msgs.find((m) => m.role === 'user');
  if (!userMsg) return 'New Chat';
  const text =
    typeof userMsg.content === 'string'
      ? userMsg.content
      : userMsg.content
          .map((p: any) => (p?.type === 'text' ? p.text : ''))
          .join(' ')
          .trim();
  if (!text) return 'New Chat';
  return text.length > 24 ? `${text.slice(0, 24)}...` : text;
};

const materializeUnsavedSession = (
  sessionId: string | null,
  msgs: ExtendedMessage[],
  sessions: ChatSessionItem[],
  sessionMessages: Record<string, ExtendedMessage[]>
) => {
  if (sessionId || msgs.length === 0) {
    return { sessionId, sessions, sessionMessages };
  }
  const id = genSessionId();
  const timestamp = nowIso();
  const session: ChatSessionItem = {
    id,
    title: inferSessionTitle(msgs),
    created_at: timestamp,
    updated_at: timestamp
  };
  return {
    sessionId: id,
    sessions: [session, ...sessions],
    sessionMessages: { ...sessionMessages, [id]: msgs }
  };
};

const syncSessionSnapshot = (
  sessionId: string | null,
  msgs: ExtendedMessage[],
  sessions: ChatSessionItem[],
  sessionMessages: Record<string, ExtendedMessage[]>
) => {
  if (!sessionId) {
    return { sessions, sessionMessages };
  }

  const updatedMessages = { ...sessionMessages, [sessionId]: msgs };
  const timestamp = nowIso();
  const updatedSessions = sessions.map((s) =>
    s.id === sessionId ? { ...s, updated_at: timestamp } : s
  );

  return { sessions: updatedSessions, sessionMessages: updatedMessages };
};

export const useChatStore = create<ChatState>()(
  persist(
    (set, get) => ({
      messages: [],
      isConnected: false,
      isThinking: false,
      currentCognitiveLayer: 'unknown',
      sessionId: null,
      sessions: [],
      sessionMessages: {},
      
      addMessage: (msg) => set((state) => {
        const messageWithId = { ...msg, id: msg.id || genMessageId() };
        const msgs = [...state.messages, messageWithId];
        const synced = syncSessionSnapshot(state.sessionId, msgs, state.sessions, state.sessionMessages);
        return { messages: msgs, sessions: synced.sessions, sessionMessages: synced.sessionMessages };
      }),
      setConnected: (status) => set({ isConnected: status }),
      setThinking: (status) => set({ isThinking: status }),
      setCurrentCognitiveLayer: (layer) => set({ currentCognitiveLayer: layer }),
      setSessionId: (id) => set((state) => {
        const legacy = materializeUnsavedSession(state.sessionId, state.messages, state.sessions, state.sessionMessages);
        const currentSync = syncSessionSnapshot(legacy.sessionId, state.messages, legacy.sessions, legacy.sessionMessages);
        if (!id) {
          return {
            sessionId: null,
            sessions: currentSync.sessions,
            sessionMessages: currentSync.sessionMessages,
            messages: []
          };
        }
        const exists = currentSync.sessions.some((s) => s.id === id);
        const timestamp = nowIso();
        const sessions = exists
          ? currentSync.sessions
          : [{ id, title: 'New Chat', created_at: timestamp, updated_at: timestamp }, ...currentSync.sessions];
        const nextMessages = currentSync.sessionMessages[id] || [];
        return {
          sessionId: id,
          sessions,
          sessionMessages: currentSync.sessionMessages,
          messages: nextMessages
        };
      }),

      createSession: (title = 'New Chat') => {
        const state = get();
        const legacy = materializeUnsavedSession(state.sessionId, state.messages, state.sessions, state.sessionMessages);
        const currentSync = syncSessionSnapshot(legacy.sessionId, state.messages, legacy.sessions, legacy.sessionMessages);
        const id = genSessionId();
        const timestamp = nowIso();
        const newSession: ChatSessionItem = { id, title, created_at: timestamp, updated_at: timestamp };
        set({
          sessionId: id,
          messages: [],
          sessions: [newSession, ...currentSync.sessions],
          sessionMessages: { ...currentSync.sessionMessages, [id]: [] }
        });
        return id;
      },

      renameSession: (id, title) => set((state) => ({
        sessions: state.sessions.map((s) => (s.id === id ? { ...s, title, updated_at: nowIso() } : s))
      })),

      deleteSessions: (ids) => set((state) => {
        if (ids.length === 0) return {};
        const idSet = new Set(ids);
        const sessions = state.sessions.filter((s) => !idSet.has(s.id));
        const sessionMessages = { ...state.sessionMessages };
        ids.forEach((id) => {
          delete sessionMessages[id];
        });

        if (state.sessionId && idSet.has(state.sessionId)) {
          const fallback = sessions[0];
          return {
            sessions,
            sessionMessages,
            sessionId: fallback ? fallback.id : null,
            messages: fallback ? (sessionMessages[fallback.id] || []) : []
          };
        }

        return { sessions, sessionMessages };
      }),

      getMessagesBySession: (id) => {
        const state = get();
        return state.sessionMessages[id] || [];
      },

      bootstrapSessions: () => set((state) => {
        let sessionId = state.sessionId;
        let sessions = [...state.sessions];
        let sessionMessages = { ...state.sessionMessages };
        let messages = [...state.messages];
        let changed = false;

        const synced = syncSessionSnapshot(sessionId, messages, sessions, sessionMessages);
        sessions = synced.sessions;
        sessionMessages = synced.sessionMessages;

        if (!sessionId && messages.length > 0) {
          const legacy = materializeUnsavedSession(sessionId, messages, sessions, sessionMessages);
          sessionId = legacy.sessionId;
          sessions = legacy.sessions;
          sessionMessages = legacy.sessionMessages;
          changed = true;
        }

        if (sessionId && !sessions.some((s) => s.id === sessionId)) {
          const timestamp = nowIso();
          sessions = [
            { id: sessionId, title: inferSessionTitle(sessionMessages[sessionId] || messages), created_at: timestamp, updated_at: timestamp },
            ...sessions
          ];
          changed = true;
        }

        if (!sessionId && sessions.length > 0) {
          sessionId = sessions[0].id;
          messages = sessionMessages[sessionId] || [];
          changed = true;
        }

        if (sessions.length === 0) {
          const id = genSessionId();
          const timestamp = nowIso();
          sessions = [{ id, title: 'New Chat', created_at: timestamp, updated_at: timestamp }];
          sessionMessages[id] = [];
          sessionId = id;
          messages = [];
          changed = true;
        }

        if (!changed) return {};
        return { sessionId, sessions, sessionMessages, messages };
      }),
      
      appendTrace: (step) => set((state) => {
          const msgs = [...state.messages];
          if (msgs.length > 0 && msgs[msgs.length - 1].role === 'assistant') {
              const lastMsg = { ...msgs[msgs.length - 1] };
              const steps = lastMsg.traceSteps || [];
              lastMsg.traceSteps = [...steps, step];
              const inferred = inferCognitiveLayer(step);
              if (inferred !== 'unknown') {
                lastMsg.cognitiveLayer = inferred;
              }
              msgs[msgs.length - 1] = lastMsg;
              const synced = syncSessionSnapshot(state.sessionId, msgs, state.sessions, state.sessionMessages);
              return { messages: msgs, sessions: synced.sessions, sessionMessages: synced.sessionMessages };
          }
          return {};
      }),
      
      appendSwarmEvent: (event) => set((state) => {
          const msgs = [...state.messages];
          if (msgs.length > 0 && msgs[msgs.length - 1].role === 'assistant') {
              const lastMsg = { ...msgs[msgs.length - 1] };
              const events = lastMsg.swarmEvents || [];
              lastMsg.swarmEvents = [...events, event];
              msgs[msgs.length - 1] = lastMsg;
              const synced = syncSessionSnapshot(state.sessionId, msgs, state.sessions, state.sessionMessages);
              return { messages: msgs, sessions: synced.sessions, sessionMessages: synced.sessionMessages };
          }
          return {};
      }),
      
      updateLastMessage: (content) => set((state) => {
        const msgs = [...state.messages];
        if (msgs.length > 0) {
          const lastMsg = { ...msgs[msgs.length - 1] };
          // If content is string, update it. If it was object (parts), we might overwrite it or need more complex logic.
          // For now assume streaming updates string content.
          if (typeof lastMsg.content === 'string') {
             lastMsg.content = content;
          } else {
             // If it was parts, convert to string? Or append?
             // Simplification: just set as string for streaming updates
             lastMsg.content = content;
          }
          msgs[msgs.length - 1] = lastMsg;
          const synced = syncSessionSnapshot(state.sessionId, msgs, state.sessions, state.sessionMessages);
          return { messages: msgs, sessions: synced.sessions, sessionMessages: synced.sessionMessages };
        }
        return {};
      }),
      
      deleteMessage: (messageId) => set((state) => {
        const msgs = state.messages.filter((m) => m.id !== messageId);
        const synced = syncSessionSnapshot(state.sessionId, msgs, state.sessions, state.sessionMessages);
        return { messages: msgs, sessions: synced.sessions, sessionMessages: synced.sessionMessages };
      }),

      editMessage: (messageId, newContent) => set((state) => {
        const msgs = state.messages.map((m) => 
            m.id === messageId ? { ...m, content: newContent } : m
        );
        const synced = syncSessionSnapshot(state.sessionId, msgs, state.sessions, state.sessionMessages);
        return { messages: msgs, sessions: synced.sessions, sessionMessages: synced.sessionMessages };
      }),
      
      clearMessages: () => set((state) => {
        const synced = syncSessionSnapshot(state.sessionId, [], state.sessions, state.sessionMessages);
        return { messages: [], sessions: synced.sessions, sessionMessages: synced.sessionMessages };
      }),
    }),
    {
      name: 'chat-storage',
      migrate: (persistedState: any) => {
        if (!persistedState) return persistedState;
        const messages: ExtendedMessage[] = Array.isArray(persistedState.messages) ? persistedState.messages : [];
        const sessionId: string | null = persistedState.sessionId ?? null;
        const sessions: ChatSessionItem[] = Array.isArray(persistedState.sessions) ? persistedState.sessions : [];
        const sessionMessages: Record<string, ExtendedMessage[]> =
          persistedState.sessionMessages && typeof persistedState.sessionMessages === 'object'
            ? persistedState.sessionMessages
            : {};
        const legacy = materializeUnsavedSession(sessionId, messages, sessions, sessionMessages);
        return {
          ...persistedState,
          sessionId: legacy.sessionId,
          sessions: legacy.sessions,
          sessionMessages: legacy.sessionMessages
        };
      },
      partialize: (state) => ({
        messages: state.messages,
        sessionId: state.sessionId,
        sessions: state.sessions,
        sessionMessages: state.sessionMessages
      }),
      version: 3,
    }
  )
);
