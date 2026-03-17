import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { 
  Code, FileText, Wrench, BarChart3, Lightbulb, Globe, 
  Target, Sparkles, MessageSquare, Zap, Search, GitBranch,
  FileCode, BookOpen, Terminal, PieChart, Brain
} from 'lucide-react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// 建议类型
export type SuggestionType = 'follow_up' | 'clarification' | 'action' | 'exploration' | 'correction';

// 建议项
export interface Suggestion {
  id: string;
  text: string;
  type: SuggestionType;
  confidence: number;
  icon?: React.ReactNode;
  action?: string; // 实际要执行的动作/提示词
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
  icon: React.ReactNode;
  prompt: string; // 实际发送给模型的提示词
  shortcut?: string;
  category: 'code' | 'analysis' | 'creative' | 'utility';
}

interface ActionableSmartSuggestionsProps {
  // 对话上下文
  lastUserMessage?: string;
  lastAssistantMessage?: string;
  conversationHistory: Array<{ role: string; content: string }>;
  // 回调函数
  onSendMessage: (message: string) => void; // 实际发送消息的函数
  // 可选配置
  maxSuggestions?: number;
  visible?: boolean;
  className?: string;
}

// 快捷操作定义 - 绑定实际功能
const QUICK_ACTIONS: QuickAction[] = [
  {
    id: 'code-review',
    label: '代码审查',
    icon: <Code className="w-5 h-5" />,
    prompt: '请对以上代码进行详细审查，包括：1) 代码质量和可读性 2) 潜在bug和安全问题 3) 性能优化建议 4) 最佳实践遵循情况。请逐行分析并给出具体改进建议。',
    shortcut: 'Ctrl+R',
    category: 'code',
  },
  {
    id: 'doc-generate',
    label: '生成文档',
    icon: <FileText className="w-5 h-5" />,
    prompt: '请为以上代码/内容生成完整的技术文档，包括：功能描述、参数说明、返回值、使用示例、注意事项。使用 Markdown 格式。',
    shortcut: 'Ctrl+D',
    category: 'code',
  },
  {
    id: 'refactor',
    label: '重构代码',
    icon: <Wrench className="w-5 h-5" />,
    prompt: '请对以上代码进行重构，要求：1) 提高可读性和可维护性 2) 遵循设计模式 3) 减少重复代码 4) 优化命名。请给出重构后的完整代码并解释改动原因。',
    shortcut: 'Ctrl+F',
    category: 'code',
  },
  {
    id: 'explain-code',
    label: '解释代码',
    icon: <BookOpen className="w-5 h-5" />,
    prompt: '请详细解释这段代码的工作原理，包括：1) 整体逻辑流程 2) 关键函数和类的作用 3) 数据流向 4) 核心算法说明。用通俗易懂的方式讲解。',
    category: 'code',
  },
  {
    id: 'data-analysis',
    label: '数据分析',
    icon: <BarChart3 className="w-5 h-5" />,
    prompt: '请对以上数据进行深入分析，包括：1) 数据特征描述 2) 趋势和模式识别 3) 异常值检测 4) 可视化建议 5) 业务洞察。',
    category: 'analysis',
  },
  {
    id: 'summarize',
    label: '总结要点',
    icon: <Target className="w-5 h-5" />,
    prompt: '请总结以上内容的关键要点，用 bullet points 列出核心观点，并给出执行建议。',
    shortcut: 'Ctrl+S',
    category: 'analysis',
  },
  {
    id: 'brainstorm',
    label: '头脑风暴',
    icon: <Lightbulb className="w-5 h-5" />,
    prompt: '请针对以上主题进行头脑风暴，提供多种不同角度的解决方案和创新思路，包括常规方法和突破性想法。',
    category: 'creative',
  },
  {
    id: 'translate',
    label: '翻译',
    icon: <Globe className="w-5 h-5" />,
    prompt: '请将以上内容翻译成中文（如果是中文则翻译成英文），保持专业术语准确，语言流畅自然。',
    shortcut: 'Ctrl+L',
    category: 'utility',
  },
  {
    id: 'simplify',
    label: '简单解释',
    icon: <Sparkles className="w-5 h-5" />,
    prompt: '请用简单易懂的方式解释以上内容，假设听众是非技术背景的初学者，避免使用专业术语，多用类比和例子。',
    category: 'utility',
  },
  {
    id: 'test-cases',
    label: '生成测试',
    icon: <Terminal className="w-5 h-5" />,
    prompt: '请为以上代码生成完整的单元测试用例，包括：正常情况、边界条件、异常情况。使用主流测试框架。',
    category: 'code',
  },
  {
    id: 'performance',
    label: '性能分析',
    icon: <Zap className="w-5 h-5" />,
    prompt: '请分析以上代码的性能瓶颈，包括：时间复杂度、空间复杂度、潜在的性能问题，并给出优化方案。',
    category: 'analysis',
  },
  {
    id: 'alternatives',
    label: '替代方案',
    icon: <GitBranch className="w-5 h-5" />,
    prompt: '请提供实现相同功能的其他方法或替代方案，比较各自的优缺点和适用场景。',
    category: 'creative',
  },
];

// 基于上下文的智能建议生成
const generateContextualSuggestions = (
  lastUserMessage: string = '',
  lastAssistantMessage: string = '',
  history: Array<{ role: string; content: string }> = []
): Suggestion[] => {
  const suggestions: Suggestion[] = [];
  const userLower = lastUserMessage.toLowerCase();
  const assistantLower = lastAssistantMessage.toLowerCase();
  
  // 代码相关上下文 - 更严格的检测
  // 必须包含代码块或明显的代码模式
  const hasCodeBlock = /```[\s\S]*?```/.test(lastAssistantMessage);
  const hasCodePatterns = /(function|class|const|let|var|def|import|from|return|if\s*\(|for\s*\(|while\s*\()[\s\S]{10,}/.test(lastAssistantMessage);
  const isCodeDiscussion = /代码|函数|类|bug|error|报错|修复|重构|优化.*代码/.test(userLower) && 
                           (userLower.includes('代码') || userLower.includes('函数') || userLower.includes('bug'));
  const hasCode = hasCodeBlock || hasCodePatterns || isCodeDiscussion;
  
  // 数据分析上下文 - 更严格的检测
  const hasData = /(\d+[,\.]?\d*\s*[%个条件])|(\d{4}[-/年])|(csv|json|xml|excel|表格|图表|趋势|统计)/.test(lastAssistantMessage) &&
                  /数据|统计|分析|趋势|dataset/.test(userLower + assistantLower);
  
  // 长文本上下文
  const isLongContent = lastAssistantMessage.length > 800;
  
  // 问题/疑问上下文
  const hasQuestion = /\?|？|如何|怎么|为什么|什么是|怎样|能否/.test(userLower);
  
  // 问候/闲聊上下文
  const isGreeting = /^(你好|您好|嗨|hello|hi|hey|早上好|下午好|晚上好|再见|拜拜|谢谢|感谢)/.test(userLower.trim());
  const isSmallTalk = /(今天|天气|怎么样|在吗|在嘛|忙吗|好吗)/.test(userLower) && userLower.length < 20;
  
  // 如果是问候或闲聊，提供相关的跟进建议
  if (isGreeting || isSmallTalk) {
    suggestions.push(
      {
        id: 'what-can-you-do',
        text: '你能帮我做什么？',
        type: 'follow_up',
        confidence: 0.90,
        icon: <Sparkles className="w-4 h-4" />,
        action: '请介绍一下你能帮我做哪些事情，比如代码审查、数据分析、文档生成等',
      },
      {
        id: 'start-coding',
        text: '帮我写一段代码',
        type: 'action',
        confidence: 0.85,
        icon: <Code className="w-4 h-4" />,
        action: '我需要写一段代码，请帮我实现：',
      }
    );
    
    // 问候场景下不添加其他类型的建议
    return suggestions.slice(0, 4);
  }
  
  // 根据上下文生成相关建议
  if (hasCode) {
    suggestions.push(
      {
        id: 'explain-code-detail',
        text: '详细解释这段代码的工作原理',
        type: 'clarification',
        confidence: 0.92,
        icon: <BookOpen className="w-4 h-4" />,
        action: '请逐行解释这段代码，说明每一行的作用和目的',
      },
      {
        id: 'optimize-code',
        text: '优化这段代码的性能',
        type: 'action',
        confidence: 0.88,
        icon: <Zap className="w-4 h-4" />,
        action: '请分析这段代码的性能瓶颈并提供优化建议',
      },
      {
        id: 'find-bugs',
        text: '找出潜在的bug',
        type: 'correction',
        confidence: 0.85,
        icon: <Target className="w-4 h-4" />,
        action: '请仔细检查这段代码，找出潜在的bug和边界情况问题',
      }
    );
  }
  
  if (hasData) {
    suggestions.push(
      {
        id: 'visualize-data',
        text: '生成数据可视化图表',
        type: 'action',
        confidence: 0.90,
        icon: <PieChart className="w-4 h-4" />,
        action: '请为这些数据生成可视化图表建议，可以使用Mermaid语法绘制',
      },
      {
        id: 'deep-analysis',
        text: '深入分析数据趋势',
        type: 'exploration',
        confidence: 0.87,
        icon: <BarChart3 className="w-4 h-4" />,
        action: '请深入分析数据背后的趋势和模式，给出业务洞察',
      }
    );
  }
  
  if (isLongContent) {
    suggestions.push(
      {
        id: 'summarize-key',
        text: '总结关键要点',
        type: 'follow_up',
        confidence: 0.89,
        icon: <Target className="w-4 h-4" />,
        action: '请总结以上内容的关键要点',
      },
      {
        id: 'action-items',
        text: '提取行动项',
        type: 'action',
        confidence: 0.82,
        icon: <MessageSquare className="w-4 h-4" />,
        action: '请从以上内容中提取具体的行动项和待办事项',
      }
    );
  }
  
  if (hasQuestion) {
    suggestions.push(
      {
        id: 'examples',
        text: '提供具体示例',
        type: 'follow_up',
        confidence: 0.86,
        icon: <FileCode className="w-4 h-4" />,
        action: '请提供更多具体的例子来说明',
      },
      {
        id: 'related-topics',
        text: '相关的其他知识点',
        type: 'exploration',
        confidence: 0.80,
        icon: <Brain className="w-4 h-4" />,
        action: '请介绍与这个话题相关的其他重要知识点',
      }
    );
  }
  
  // 通用建议（始终添加）
  suggestions.push(
    {
      id: 'elaborate',
      text: '详细说明这一点',
      type: 'follow_up',
      confidence: 0.78,
      icon: <Sparkles className="w-4 h-4" />,
      action: '请更详细地说明这一点',
    },
    {
      id: 'alternatives-generic',
      text: '还有其他方法吗？',
      type: 'exploration',
      confidence: 0.75,
      icon: <GitBranch className="w-4 h-4" />,
      action: '请提供其他替代方法或思路',
    }
  );
  
  // 根据置信度排序并返回
  return suggestions
    .sort((a, b) => b.confidence - a.confidence)
    .slice(0, 4);
};

export const ActionableSmartSuggestions: React.FC<ActionableSmartSuggestionsProps> = ({
  lastUserMessage = '',
  lastAssistantMessage = '',
  conversationHistory,
  onSendMessage,
  maxSuggestions = 4,
  visible = true,
  className,
}) => {
  const [activeCategory, setActiveCategory] = useState<string>('all');
  const [isExpanded, setIsExpanded] = useState(false);
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);

  // 生成基于上下文的建议
  useEffect(() => {
    if (!visible) {
      setSuggestions([]);
      return;
    }
    
    const newSuggestions = generateContextualSuggestions(
      lastUserMessage,
      lastAssistantMessage,
      conversationHistory
    );
    setSuggestions(newSuggestions.slice(0, maxSuggestions));
  }, [lastUserMessage, lastAssistantMessage, conversationHistory, maxSuggestions, visible]);

  // 过滤快捷操作
  const filteredActions = useMemo(() => {
    return activeCategory === 'all' 
      ? QUICK_ACTIONS 
      : QUICK_ACTIONS.filter(a => a.category === activeCategory);
  }, [activeCategory]);

  // 处理快捷操作点击
  const handleQuickAction = useCallback((action: QuickAction) => {
    onSendMessage(action.prompt);
  }, [onSendMessage]);

  // 处理建议点击
  const handleSuggestionClick = useCallback((suggestion: Suggestion) => {
    if (suggestion.action) {
      onSendMessage(suggestion.action);
    }
  }, [onSendMessage]);

  // 获取建议类型样式
  const getSuggestionStyle = (type: SuggestionType) => {
    const styles: Record<SuggestionType, { bg: string; border: string; color: string; label: string }> = {
      'follow_up': { 
        bg: 'bg-blue-500/10', 
        border: 'border-blue-500/30',
        color: 'text-blue-400',
        label: '跟进'
      },
      'clarification': { 
        bg: 'bg-emerald-500/10', 
        border: 'border-emerald-500/30',
        color: 'text-emerald-400',
        label: '澄清'
      },
      'action': { 
        bg: 'bg-amber-500/10', 
        border: 'border-amber-500/30',
        color: 'text-amber-400',
        label: '行动'
      },
      'exploration': { 
        bg: 'bg-purple-500/10', 
        border: 'border-purple-500/30',
        color: 'text-purple-400',
        label: '探索'
      },
      'correction': {
        bg: 'bg-rose-500/10',
        border: 'border-rose-500/30',
        color: 'text-rose-400',
        label: '纠正'
      },
    };
    return styles[type];
  };

  if (!visible) return null;

  // 根据上下文确定最相关的类别
  const userLower = lastUserMessage.toLowerCase();
  const assistantLower = lastAssistantMessage.toLowerCase();
  const hasCodeContext = /```[\s\S]*?```/.test(lastAssistantMessage) || 
                         /(function|class|const|let|var|def|import)[\s\S]{10,}/.test(lastAssistantMessage);
  const hasDataContext = /(\d+[,\.]?\d*\s*[%个条件])|(\d{4}[-/年])/.test(lastAssistantMessage);
  const isGreetingContext = /^(你好|您好|嗨|hello|hi|hey)/.test(userLower.trim());
  
  // 确定默认类别和相关操作
  const getRelevantCategories = () => {
    if (hasCodeContext) return ['code', 'analysis'];
    if (hasDataContext) return ['analysis', 'creative'];
    if (isGreetingContext) return ['utility'];
    return ['all'];
  };
  
  const relevantCategories = getRelevantCategories();
  const showQuickActions = !isGreetingContext && (hasCodeContext || hasDataContext || lastAssistantMessage.length > 200);

  return (
    <div className={cn(
      "rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white/50 dark:bg-zinc-900/50 overflow-hidden",
      className
    )}>
      {/* 快捷操作栏 - 仅在相关上下文中显示 */}
      {showQuickActions && (
        <div className="p-3 border-b border-zinc-200 dark:border-zinc-800">
          {/* 分类标签 - 仅显示相关类别 */}
          <div className="flex items-center gap-1 mb-3 overflow-x-auto">
            <button
              className={cn(
                "px-3 py-1.5 rounded-lg text-xs font-medium transition-colors whitespace-nowrap",
                activeCategory === 'all' 
                  ? "bg-blue-500/20 text-blue-400" 
                  : "text-zinc-500 hover:bg-zinc-100 dark:hover:bg-zinc-800"
              )}
              onClick={() => setActiveCategory('all')}
            >
              全部
            </button>
            {hasCodeContext && (
              <button
                className={cn(
                  "px-3 py-1.5 rounded-lg text-xs font-medium transition-colors whitespace-nowrap flex items-center gap-1",
                  activeCategory === 'code' 
                    ? "bg-emerald-500/20 text-emerald-400" 
                    : "text-zinc-500 hover:bg-zinc-100 dark:hover:bg-zinc-800"
                )}
                onClick={() => setActiveCategory('code')}
              >
                <Code className="w-3 h-3" />
                代码
              </button>
            )}
            {hasDataContext && (
              <button
                className={cn(
                  "px-3 py-1.5 rounded-lg text-xs font-medium transition-colors whitespace-nowrap flex items-center gap-1",
                  activeCategory === 'analysis' 
                    ? "bg-amber-500/20 text-amber-400" 
                    : "text-zinc-500 hover:bg-zinc-100 dark:hover:bg-zinc-800"
                )}
                onClick={() => setActiveCategory('analysis')}
              >
                <BarChart3 className="w-3 h-3" />
                分析
              </button>
            )}
            <button
              className={cn(
                "px-3 py-1.5 rounded-lg text-xs font-medium transition-colors whitespace-nowrap flex items-center gap-1",
                activeCategory === 'creative' 
                  ? "bg-purple-500/20 text-purple-400" 
                  : "text-zinc-500 hover:bg-zinc-100 dark:hover:bg-zinc-800"
              )}
              onClick={() => setActiveCategory('creative')}
            >
              <Lightbulb className="w-3 h-3" />
              创意
            </button>
            <button
              className={cn(
                "px-3 py-1.5 rounded-lg text-xs font-medium transition-colors whitespace-nowrap flex items-center gap-1",
                activeCategory === 'utility' 
                  ? "bg-cyan-500/20 text-cyan-400" 
                  : "text-zinc-500 hover:bg-zinc-100 dark:hover:bg-zinc-800"
              )}
              onClick={() => setActiveCategory('utility')}
            >
              <Sparkles className="w-3 h-3" />
              工具
            </button>
          </div>
          
          {/* 快捷操作网格 */}
          <div className={cn(
            "grid grid-cols-4 gap-2 transition-all duration-300",
            isExpanded ? 'max-h-[200px]' : 'max-h-[80px] overflow-hidden'
          )}>
            {filteredActions.map(action => (
              <button
                key={action.id}
                className={cn(
                  "flex flex-col items-center gap-1 p-2 rounded-lg transition-all",
                  "bg-zinc-100 dark:bg-zinc-800 hover:bg-zinc-200 dark:hover:bg-zinc-700",
                  "text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-200"
                )}
                onClick={() => handleQuickAction(action)}
                title={action.shortcut ? `${action.label} (${action.shortcut})` : action.label}
              >
                <span className="text-zinc-500 dark:text-zinc-400">{action.icon}</span>
                <span className="text-[10px] font-medium truncate w-full text-center">{action.label}</span>
              </button>
            ))}
          </div>
          
          {/* 展开/收起按钮 */}
          {filteredActions.length > 4 && (
            <button 
              className="w-full mt-2 py-1 text-[10px] text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300 transition-colors"
              onClick={() => setIsExpanded(!isExpanded)}
            >
              {isExpanded ? '收起 ▲' : '更多 ▼'}
            </button>
          )}
        </div>
      )}

      {/* 智能建议列表 */}
      {suggestions.length > 0 && (
        <div className="p-3">
          <div className="flex items-center gap-2 mb-3">
            <Sparkles className="w-4 h-4 text-amber-400" />
            <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300">智能建议</span>
            <span className="px-1.5 py-0.5 bg-zinc-200 dark:bg-zinc-700 rounded text-[10px] text-zinc-600 dark:text-zinc-400">
              {suggestions.length}
            </span>
          </div>
          
          <div className="space-y-2">
            {suggestions.map((suggestion) => {
              const style = getSuggestionStyle(suggestion.type);
              
              return (
                <button
                  key={suggestion.id}
                  className={cn(
                    "w-full flex items-center gap-3 p-3 rounded-lg border transition-all text-left",
                    "hover:scale-[1.02] active:scale-[0.98]",
                    style.bg,
                    style.border,
                    style.color
                  )}
                  onClick={() => handleSuggestionClick(suggestion)}
                >
                  <span className="flex-shrink-0">{suggestion.icon}</span>
                  <span className="flex-1 text-sm font-medium">{suggestion.text}</span>
                  <span className={cn(
                    "px-2 py-0.5 rounded text-[10px] font-medium",
                    "bg-white/20 dark:bg-black/20"
                  )}>
                    {style.label}
                  </span>
                </button>
              );
            })}
          </div>
        </div>
      )}

      {/* 上下文提示 */}
      {(lastUserMessage || lastAssistantMessage) && (
        <div className="px-3 py-2 border-t border-zinc-200 dark:border-zinc-800">
          <div className="flex items-center gap-2 text-[10px] text-zinc-500">
            <Search className="w-3 h-3" />
            <span className="truncate">
              基于对话上下文生成的建议
            </span>
          </div>
        </div>
      )}
    </div>
  );
};

export default ActionableSmartSuggestions;
