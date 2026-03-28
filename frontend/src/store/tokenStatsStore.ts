import { create } from 'zustand';
import { persist } from 'zustand/middleware';

/**
 * Token Usage Statistics Store
 * 
 * Tracks token usage for context window management and TopK dynamic adjustment.
 * Data flows:
 * - Hot data (real-time): Stored in Redis for fast access
 * - Cold data (historical): Stored in SQLite for persistence
 */

export interface TokenUsageStats {
  sessionId: string;
  totalTokens: number;
  promptTokens: number;
  completionTokens: number;
  tokenLimit: number;
  usagePercentage: number;
  lastUpdated: number;
}

export interface TokenStatsState {
  // Current session token usage
  currentSessionTokens: number;
  currentSessionLimit: number;
  usagePercentage: number;
  
  // Historical stats (last N sessions)
  sessionHistory: TokenUsageStats[];
  
  // TopK related
  currentTopK: number;
  recommendedTopK: number;
  
  // Actions
  updateTokenUsage: (stats: Partial<TokenUsageStats>) => void;
  setTopK: (k: number) => void;
  recommendTopK: (percentage: number) => void;
  addToHistory: (stats: TokenUsageStats) => void;
  clearHistory: () => void;
  reset: () => void;
}

/**
 * Calculate recommended TopK based on context usage percentage
 * - usage < 50%: maintain current TopK
 * - usage 50-70%: reduce TopK by 20%
 * - usage 70-85%: reduce TopK by 40%
 * - usage > 85%: reduce TopK by 60%
 */
const calculateRecommendedTopK = (usagePercentage: number, currentTopK: number): number => {
  if (usagePercentage < 50) return currentTopK;
  if (usagePercentage < 70) return Math.max(1, Math.floor(currentTopK * 0.8));
  if (usagePercentage < 85) return Math.max(1, Math.floor(currentTopK * 0.6));
  return Math.max(1, Math.floor(currentTopK * 0.4));
};

export const useTokenStatsStore = create<TokenStatsState>()(
  persist(
    (set, get) => ({
      // Default values
      currentSessionTokens: 0,
      currentSessionLimit: 128000, // Default context window
      usagePercentage: 0,
      sessionHistory: [],
      currentTopK: 5,
      recommendedTopK: 5,
      
      updateTokenUsage: (stats) => {
        const { totalTokens, tokenLimit } = stats;
        const newPercentage = tokenLimit && tokenLimit > 0 
          ? (totalTokens ?? get().currentSessionTokens) / tokenLimit * 100 
          : 0;
        
        set((state) => ({
          currentSessionTokens: totalTokens ?? state.currentSessionTokens,
          currentSessionLimit: tokenLimit ?? state.currentSessionLimit,
          usagePercentage: newPercentage,
        }));
        
        // Auto-recommend TopK based on usage
        get().recommendTopK(newPercentage);
      },
      
      setTopK: (k) => {
        set({ currentTopK: k, recommendedTopK: k });
      },
      
      recommendTopK: (percentage) => {
        const currentTopK = get().currentTopK;
        const recommended = calculateRecommendedTopK(percentage, currentTopK);
        set({ recommendedTopK: recommended });
      },
      
      addToHistory: (stats) => {
        set((state) => ({
          sessionHistory: [stats, ...state.sessionHistory].slice(0, 50), // Keep last 50 sessions
        }));
      },
      
      clearHistory: () => {
        set({ sessionHistory: [] });
      },
      
      reset: () => {
        set({
          currentSessionTokens: 0,
          currentSessionLimit: 128000,
          usagePercentage: 0,
          recommendedTopK: 5,
        });
      },
    }),
    {
      name: 'crablet-token-stats',
      partialize: (state) => ({
        currentTopK: state.currentTopK,
        sessionHistory: state.sessionHistory,
      }),
    }
  )
);

// Helper to sync with backend storage
export const syncTokenUsageToBackend = async (sessionId: string, stats: TokenUsageStats) => {
  try {
    // TODO: Call backend API to persist token stats
    // await fetch('/api/v1/storage/token-usage', {
    //   method: 'POST',
    //   headers: { 'Content-Type': 'application/json' },
    //   body: JSON.stringify({ sessionId, ...stats }),
    // });
    
    // Update local store with history
    useTokenStatsStore.getState().addToHistory(stats);
  } catch (error) {
    console.error('Failed to sync token usage:', error);
  }
};
