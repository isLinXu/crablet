/**
 * StarsList Component
 * 
 * Displays a list of starred/favorited messages for the current session.
 * Features:
 * - Lists all starred messages with timestamps
 * - Quick unstar action
 * - Navigate to starred message in context
 */

import React, { useEffect } from 'react';
import { X, Star, Trash2, Clock } from 'lucide-react';
import { useMessageStarsStore } from '@/store/messageStarsStore';
import { Button } from '../ui/Button';

interface StarsListProps {
  isOpen: boolean;
  onClose: () => void;
  sessionId: string;
}

export const StarsList: React.FC<StarsListProps> = ({
  isOpen,
  onClose,
  sessionId,
}) => {
  const {
    stars,
    starCount,
    isLoading,
    loadStars,
    unstarMessage,
  } = useMessageStarsStore();

  // Load stars when modal opens
  useEffect(() => {
    if (isOpen && sessionId) {
      loadStars(sessionId);
    }
  }, [isOpen, sessionId, loadStars]);

  const handleUnstar = async (messageId: string) => {
    if (!sessionId) return;
    await unstarMessage(sessionId, messageId);
  };

  // Format timestamp
  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    return date.toLocaleString('zh-CN', {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/50 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-full max-w-lg mx-4 bg-white dark:bg-zinc-900 rounded-2xl shadow-2xl overflow-hidden">
        {/* Header */}
        <div className="px-6 py-4 border-b border-zinc-200 dark:border-zinc-800 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="p-2 rounded-lg bg-amber-100 dark:bg-amber-900/30">
              <Star className="w-5 h-5 text-amber-600 dark:text-amber-400" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-zinc-900 dark:text-zinc-100">
                Starred Messages
              </h2>
              <p className="text-sm text-zinc-500 dark:text-zinc-400">
                {starCount} {starCount === 1 ? 'message' : 'messages'} starred
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-zinc-100 dark:hover:bg-zinc-800 rounded-lg transition-colors"
          >
            <X className="w-5 h-5 text-zinc-500" />
          </button>
        </div>

        {/* Content */}
        <div className="max-h-[60vh] overflow-y-auto">
          {isLoading ? (
            <div className="p-8 text-center">
              <div className="inline-flex items-center gap-2 text-zinc-500">
                <div className="w-5 h-5 border-2 border-zinc-300 border-t-zinc-600 rounded-full animate-spin" />
                Loading stars...
              </div>
            </div>
          ) : stars.length === 0 ? (
            <div className="p-8 text-center">
              <div className="inline-flex items-center justify-center w-16 h-16 mb-4 rounded-full bg-zinc-100 dark:bg-zinc-800">
                <Star className="w-8 h-8 text-zinc-400" />
              </div>
              <p className="text-zinc-500 dark:text-zinc-400">
                No starred messages yet
              </p>
              <p className="text-sm text-zinc-400 dark:text-zinc-500 mt-1">
                Click the star icon on any message to save it here
              </p>
            </div>
          ) : (
            <div className="divide-y divide-zinc-100 dark:divide-zinc-800">
              {stars.map((star) => (
                <div
                  key={star.id}
                  className="p-4 hover:bg-zinc-50 dark:hover:bg-zinc-800/50 transition-colors group"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-1">
                        <span className="inline-flex items-center gap-1 text-xs text-zinc-500">
                          <Clock className="w-3 h-3" />
                          {formatTime(star.created_at)}
                        </span>
                      </div>
                      <p className="text-sm text-zinc-600 dark:text-zinc-300 font-mono bg-zinc-100 dark:bg-zinc-800 px-2 py-1 rounded">
                        {star.message_id}
                      </p>
                    </div>
                    <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                      <button
                        onClick={() => handleUnstar(star.message_id)}
                        className="p-2 text-zinc-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
                        title="Unstar"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-6 py-4 border-t border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-900/50">
          <Button
            variant="secondary"
            onClick={onClose}
            className="w-full"
          >
            Close
          </Button>
        </div>
      </div>
    </div>
  );
};
