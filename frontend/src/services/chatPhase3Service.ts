/**
 * Phase 3 Chat Enhancement API Service
 * 
 * Provides API methods for:
 * - Token usage statistics (3-1)
 * - TopK dynamic recommendation (3-2)
 * - Dual search (RAG + History) (3-3)
 * - Message stars/favorites (3-4)
 */

import { api } from './api';

// ============= Type Definitions =============

// Token Usage Types (3-1)
export interface TokenUsageResponse {
  status: 'success' | 'error';
  session_id: string;
  total_tokens: number;
  prompt_tokens: number;
  completion_tokens: number;
  token_limit: number;
  usage_percentage: number;
  last_updated: number;
}

export interface CompressSessionRequest {
  keep_recent?: number;
}

export interface CompressSessionResponse {
  status: 'success' | 'error';
  session_id: string;
  compressed: boolean;
  kept_messages: number;
}

// TopK Recommendation Types (3-2)
export interface TopKRecommendResponse {
  status: 'success' | 'error';
  recommended_topk: number;
  current_topk: number;
  reason: string;
  token_usage_percentage: number;
  session_token_count: number;
  session_token_limit: number;
}

// Dual Search Types (3-3)
export type SearchMode = 'dual' | 'kb_only' | 'history_only';

export interface SearchResult {
  source: string;
  source_type: 'knowledge_base' | 'history';
  content: string;
  score: number;
  session_id: string | null;
  message_id: string | null;
}

export interface DualSearchResponse {
  status: 'success' | 'error';
  query: string;
  mode: SearchMode;
  alpha: number;
  kb_count: number;
  history_count: number;
  results: SearchResult[];
}

// Message Stars Types (3-4)
export interface MessageStar {
  id: string;
  session_id: string;
  message_id: string;
  created_at: number;
}

export interface StarMessageRequest {
  message_id: string;
}

export interface StarMessageResponse {
  status: 'starred' | 'error';
  id: string;
  session_id: string;
  message_id: string;
  created_at: number;
}

export interface StarListResponse {
  status: 'success' | 'error';
  session_id: string;
  count: number;
  stars: MessageStar[];
}

export interface StarStatusResponse {
  status: 'success' | 'error';
  session_id: string;
  message_id: string;
  starred: boolean;
}

export interface StarCountResponse {
  status: 'success' | 'error';
  session_id: string;
  star_count: number;
}

// ============= API Service =============

export const chatPhase3Service = {
  // ============= 3-1: Token Usage APIs =============

  /**
   * Get token usage statistics for a session
   * GET /api/v1/chat/sessions/:id/token-usage
   */
  getTokenUsage: async (sessionId: string): Promise<TokenUsageResponse> => {
    return api.get<TokenUsageResponse>(`/v1/chat/sessions/${sessionId}/token-usage`);
  },

  /**
   * Compress session context
   * POST /api/v1/chat/sessions/:id/compress
   */
  compressSession: async (
    sessionId: string,
    options?: CompressSessionRequest
  ): Promise<CompressSessionResponse> => {
    return api.post<CompressSessionResponse>(
      `/v1/chat/sessions/${sessionId}/compress`,
      options || {}
    );
  },

  // ============= 3-2: TopK Recommendation APIs =============

  /**
   * Get recommended TopK value based on token usage
   * GET /api/v1/rag/topk-recommend
   */
  getTopKRecommend: async (
    sessionId: string,
    currentTopK: number
  ): Promise<TopKRecommendResponse> => {
    return api.get<TopKRecommendResponse>(
      `/v1/rag/topk-recommend?session_id=${encodeURIComponent(sessionId)}&current_topk=${currentTopK}`
    );
  },

  // ============= 3-3: Dual Search APIs =============

  /**
   * Search across knowledge base and chat history
   * GET /api/v1/rag/search?q=&mode=dual&alpha=0.6
   */
  dualSearch: async (
    query: string,
    mode: SearchMode = 'dual',
    alpha: number = 0.6
  ): Promise<DualSearchResponse> => {
    return api.get<DualSearchResponse>(
      `/v1/rag/search?q=${encodeURIComponent(query)}&mode=${mode}&alpha=${alpha}`
    );
  },

  // ============= 3-4: Message Stars APIs =============

  /**
   * Star a message
   * POST /api/v1/chat/sessions/:id/stars
   */
  starMessage: async (
    sessionId: string,
    messageId: string
  ): Promise<StarMessageResponse> => {
    return api.post<StarMessageResponse>(
      `/v1/chat/sessions/${sessionId}/stars`,
      { message_id: messageId }
    );
  },

  /**
   * Unstar a message
   * DELETE /api/v1/chat/sessions/:id/stars/:messageId
   */
  unstarMessage: async (
    sessionId: string,
    messageId: string
  ): Promise<void> => {
    return api.delete<void>(
      `/v1/chat/sessions/${sessionId}/stars/${messageId}`
    );
  },

  /**
   * List all starred messages in a session
   * GET /api/v1/chat/sessions/:id/stars
   */
  listStars: async (sessionId: string): Promise<StarListResponse> => {
    return api.get<StarListResponse>(
      `/v1/chat/sessions/${sessionId}/stars`
    );
  },

  /**
   * Check if a message is starred
   * GET /api/v1/chat/sessions/:id/stars/:messageId
   */
  getStarStatus: async (
    sessionId: string,
    messageId: string
  ): Promise<StarStatusResponse> => {
    return api.get<StarStatusResponse>(
      `/v1/chat/sessions/${sessionId}/stars/${messageId}`
    );
  },

  /**
   * Get star count for a session
   * GET /api/v1/chat/sessions/:id/star-count
   */
  getStarCount: async (sessionId: string): Promise<StarCountResponse> => {
    return api.get<StarCountResponse>(
      `/v1/chat/sessions/${sessionId}/star-count`
    );
  },
};
