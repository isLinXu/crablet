import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { chatPhase3Service, type MessageStar, type StarListResponse } from '@/services/chatPhase3Service';

/**
 * Message Stars Store
 * 
 * Manages message favorite/star functionality for chat sessions.
 * Provides:
 * - Star/unstar messages
 * - List starred messages
 * - Check star status
 * - Star count tracking
 */

export interface MessageStarsState {
  // Current session stars
  stars: MessageStar[];
  starCount: number;
  isLoading: boolean;
  error: string | null;

  // Starred message IDs for quick lookup (current session)
  starredMessageIds: Set<string>;

  // Actions
  loadStars: (sessionId: string) => Promise<void>;
  starMessage: (sessionId: string, messageId: string) => Promise<boolean>;
  unstarMessage: (sessionId: string, messageId: string) => Promise<boolean>;
  isStarred: (messageId: string) => boolean;
  clearStars: () => void;
  setError: (error: string | null) => void;
}

export const useMessageStarsStore = create<MessageStarsState>()(
  persist(
    (set, get) => ({
      // Default values
      stars: [],
      starCount: 0,
      isLoading: false,
      error: null,
      starredMessageIds: new Set<string>(),

      loadStars: async (sessionId: string) => {
        set({ isLoading: true, error: null });
        try {
          const response: StarListResponse = await chatPhase3Service.listStars(sessionId);
          if (response.status === 'success') {
            const starredIds = new Set(response.stars.map(s => s.message_id));
            set({
              stars: response.stars,
              starCount: response.count,
              starredMessageIds: starredIds,
              isLoading: false,
            });
          } else {
            set({ isLoading: false, error: 'Failed to load stars' });
          }
        } catch (error) {
          console.error('Failed to load stars:', error);
          set({ isLoading: false, error: 'Failed to load stars' });
        }
      },

      starMessage: async (sessionId: string, messageId: string) => {
        try {
          const response = await chatPhase3Service.starMessage(sessionId, messageId);
          if (response.status === 'starred') {
            const newStar: MessageStar = {
              id: response.id,
              session_id: response.session_id,
              message_id: response.message_id,
              created_at: response.created_at,
            };
            const newStarredIds = new Set(get().starredMessageIds);
            newStarredIds.add(messageId);
            set(state => ({
              stars: [newStar, ...state.stars],
              starCount: state.starCount + 1,
              starredMessageIds: newStarredIds,
            }));
            return true;
          }
          return false;
        } catch (error) {
          console.error('Failed to star message:', error);
          set({ error: 'Failed to star message' });
          return false;
        }
      },

      unstarMessage: async (sessionId: string, messageId: string) => {
        try {
          await chatPhase3Service.unstarMessage(sessionId, messageId);
          const newStarredIds = new Set(get().starredMessageIds);
          newStarredIds.delete(messageId);
          set(state => ({
            stars: state.stars.filter(s => s.message_id !== messageId),
            starCount: Math.max(0, state.starCount - 1),
            starredMessageIds: newStarredIds,
          }));
          return true;
        } catch (error) {
          console.error('Failed to unstar message:', error);
          set({ error: 'Failed to unstar message' });
          return false;
        }
      },

      isStarred: (messageId: string) => {
        return get().starredMessageIds.has(messageId);
      },

      clearStars: () => {
        set({
          stars: [],
          starCount: 0,
          starredMessageIds: new Set<string>(),
          error: null,
        });
      },

      setError: (error: string | null) => {
        set({ error });
      },
    }),
    {
      name: 'crablet-message-stars',
      partialize: (state) => ({
        // Only persist non-sensitive data
        starCount: state.starCount,
      }),
    }
  )
);
