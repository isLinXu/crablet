import { beforeEach, describe, expect, it } from 'vitest';
import { syncTokenUsageToBackend, useTokenStatsStore } from '../tokenStatsStore';

describe('useTokenStatsStore', () => {
  beforeEach(() => {
    localStorage.clear();
    useTokenStatsStore.setState({
      currentSessionTokens: 0,
      currentSessionLimit: 128000,
      usagePercentage: 0,
      sessionHistory: [],
      currentTopK: 5,
      recommendedTopK: 5,
    });
  });

  it('updates usage and recommends lower topK as context pressure grows', () => {
    useTokenStatsStore.getState().updateTokenUsage({
      totalTokens: 64000,
      tokenLimit: 80000,
    });

    const state = useTokenStatsStore.getState();
    expect(state.currentSessionTokens).toBe(64000);
    expect(state.currentSessionLimit).toBe(80000);
    expect(state.usagePercentage).toBe(80);
    expect(state.recommendedTopK).toBe(3);
  });

  it('setTopK keeps current and recommended values aligned', () => {
    useTokenStatsStore.getState().setTopK(9);
    expect(useTokenStatsStore.getState()).toMatchObject({
      currentTopK: 9,
      recommendedTopK: 9,
    });
  });

  it('caps historical entries at the latest 50 sessions', () => {
    const { addToHistory } = useTokenStatsStore.getState();
    for (let index = 0; index < 55; index += 1) {
      addToHistory({
        sessionId: `s-${index}`,
        totalTokens: index,
        promptTokens: index,
        completionTokens: 0,
        tokenLimit: 100,
        usagePercentage: index,
        lastUpdated: index,
      });
    }

    const { sessionHistory } = useTokenStatsStore.getState();
    expect(sessionHistory).toHaveLength(50);
    expect(sessionHistory[0].sessionId).toBe('s-54');
    expect(sessionHistory.at(-1)?.sessionId).toBe('s-5');
  });

  it('syncTokenUsageToBackend appends to local history even before backend wiring exists', async () => {
    await syncTokenUsageToBackend('session-1', {
      sessionId: 'session-1',
      totalTokens: 1200,
      promptTokens: 900,
      completionTokens: 300,
      tokenLimit: 8000,
      usagePercentage: 15,
      lastUpdated: 123,
    });

    expect(useTokenStatsStore.getState().sessionHistory).toHaveLength(1);
    expect(useTokenStatsStore.getState().sessionHistory[0].sessionId).toBe('session-1');
  });
});
