import React, { useState, useEffect, useRef, useCallback } from 'react';
import './ThinkingStream.css';

// 思考token类型
export interface ThinkingToken {
  id: string;
  text: string;
  timestamp: number;
  type: 'thought' | 'tool' | 'observation' | 'reflection' | 'decision';
  confidence?: number;
  isComplete: boolean;
}

// 思考质量指标
export interface ThinkingMetrics {
  tokensPerSecond: number;
  averageConfidence: number;
  backtrackCount: number;
  toolHitRate: number;
  reasoningDepth: number;
  coherenceScore: number;
}

// 思考阶段
export type ThinkingPhase =
  | 'initializing'
  | 'analyzing'
  | 'reasoning'
  | 'tool_calling'
  | 'synthesizing'
  | 'finalizing'
  | 'complete';

interface ThinkingStreamProps {
  tokens: ThinkingToken[];
  metrics?: ThinkingMetrics;
  phase: ThinkingPhase;
  isPaused?: boolean;
  onPause?: () => void;
  onResume?: () => void;
  onSpeedChange?: (speed: number) => void;
  showMetrics?: boolean;
  maxVisibleTokens?: number;
}

export const ThinkingStream: React.FC<ThinkingStreamProps> = ({
  tokens,
  metrics,
  phase,
  isPaused = false,
  onPause,
  onResume,
  onSpeedChange,
  showMetrics = true,
  maxVisibleTokens = 100,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [displaySpeed, setDisplaySpeed] = useState(1);
  const [visibleTokens, setVisibleTokens] = useState<ThinkingToken[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const animationRef = useRef<number | null>(null);
  const lastUpdateRef = useRef<number>(0);

  // 打字机效果显示token
  useEffect(() => {
    if (isPaused) {
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current);
      }
      return;
    }

    const animate = (timestamp: number) => {
      const interval = 50 / displaySpeed; // 基础间隔50ms
      
      if (timestamp - lastUpdateRef.current >= interval) {
        if (currentIndex < tokens.length) {
          setVisibleTokens(prev => {
            const newTokens = [...prev, tokens[currentIndex]];
            // 保持最大可见数量
            if (newTokens.length > maxVisibleTokens) {
              return newTokens.slice(-maxVisibleTokens);
            }
            return newTokens;
          });
          setCurrentIndex(prev => prev + 1);
          lastUpdateRef.current = timestamp;
        }
      }
      
      animationRef.current = requestAnimationFrame(animate);
    };

    animationRef.current = requestAnimationFrame(animate);

    return () => {
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current);
      }
    };
  }, [tokens, currentIndex, displaySpeed, isPaused, maxVisibleTokens]);

  // 自动滚动到底部
  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [visibleTokens]);

  // 当新tokens到来时重置
  useEffect(() => {
    if (tokens.length > 0 && tokens[0].id !== visibleTokens[0]?.id) {
      setVisibleTokens([]);
      setCurrentIndex(0);
    }
  }, [tokens, visibleTokens]);

  // 获取阶段显示信息
  const getPhaseInfo = (phase: ThinkingPhase) => {
    const phaseConfig: Record<ThinkingPhase, { icon: string; label: string; color: string }> = {
      'initializing': { icon: '⚡', label: '初始化', color: '#64748b' },
      'analyzing': { icon: '🔍', label: '分析中', color: '#3b82f6' },
      'reasoning': { icon: '🧠', label: '推理中', color: '#8b5cf6' },
      'tool_calling': { icon: '🔧', label: '调用工具', color: '#f59e0b' },
      'synthesizing': { icon: '✨', label: '综合中', color: '#10b981' },
      'finalizing': { icon: '📝', label: '整理中', color: '#06b6d4' },
      'complete': { icon: '✅', label: '完成', color: '#22c55e' },
    };
    return phaseConfig[phase];
  };

  // 获取token样式
  const getTokenStyle = (token: ThinkingToken) => {
    const baseStyle: React.CSSProperties = {
      opacity: token.isComplete ? 1 : 0.7,
      animation: token.isComplete ? 'fadeIn 0.3s ease' : 'pulse 1.5s infinite',
    };

    switch (token.type) {
      case 'tool':
        return { ...baseStyle, color: '#f59e0b', fontWeight: 500 };
      case 'observation':
        return { ...baseStyle, color: '#10b981' };
      case 'reflection':
        return { ...baseStyle, color: '#ec4899', fontStyle: 'italic' };
      case 'decision':
        return { ...baseStyle, color: '#8b5cf6', fontWeight: 600 };
      default:
        return baseStyle;
    }
  };

  const phaseInfo = getPhaseInfo(phase);

  return (
    <div className="thinking-stream">
      {/* 头部状态栏 */}
      <div className="thinking-header">
        <div className="phase-indicator" style={{ background: phaseInfo.color }}>
          <span className="phase-icon">{phaseInfo.icon}</span>
          <span className="phase-label">{phaseInfo.label}</span>
        </div>
        
        <div className="thinking-controls">
          <button
            className="control-btn"
            onClick={isPaused ? onResume : onPause}
            title={isPaused ? '继续' : '暂停'}
          >
            {isPaused ? '▶️' : '⏸️'}
          </button>
          
          <div className="speed-control">
            <span className="speed-label">速度:</span>
            <input
              type="range"
              min="0.5"
              max="3"
              step="0.5"
              value={displaySpeed}
              onChange={(e) => {
                const speed = parseFloat(e.target.value);
                setDisplaySpeed(speed);
                onSpeedChange?.(speed);
              }}
              className="speed-slider"
            />
            <span className="speed-value">{displaySpeed}x</span>
          </div>
        </div>
      </div>

      {/* 质量指标面板 */}
      {showMetrics && metrics && (
        <div className="metrics-panel">
          <div className="metric-item">
            <div className="metric-icon">⚡</div>
            <div className="metric-content">
              <span className="metric-value">{metrics.tokensPerSecond.toFixed(1)}</span>
              <span className="metric-label">token/s</span>
            </div>
          </div>
          
          <div className="metric-item">
            <div className="metric-icon">🎯</div>
            <div className="metric-content">
              <span className="metric-value">{(metrics.averageConfidence * 100).toFixed(0)}%</span>
              <span className="metric-label">置信度</span>
            </div>
            <div className="metric-bar">
              <div 
                className="metric-fill"
                style={{ 
                  width: `${metrics.averageConfidence * 100}%`,
                  background: metrics.averageConfidence > 0.8 ? '#22c55e' : 
                             metrics.averageConfidence > 0.5 ? '#eab308' : '#ef4444'
                }}
              />
            </div>
          </div>
          
          <div className="metric-item">
            <div className="metric-icon">🔄</div>
            <div className="metric-content">
              <span className="metric-value">{metrics.backtrackCount}</span>
              <span className="metric-label">回溯</span>
            </div>
          </div>
          
          <div className="metric-item">
            <div className="metric-icon">🔧</div>
            <div className="metric-content">
              <span className="metric-value">{(metrics.toolHitRate * 100).toFixed(0)}%</span>
              <span className="metric-label">工具命中</span>
            </div>
            <div className="metric-bar">
              <div 
                className="metric-fill"
                style={{ width: `${metrics.toolHitRate * 100}%` }}
              />
            </div>
          </div>
          
          <div className="metric-item">
            <div className="metric-icon">📊</div>
            <div className="metric-content">
              <span className="metric-value">{metrics.reasoningDepth}</span>
              <span className="metric-label">推理深度</span>
            </div>
          </div>
          
          <div className="metric-item">
            <div className="metric-icon">🔗</div>
            <div className="metric-content">
              <span className="metric-value">{(metrics.coherenceScore * 100).toFixed(0)}%</span>
              <span className="metric-label">连贯性</span>
            </div>
          </div>
        </div>
      )}

      {/* Token流显示区 */}
      <div className="thinking-content" ref={containerRef}>
        {visibleTokens.length === 0 ? (
          <div className="thinking-placeholder">
            <div className="thinking-dots">
              <span></span>
              <span></span>
              <span></span>
            </div>
            <span className="placeholder-text">正在思考...</span>
          </div>
        ) : (
          <div className="token-stream">
            {visibleTokens.map((token, index) => (
              <span
                key={`${token.id}-${index}`}
                className={`thinking-token token-${token.type}`}
                style={getTokenStyle(token)}
                title={token.confidence ? `置信度: ${(token.confidence * 100).toFixed(1)}%` : undefined}
              >
                {token.text}
                {token.confidence !== undefined && token.confidence < 0.5 && (
                  <span className="low-confidence-indicator">⚠️</span>
                )}
              </span>
            ))}
            {!isPaused && currentIndex < tokens.length && (
              <span className="typing-cursor">|</span>
            )}
          </div>
        )}
      </div>

      {/* 底部进度条 */}
      <div className="thinking-progress">
        <div className="progress-bar">
          <div 
            className="progress-fill"
            style={{ 
              width: `${tokens.length > 0 ? (currentIndex / tokens.length) * 100 : 0}%`,
              background: phaseInfo.color
            }}
          />
        </div>
        <div className="progress-info">
          <span>{currentIndex} / {tokens.length} tokens</span>
          <span>{((currentIndex / Math.max(tokens.length, 1)) * 100).toFixed(0)}%</span>
        </div>
      </div>
    </div>
  );
};

export default ThinkingStream;
