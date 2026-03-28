import { describe, it, expect, beforeEach } from 'vitest';
import { useChatStore } from '../chatStore';

describe('useChatStore', () => {
  beforeEach(() => {
    // Reset store
    useChatStore.setState({
      messages: [],
      isConnected: false,
      isThinking: false,
      sessionId: null,
      sessions: [],
      sessionMessages: {},
    });
  });

  it('should start with empty state', () => {
    const state = useChatStore.getState();
    expect(state.messages).toEqual([]);
    expect(state.isConnected).toBe(false);
  });

  it('should add messages', () => {
    const { addMessage } = useChatStore.getState();
    const msg = { role: 'user' as const, content: 'Hello', timestamp: '2023-01-01' };
    
    addMessage(msg);
    
    const state = useChatStore.getState();
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0]).toMatchObject(msg);
    expect(state.messages[0].id).toBeTruthy();
  });

  it('should update connection status', () => {
    const { setConnected } = useChatStore.getState();
    setConnected(true);
    expect(useChatStore.getState().isConnected).toBe(true);
  });

  it('should append trace steps to last assistant message', () => {
    const { addMessage, appendTrace } = useChatStore.getState();
    
    // Must have an assistant message first
    addMessage({ role: 'assistant', content: '', timestamp: 'now' });
    
    const trace = { thought: 'Thinking...', action: '', input: '', observation: '' };
    appendTrace(trace);
    
    const state = useChatStore.getState();
    expect(state.messages[0].traceSteps).toHaveLength(1);
    expect(state.messages[0].traceSteps?.[0]).toEqual(trace);
  });

  it('should append swarm events to last assistant message', () => {
    const { addMessage, appendSwarmEvent } = useChatStore.getState();
    
    addMessage({ role: 'assistant', content: '', timestamp: 'now' });
    
    const event = { 
        taskId: '1', 
        from: 'A', 
        to: 'B', 
        type: 'delegate', 
        content: 'task', 
        timestamp: 123 
    };
    appendSwarmEvent(event);
    
    const state = useChatStore.getState();
    expect(state.messages[0].swarmEvents).toHaveLength(1);
    expect(state.messages[0].swarmEvents?.[0]).toEqual(event);
  });

  it('should update last message content (streaming)', () => {
    const { addMessage, updateLastMessage } = useChatStore.getState();
    
    addMessage({ role: 'assistant', content: 'Init', timestamp: 'now' });
    updateLastMessage('Updated');
    
    const state = useChatStore.getState();
    expect(state.messages[0].content).toBe('Updated');
  });
});
