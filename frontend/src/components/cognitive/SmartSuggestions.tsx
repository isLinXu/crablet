import React, { useState, useEffect, useCallback, useRef } from 'react';
import './SmartSuggestions.css';

// 建议类型
export type SuggestionType = 'follow_up' | 'clarification' | 'action' | 'exploration' | 'correction';

// 建议项
export interface Suggestion {
  id: string;
  text: string;
  type: SuggestionType;
  confidence: number;
  icon?: string;
  shortcut?: string;
  metadata?: {
    relatedContext?: string;
    estimatedTokens?: number;
    skillName?: string;
  };
}

// 快捷操作
export interface QuickAction {
  id: string;
  label: string;
  icon: string;
  action: () => void;
  shortcut?: string;
  category: 'code' | 'analysis' | 'creative' | 'utility';
}

interface SmartSuggestionsProps {
  context: string;
  lastResponse?: string;
  conversationHistory: Array<{ role: string; content: string }>;
  onSuggestionClick: (suggestion: Suggestion) => void;
  onQuickActionClick: (action: QuickAction) => void;
  maxSuggestions?: number;
  position?: 'above' | 'below';
  visible?: boolean;
}

// 模拟建议生成（实际应调用后端API）
const generateSuggestions = (
  context: string,
  lastResponse?: string,
  history?: Array<{ role: string; content: string }>
): Suggestion[] => {
  const suggestions: Suggestion[] = [];
  
  // 基于上下文的启发式建议生成
  const lowerContext = context.toLowerCase();
  const lowerResponse = lastResponse?.toLowerCase() || '';
  
  // 代码相关建议
  if (lowerContext.includes('code') || lowerContext.includes('函数') || lowerContext.includes('bug')) {
    suggestions.push(
      {
        id: '1',
        text: '优化这段代码的性能',
        type: 'action',
        confidence: 0.92,
        icon: '⚡',
        shortcut: 'Ctrl+O',
      },
      {
        id: '2',
        text: '添加单元测试',
        type: 'action',
        confidence: 0.88,
        icon: '🧪',
        shortcut: 'Ctrl+T',
      },
      {
        id: '3',
        text: '解释这段代码的工作原理',
        type: 'clarification',
        confidence: 0.85,
        icon: '💡',
      }
    );
  }
  
  // 分析相关建议
  if (lowerContext.includes('分析') || lowerContext.includes('数据') || lowerContext.includes('报告')) {
    suggestions.push(
      {
        id: '4',
        text: '生成可视化图表',
        type: 'action',
        confidence: 0.90,
        icon: '📊',
      },
      {
        id: '5',
        text: '深入分析根本原因',
        type: 'exploration',
        confidence: 0.87,
        icon: '🔍',
      }
    );
  }
  
  // 通用跟进建议
  suggestions.push(
    {
      id: '6',
      text: '详细说明这一点',
      type: 'follow_up',
      confidence: 0.80,
      icon: '📝',
    },
    {
      id: '7',
      text: '提供具体示例',
      type: 'follow_up',
      confidence: 0.78,
      icon: '💻',
    },
    {
      id: '8',
      text: '还有其他方法吗？',
      type: 'exploration',
      confidence: 0.75,
      icon: '🤔',
    }
  );
  
  // 根据置信度排序并返回
  return suggestions
    .sort((a, b) => b.confidence - a.confidence)
    .slice(0, 5);
};

// 快捷操作定义
const QUICK_ACTIONS: QuickAction[] = [
  {
    id: 'code-review',
    label: '代码审查',
    icon: '🔍',
    action: () => {},
    shortcut: 'Ctrl+R',
    category: 'code',
  },
  {
    id: 'doc-generate',
    label: '生成文档',
    icon: '📄',
    action: () => {},
    shortcut: 'Ctrl+D',
    category: 'code',
  },
  {
    id: 'refactor',
    label: '重构代码',
    icon: '🔧',
    action: () => {},
    shortcut: 'Ctrl+F',
    category: 'code',
  },
  {
    id: 'data-analysis',
    label: '数据分析',
    icon: '📈',
    action: () => {},
    category: 'analysis',
  },
  {
    id: 'summarize',
    label: '总结要点',
    icon: '📝',
    action: () => {},
    shortcut: 'Ctrl+S',
    category: 'analysis',
  },
  {
    id: 'brainstorm',
    label: '头脑风暴',
    icon: '💡',
    action: () => {},
    category: 'creative',
  },
  {
    id: 'translate',
    label: '翻译',
    icon: '🌐',
    action: () => {},
    shortcut: 'Ctrl+L',
    category: 'utility',
  },
  {
    id: 'explain',
    label: '简单解释',
    icon: '🎯',
    action: () => {},
    category: 'utility',
  },
];

export const SmartSuggestions: React.FC<SmartSuggestionsProps> = ({
  context,
  lastResponse,
  conversationHistory,
  onSuggestionClick,
  onQuickActionClick,
  maxSuggestions = 4,
  position = 'above',
  visible = true,
}) => {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [activeCategory, setActiveCategory] = useState<string>('all');
  const [isExpanded, setIsExpanded] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const containerRef = useRef<HTMLDivElement>(null);

  // 生成建议
  useEffect(() => {
    if (!visible || !context) {
      setSuggestions([]);
      return;
    }

    const newSuggestions = generateSuggestions(context, lastResponse, conversationHistory);
    setSuggestions(newSuggestions.slice(0, maxSuggestions));
  }, [context, lastResponse, conversationHistory, maxSuggestions, visible]);

  // 键盘导航
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!visible || suggestions.length === 0) return;

      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          setSelectedIndex(prev => 
            prev < suggestions.length - 1 ? prev + 1 : prev
          );
          break;
        case 'ArrowUp':
          e.preventDefault();
          setSelectedIndex(prev => prev > 0 ? prev - 1 : -1);
          break;
        case 'Enter':
          if (selectedIndex >= 0 && selectedIndex < suggestions.length) {
            onSuggestionClick(suggestions[selectedIndex]);
          }
          break;
        case 'Escape':
          setSelectedIndex(-1);
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [suggestions, selectedIndex, onSuggestionClick, visible]);

  // 获取建议类型样式
  const getSuggestionStyle = (type: SuggestionType) => {
    const styles: Record<SuggestionType, { bg: string; border: string; color: string }> = {
      'follow_up': { 
        bg: 'rgba(59, 130, 246, 0.1)', 
        border: 'rgba(59, 130, 246, 0.3)',
        color: '#60a5fa'
      },
      'clarification': { 
        bg: 'rgba(16, 185, 129, 0.1)', 
        border: 'rgba(16, 185, 129, 0.3)',
        color: '#34d399'
      },
      'action': { 
        bg: 'rgba(245, 158, 11, 0.1)', 
        border: 'rgba(245, 158, 11, 0.3)',
        color: '#fbbf24'
      },
      'exploration': { 
        bg: 'rgba(139, 92, 246, 0.1)', 
        border: 'rgba(139, 92, 246, 0.3)',
        color: '#a78bfa'
      },
      'correction': {
        bg: 'rgba(239, 68, 68, 0.1)',
        border: 'rgba(239, 68, 68, 0.3)',
        color: '#f87171'
      },
    };
    return styles[type];
  };

  // 获取类型标签
  const getTypeLabel = (type: SuggestionType) => {
    const labels: Record<SuggestionType, string> = {
      'follow_up': '跟进',
      'clarification': '澄清',
      'action': '行动',
      'exploration': '探索',
      'correction': '纠正',
    };
    return labels[type];
  };

  // 过滤快捷操作
  const filteredActions = activeCategory === 'all' 
    ? QUICK_ACTIONS 
    : QUICK_ACTIONS.filter(a => a.category === activeCategory);

  if (!visible) return null;

  return (
    <div 
      ref={containerRef}
      className={`smart-suggestions ${position} ${isExpanded ? 'expanded' : ''}`}
    >
      {/* 快捷操作栏 */}
      <div className="quick-actions-bar">
        <div className="category-tabs">
          <button
            className={`category-tab ${activeCategory === 'all' ? 'active' : ''}`}
            onClick={() => setActiveCategory('all')}
          >
            全部
          </button>
          <button
            className={`category-tab ${activeCategory === 'code' ? 'active' : ''}`}
            onClick={() => setActiveCategory('code')}
          >
            💻 代码
          </button>
          <button
            className={`category-tab ${activeCategory === 'analysis' ? 'active' : ''}`}
            onClick={() => setActiveCategory('analysis')}
          >
            📊 分析
          </button>
          <button
            className={`category-tab ${activeCategory === 'creative' ? 'active' : ''}`}
            onClick={() => setActiveCategory('creative')}
          >
            ✨ 创意
          </button>
          <button
            className={`category-tab ${activeCategory === 'utility' ? 'active' : ''}`}
            onClick={() => setActiveCategory('utility')}
          >
            🛠️ 工具
          </button>
        </div>
        
        <div className={`quick-actions-grid ${isExpanded ? 'expanded' : ''}`}>
          {filteredActions.map(action => (
            <button
              key={action.id}
              className="quick-action-btn"
              onClick={() => onQuickActionClick(action)}
              title={action.shortcut ? `${action.label} (${action.shortcut})` : action.label}
            >
              <span className="action-icon">{action.icon}</span>
              <span className="action-label">{action.label}</span>
              {action.shortcut && (
                <span className="action-shortcut">{action.shortcut}</span>
              )}
            </button>
          ))}
        </div>
        
        <button 
          className="expand-toggle"
          onClick={() => setIsExpanded(!isExpanded)}
        >
          {isExpanded ? '收起 ▲' : '更多 ▼'}
        </button>
      </div>

      {/* 智能建议列表 */}
      {suggestions.length > 0 && (
        <div className="suggestions-list">
          <div className="suggestions-header">
            <span className="suggestions-title">💡 智能建议</span>
            <span className="suggestions-count">{suggestions.length}</span>
          </div>
          
          <div className="suggestions-items">
            {suggestions.map((suggestion, index) => {
              const style = getSuggestionStyle(suggestion.type);
              const isSelected = index === selectedIndex;
              
              return (
                <button
                  key={suggestion.id}
                  className={`suggestion-item ${isSelected ? 'selected' : ''}`}
                  onClick={() => onSuggestionClick(suggestion)}
                  style={{
                    background: style.bg,
                    borderColor: isSelected ? style.color : style.border,
                  }}
                >
                  <span className="suggestion-icon">{suggestion.icon}</span>
                  <span className="suggestion-text">{suggestion.text}</span>
                  <span 
                    className="suggestion-type-badge"
                    style={{ background: style.color }}
                  >
                    {getTypeLabel(suggestion.type)}
                  </span>
                  
                  {/* 置信度指示器 */}
                  <div className="confidence-indicator">
                    <div 
                      className="confidence-bar"
                      style={{ 
                        width: `${suggestion.confidence * 100}%`,
                        background: suggestion.confidence > 0.9 ? '#22c55e' :
                                   suggestion.confidence > 0.7 ? '#eab308' : '#64748b'
                      }}
                    />
                  </div>
                  
                  {suggestion.shortcut && (
                    <span className="suggestion-shortcut">{suggestion.shortcut}</span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      )}

      {/* 上下文提示 */}
      {context && (
        <div className="context-hint">
          <span className="hint-icon">📝</span>
          <span className="hint-text">
            基于"{context.slice(0, 30)}{context.length > 30 ? '...' : ''}"的上下文
          </span>
        </div>
      )}
    </div>
  );
};

export default SmartSuggestions;
