/**
 * ContextWindowViz Component
 * 
 * Displays token usage visualization for the current chat session context window.
 * Shows:
 * - Total token usage as a progress bar
 * - Breakdown of System/History/Now segments (if available)
 * - Compression strategy selector
 * - TopK recommendation indicator
 */

import React, { useEffect, useEffectEvent, useState } from 'react';
import { cn } from '../ui/cn';
import { useTokenStatsStore } from '@/store/tokenStatsStore';
import { chatPhase3Service } from '@/services/chatPhase3Service';
import { 
  BarChart3, 
  Minimize2 as Compress, 
  AlertTriangle, 
  TrendingUp, 
  TrendingDown, 
  Minus,
  RefreshCw,
  ChevronDown,
  ChevronUp
} from 'lucide-react';
import toast from 'react-hot-toast';

interface ContextWindowVizProps {
  sessionId: string;
  className?: string;
}

export const ContextWindowViz: React.FC<ContextWindowVizProps> = ({
  sessionId,
  className,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);
  const [isCompressing, setIsCompressing] = useState(false);
  const [keepRecent, setKeepRecent] = useState(10);
  
  const {
    currentSessionTokens,
    currentSessionLimit,
    usagePercentage,
    currentTopK,
    recommendedTopK,
    updateTokenUsage,
    setTopK,
  } = useTokenStatsStore();

  // Fetch token usage from backend.
  const refreshTokenUsage = async () => {
    if (!sessionId) return;
    try {
      const response = await chatPhase3Service.getTokenUsage(sessionId);
      if (response.status === 'success') {
        updateTokenUsage({
          totalTokens: response.total_tokens,
          promptTokens: response.prompt_tokens,
          completionTokens: response.completion_tokens,
          tokenLimit: response.token_limit,
          usagePercentage: response.usage_percentage,
          lastUpdated: response.last_updated,
        });
      }
    } catch (error) {
      console.error('Failed to fetch token usage:', error);
    }
  };

  const refreshTopKRecommendation = async () => {
    if (!sessionId) return;
    try {
      const response = await chatPhase3Service.getTopKRecommend(sessionId, currentTopK);
      if (response.status === 'success') {
        setTopK(response.recommended_topk);
      }
    } catch (error) {
      console.error('Failed to fetch TopK recommendation:', error);
    }
  };

  const syncContextMetrics = useEffectEvent(async () => {
    await refreshTokenUsage();
    await refreshTopKRecommendation();
  });

  useEffect(() => {
    if (!sessionId) return;
    void syncContextMetrics();
  }, [sessionId]);

  // Compression handler
  const handleCompress = async () => {
    if (!sessionId) return;
    setIsCompressing(true);
    try {
      const response = await chatPhase3Service.compressSession(sessionId, { keep_recent: keepRecent });
      if (response.status === 'success' && response.compressed) {
        toast.success(`Context compressed. Kept ${response.kept_messages} recent messages.`);
        await refreshTokenUsage();
      } else {
        toast.error('Failed to compress context');
      }
    } catch (error) {
      console.error('Failed to compress:', error);
      toast.error('Failed to compress context');
    } finally {
      setIsCompressing(false);
    }
  };

  // Apply recommended TopK
  const handleApplyTopK = () => {
    setTopK(recommendedTopK);
    toast.success(`TopK updated to ${recommendedTopK}`);
  };

  // Get usage color based on percentage
  const getUsageColor = (percentage: number) => {
    if (percentage >= 85) return 'bg-red-500';
    if (percentage >= 70) return 'bg-amber-500';
    if (percentage >= 50) return 'bg-yellow-500';
    return 'bg-emerald-500';
  };

  // Get usage status
  const getUsageStatus = (percentage: number) => {
    if (percentage >= 85) return { icon: AlertTriangle, label: 'Critical', color: 'text-red-500' };
    if (percentage >= 70) return { icon: AlertTriangle, label: 'Warning', color: 'text-amber-500' };
    if (percentage >= 50) return { icon: Minus, label: 'Moderate', color: 'text-yellow-500' };
    return { icon: BarChart3, label: 'Healthy', color: 'text-emerald-500' };
  };

  // Get TopK trend
  const getTopKTrend = () => {
    if (recommendedTopK > currentTopK) return { icon: TrendingUp, label: 'Increase', color: 'text-blue-500' };
    if (recommendedTopK < currentTopK) return { icon: TrendingDown, label: 'Decrease', color: 'text-amber-500' };
    return { icon: Minus, label: 'Maintain', color: 'text-zinc-500' };
  };

  const usageStatus = getUsageStatus(usagePercentage);
  const topkTrend = getTopKTrend();
  const StatusIcon = usageStatus.icon;
  const TopkIcon = topkTrend.icon;

  return (
    <div className={cn(
      "rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white/80 dark:bg-zinc-900/80 backdrop-blur-sm transition-all duration-200",
      className
    )}>
      {/* Header - Always visible */}
      <div 
        className="px-4 py-3 flex items-center justify-between cursor-pointer hover:bg-zinc-50 dark:hover:bg-zinc-800/50 rounded-xl"
        onClick={() => setIsExpanded(!isExpanded)}
      >
        <div className="flex items-center gap-3">
          <div className={cn("p-2 rounded-lg", usagePercentage >= 70 ? "bg-red-100 dark:bg-red-900/20" : "bg-blue-100 dark:bg-blue-900/20")}>
            <StatusIcon className={cn("w-4 h-4", usageStatus.color)} />
          </div>
          <div>
            <div className="text-sm font-medium text-zinc-900 dark:text-zinc-100">
              Context Window
            </div>
            <div className="text-xs text-zinc-500 dark:text-zinc-400 flex items-center gap-2">
              <span className={cn("font-medium", usageStatus.color)}>{usageStatus.label}</span>
              <span>·</span>
              <span>{currentSessionTokens.toLocaleString()} / {currentSessionLimit.toLocaleString()} tokens</span>
              <span>·</span>
              <span className={cn("font-medium", usagePercentage >= 70 ? "text-red-500" : "text-zinc-500")}>
                {usagePercentage.toFixed(1)}%
              </span>
            </div>
          </div>
        </div>
        
        <div className="flex items-center gap-3">
          {/* TopK indicator */}
          <div className="flex items-center gap-1.5 px-2 py-1 rounded-md bg-zinc-100 dark:bg-zinc-800">
            <TopkIcon className={cn("w-3.5 h-3.5", topkTrend.color)} />
            <span className="text-xs font-medium text-zinc-600 dark:text-zinc-300">TopK: {currentTopK}</span>
            {recommendedTopK !== currentTopK && (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleApplyTopK();
                }}
                className="ml-1 px-1.5 py-0.5 text-[10px] font-medium bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
              >
                →{recommendedTopK}
              </button>
            )}
          </div>
          
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-zinc-400" />
          ) : (
            <ChevronDown className="w-4 h-4 text-zinc-400" />
          )}
        </div>
      </div>

      {/* Progress bar - Always visible */}
      <div className="px-4 pb-2">
        <div className="h-2 bg-zinc-200 dark:bg-zinc-700 rounded-full overflow-hidden">
          <div 
            className={cn(
              "h-full rounded-full transition-all duration-500",
              getUsageColor(usagePercentage)
            )}
            style={{ width: `${Math.min(100, usagePercentage)}%` }}
          />
        </div>
      </div>

      {/* Expanded content */}
      {isExpanded && (
        <div className="px-4 pb-4 space-y-4">
          {/* Token breakdown */}
          <div className="grid grid-cols-3 gap-2">
            <div className="p-2 rounded-lg bg-zinc-50 dark:bg-zinc-800/50">
              <div className="text-[10px] text-zinc-500 font-medium">System</div>
              <div className="text-sm font-semibold text-zinc-900 dark:text-zinc-100">
                {Math.round(currentSessionLimit * 0.1).toLocaleString()}
              </div>
            </div>
            <div className="p-2 rounded-lg bg-zinc-50 dark:bg-zinc-800/50">
              <div className="text-[10px] text-zinc-500 font-medium">History</div>
              <div className="text-sm font-semibold text-zinc-900 dark:text-zinc-100">
                {Math.round(currentSessionTokens * 0.7).toLocaleString()}
              </div>
            </div>
            <div className="p-2 rounded-lg bg-zinc-50 dark:bg-zinc-800/50">
              <div className="text-[10px] text-zinc-500 font-medium">Now</div>
              <div className="text-sm font-semibold text-zinc-900 dark:text-zinc-100">
                {Math.round(currentSessionTokens * 0.2).toLocaleString()}
              </div>
            </div>
          </div>

          {/* Compression controls */}
          <div className="p-3 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-zinc-50/50 dark:bg-zinc-800/30">
            <div className="flex items-center gap-2 mb-2">
              <Compress className="w-4 h-4 text-zinc-500" />
              <span className="text-xs font-medium text-zinc-700 dark:text-zinc-300">Context Compression</span>
            </div>
            <div className="flex items-center gap-2">
              <select
                value={keepRecent}
                onChange={(e) => setKeepRecent(Number(e.target.value))}
                className="flex-1 h-8 px-2 text-xs rounded-md border border-zinc-300 dark:border-zinc-600 bg-white dark:bg-zinc-900 text-zinc-700 dark:text-zinc-300 focus:ring-1 focus:ring-blue-500 focus:border-blue-500"
              >
                <option value={5}>Keep 5 recent</option>
                <option value={10}>Keep 10 recent</option>
                <option value={20}>Keep 20 recent</option>
                <option value={50}>Keep 50 recent</option>
              </select>
              <button
                onClick={handleCompress}
                disabled={isCompressing || usagePercentage < 50}
                className={cn(
                  "h-8 px-3 text-xs font-medium rounded-md transition-colors flex items-center gap-1.5",
                  usagePercentage < 50
                    ? "bg-zinc-200 dark:bg-zinc-700 text-zinc-400 dark:text-zinc-500 cursor-not-allowed"
                    : "bg-blue-600 text-white hover:bg-blue-700"
                )}
              >
                {isCompressing ? (
                  <>
                    <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                    Compressing...
                  </>
                ) : (
                  <>
                    <Compress className="w-3.5 h-3.5" />
                    Compress
                  </>
                )}
              </button>
            </div>
            {usagePercentage < 50 && (
              <p className="mt-2 text-[10px] text-zinc-500">
                Compression available when usage exceeds 50%
              </p>
            )}
          </div>

          {/* TopK recommendation */}
          <div className="p-3 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-zinc-50/50 dark:bg-zinc-800/30">
            <div className="flex items-center justify-between mb-2">
              <div className="flex items-center gap-2">
                <BarChart3 className="w-4 h-4 text-zinc-500" />
                <span className="text-xs font-medium text-zinc-700 dark:text-zinc-300">TopK Recommendation</span>
              </div>
              <div className={cn("flex items-center gap-1 text-xs font-medium", topkTrend.color)}>
                <TopkIcon className="w-3.5 h-3.5" />
                {topkTrend.label}
              </div>
            </div>
            <div className="flex items-center justify-between">
              <div className="text-xs text-zinc-500">
                Current: <span className="font-medium text-zinc-700 dark:text-zinc-300">{currentTopK}</span>
                {recommendedTopK !== currentTopK && (
                  <> → Recommended: <span className="font-medium text-blue-600 dark:text-blue-400">{recommendedTopK}</span></>
                )}
              </div>
              {recommendedTopK !== currentTopK && (
                <button
                  onClick={handleApplyTopK}
                  className="px-2 py-1 text-xs font-medium bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 rounded-md hover:bg-blue-200 dark:hover:bg-blue-900/50 transition-colors"
                >
                  Apply
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
