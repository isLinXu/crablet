import React, { useMemo } from 'react';
import ReactMarkdown from 'react-markdown';
import type { Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { cn } from '../ui/cn';
import type { ContentPart } from '@/types/domain';
import type { ExtendedMessage } from '@/store/chatStore';
import { CodeBlock } from './CodeBlock';
import { Bot, User, Copy, Check, Download, Pencil, Trash2, X } from 'lucide-react';
import { cognitiveLayerLabel } from '@/utils/cognitive';
import { EnhancedThinkingVisualization, type ThinkingProcess } from './EnhancedThinkingVisualization';
import { CrabThinking } from '../ui/CrabElements';
import { sanitizeUrl } from '@/utils/security';

interface MessageBubbleProps {
  message: ExtendedMessage;
  onEdit?: (id: string, newContent: string) => void;
  onDelete?: (id: string) => void;
  thinkingProcess?: ThinkingProcess;
  isThinking?: boolean;
  onSendMessage?: (message: string) => void; // 新增：发送消息回调
  conversationHistory?: Array<{ role: string; content: string }>; // 新增：对话历史
  lastUserMessage?: string; // 新增：最后一条用户消息
}

const STEP_LABELS: Record<string, string> = {
  reasoning: '推理思考',
  search: '知识检索',
  code: '代码分析',
  insight: '洞察发现',
};

const getTimestampMs = (timestamp?: string) => {
  if (!timestamp) return 0;
  const parsed = new Date(timestamp).getTime();
  return Number.isFinite(parsed) ? parsed : 0;
};

type MarkdownCodeProps = React.ComponentPropsWithoutRef<'code'> & { inline?: boolean };
type MarkdownLinkProps = React.ComponentPropsWithoutRef<'a'>;
type MarkdownImageProps = React.ComponentPropsWithoutRef<'img'>;
type MarkdownDivProps = React.ComponentPropsWithoutRef<'div'>;

// 使用 React.memo 避免不必要的重渲染
export const MessageBubble: React.FC<MessageBubbleProps> = React.memo(({ 
  message, 
  onEdit, 
  onDelete, 
  thinkingProcess, 
  isThinking,
  onSendMessage,
  conversationHistory = [],
  lastUserMessage = '',
}) => {
  const isUser = message.role === 'user';
  const [copied, setCopied] = React.useState(false);
  const [isEditing, setIsEditing] = React.useState(false);
  const [editValue, setEditValue] = React.useState('');
  const messageTimestampMs = useMemo(() => getTimestampMs(message.timestamp), [message.timestamp]);
  const fileTimestamp = messageTimestampMs || Number.parseInt((message.id || '0').replace(/\D/g, ''), 10) || 0;

  const handleCopy = () => {
    if (typeof message.content === 'string') {
        navigator.clipboard.writeText(message.content);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    }
  };

  const downloadImage = async (url: string, name: string) => {
    try {
      if (url.startsWith('data:image/')) {
        const a = document.createElement('a');
        a.href = url;
        a.download = name;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        return;
      }
      const res = await fetch(url);
      const blob = await res.blob();
      const objectUrl = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = objectUrl;
      a.download = name;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(objectUrl);
    } catch {
      window.open(url, '_blank', 'noopener,noreferrer');
    }
  };

  const downloadAllImages = async (urls: string[]) => {
    for (let i = 0; i < urls.length; i++) {
      await downloadImage(urls[i], `crablet-image-${fileTimestamp}-${i + 1}.png`);
      await new Promise((resolve) => setTimeout(resolve, 120));
    }
  };
  
  const handleSaveEdit = () => {
    if (message.id && onEdit && editValue.trim() !== '') {
        onEdit(message.id, editValue);
        setIsEditing(false);
    }
  };

  const startEditing = () => {
      if (typeof message.content === 'string') {
          setEditValue(message.content);
          setIsEditing(true);
      }
  };

  // 使用 useMemo 缓存 Markdown 渲染结果，避免流式输出时的频繁重渲染
  const renderedContent = (() => {
    const markdownComponents = {
      code({ inline, className, children, ...props }: MarkdownCodeProps) {
        const match = /language-(\w+)/.exec(className || '');
        return !inline && match ? (
          <CodeBlock language={match[1]} value={String(children).replace(/\n$/, '')} {...props} />
        ) : (
          <code className={cn(className, 'bg-gray-200 dark:bg-gray-800 rounded px-1 py-0.5 text-sm font-mono')} {...props}>
            {children}
          </code>
        );
      },
      a({ href, children, ...props }: MarkdownLinkProps) {
        const safeHref = sanitizeUrl(href || '');
        if (!safeHref) {
          return <span className="text-gray-400">{children}</span>;
        }
        return (
          <a
            href={safeHref}
            target="_blank"
            rel="noopener noreferrer"
            className="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300 underline"
            {...props}
          >
            {children}
          </a>
        );
      },
    } satisfies Components;

    const richMarkdownComponents = {
      ...markdownComponents,
      img({ src, alt, ...props }: MarkdownImageProps) {
        const safeSrc = sanitizeUrl(src || '');
        if (!safeSrc) {
          return null;
        }
        return (
          <img
            src={safeSrc}
            alt={alt || 'Image'}
            className="max-w-full h-auto rounded-lg"
            loading="lazy"
            {...props}
          />
        );
      },
      div({ children, ...props }: MarkdownDivProps) {
        return <div {...props}>{children}</div>;
      },
    } satisfies Components;

    if (isEditing) {
        return (
            <div className="flex flex-col gap-2 w-full">
                <textarea
                    value={editValue}
                    onChange={(e) => setEditValue(e.target.value)}
                    className="w-full bg-white/10 text-inherit border border-white/20 rounded p-2 min-h-[100px] focus:outline-none focus:ring-1 focus:ring-white/30 resize-y"
                />
                <div className="flex justify-end gap-2">
                    <button onClick={() => setIsEditing(false)} className="p-1 hover:bg-white/10 rounded" title="Cancel">
                        <X className="w-4 h-4" />
                    </button>
                    <button onClick={handleSaveEdit} className="p-1 hover:bg-white/10 rounded" title="Save">
                        <Check className="w-4 h-4" />
                    </button>
                </div>
            </div>
        );
    }

    if (typeof message.content === 'string') {
      return (
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          components={richMarkdownComponents}
        >
          {message.content}
        </ReactMarkdown>
      );
    }
    
    const contentParts = message.content as ContentPart[];
    const textParts = contentParts.filter((part): part is Extract<ContentPart, { type: 'text' }> => part.type === 'text');
    const imageParts = contentParts.filter((part): part is Extract<ContentPart, { type: 'image_url' }> => part.type === 'image_url');
    // Sanitize all image URLs
    const imageUrls = imageParts.map((part) => part.image_url.url).filter(Boolean).map(sanitizeUrl).filter(Boolean);
    return (
      <>
        {textParts.map((part, index) => (
          <ReactMarkdown 
            key={`text-${index}`}
            remarkPlugins={[remarkGfm]}
            components={markdownComponents}
          >
            {part.text}
          </ReactMarkdown>
        ))}
        {imageUrls.length > 0 && (
          <div className="mt-2 space-y-2">
            {imageUrls.length > 1 && (
              <div className="flex justify-end">
                <button
                  onClick={() => downloadAllImages(imageUrls)}
                  className="inline-flex items-center gap-1 text-xs px-2 py-1 rounded-md border border-zinc-300 dark:border-zinc-600 hover:bg-zinc-100 dark:hover:bg-zinc-800"
                >
                  <Download className="w-3.5 h-3.5" />
                  下载全部
                </button>
              </div>
            )}
            <div className={cn("grid gap-2", imageUrls.length === 1 ? "grid-cols-1 max-w-sm" : "grid-cols-2")}>
              {imageUrls.map((url, index) => {
                return (
                  <div key={`img-${index}`} className="relative group/image">
                    <img src={url} alt={`Generated ${index + 1}`} className="w-full rounded-lg shadow-sm border border-gray-200 dark:border-gray-700 object-cover" loading="lazy" />
                    <button
                      onClick={() => downloadImage(url, `crablet-image-${fileTimestamp}-${index + 1}.png`)}
                      className="absolute top-2 right-2 opacity-0 group-hover/image:opacity-100 transition-opacity inline-flex items-center gap-1 text-xs px-2 py-1 rounded-md border border-zinc-300 dark:border-zinc-600 bg-white/90 dark:bg-zinc-900/90 hover:bg-white dark:hover:bg-zinc-900"
                    >
                      <Download className="w-3.5 h-3.5" />
                      下载
                    </button>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </>
    );
  })();

  const knowledgeJump = (source: string, snippet?: string) =>
    `/knowledge?q=${encodeURIComponent(source)}&source=${encodeURIComponent(source)}${snippet ? `&snippet=${encodeURIComponent(snippet.slice(0, 160))}` : ''}`;

  // 步骤类型标签映射
  // 从消息中构建思考过程
  const messageThinkingProcess: ThinkingProcess | undefined = React.useMemo(() => {
    if (isUser) return undefined;
    
    // 如果有外部传入的思考过程（当前正在思考的消息），使用外部过程
    if (thinkingProcess) {
      return thinkingProcess;
    }
    
    // 否则从消息的 traceSteps 构建
    const traceSteps = message.traceSteps;
    if (traceSteps && traceSteps.length > 0) {
      const steps: ThinkingProcess['steps'] = [];
      
      // 1. 添加意图识别步骤（基于第一条 trace）
      const firstTrace = traceSteps[0];
      const firstText = (firstTrace.thought + ' ' + firstTrace.action).toLowerCase();
      
      // 检测意图类型
      let intentType = 'General';
      let intentDescription = '通用查询';
      
      if (firstText.includes('greeting') || firstText.includes('你好') || firstText.includes('hello') || firstText.includes('hi')) {
        intentType = 'Greeting';
        intentDescription = '问候/打招呼';
      } else if (firstText.includes('code') || firstText.includes('代码') || firstText.includes('function') || firstText.includes('programming')) {
        intentType = 'Coding';
        intentDescription = '代码相关';
      } else if (firstText.includes('search') || firstText.includes('检索') || firstText.includes('查找') || firstText.includes('query')) {
        intentType = 'Search';
        intentDescription = '知识检索';
      } else if (firstText.includes('analyze') || firstText.includes('分析') || firstText.includes('compare')) {
        intentType = 'Analysis';
        intentDescription = '数据分析';
      } else if (firstText.includes('help') || firstText.includes('帮助')) {
        intentType = 'Help';
        intentDescription = '寻求帮助';
      }
      
      steps.push({
        id: 'intent-recognition',
        type: 'decision' as any,
        title: '意图识别',
        content: `识别用户意图: ${intentDescription} (${intentType})`,
        timestamp: messageTimestampMs - traceSteps.length * 500 - 200,
        duration: 50,
        details: {
          reason: `${intentDescription} (${intentType})`,
          confidenceScore: 0.95,
        },
      });
      
      // 2. 添加系统选择步骤
      const currentLayer = message.cognitiveLayer || 'unknown';
      const layerNames: Record<string, string> = {
        'system1': 'System 1 (快速直觉)',
        'system2': 'System 2 (深度分析)',
        'system3': 'System 3 (元认知)',
        'unknown': '自动选择',
      };
      
      steps.push({
        id: 'system-selection',
        type: 'system',
        title: '系统选择',
        content: `路由到: ${layerNames[currentLayer] || currentLayer}`,
        timestamp: messageTimestampMs - traceSteps.length * 500 - 100,
        duration: 30,
        details: {
          reason: intentType === 'Greeting' ? '简单问候，使用快速响应' : '基于复杂度评估',
        },
      });
      
      // 3. 添加原始 traceSteps
      traceSteps.forEach((trace, index) => {
        const text = (trace.thought + ' ' + trace.action).toLowerCase();
        let type: 'reasoning' | 'search' | 'code' | 'insight' = 'reasoning';
        if (text.includes('search') || text.includes('检索') || text.includes('查找') || text.includes('query') || text.includes('rag')) {
          type = 'search';
        } else if (text.includes('code') || text.includes('代码') || text.includes('program') || text.includes('function')) {
          type = 'code';
        } else if (text.includes('insight') || text.includes('发现') || text.includes('realize') || text.includes('understand')) {
          type = 'insight';
        }
        
        steps.push({
          id: `trace-${index}`,
          type,
          title: STEP_LABELS[type] || '思考',
          content: trace.thought || trace.action || 'Processing...',
          timestamp: messageTimestampMs - (traceSteps.length - index) * 500,
          duration: trace.observation ? 500 : undefined,
          details: {
            thought: trace.thought,
            action: trace.action,
            observation: trace.observation,
          },
        });
      });
      
      return {
        steps,
        systemSwitches: [],
        paradigmSwitches: [],
        callStack: [],
        currentLayer: message.cognitiveLayer || 'unknown',
        currentParadigm: 'unknown',
        startTime: messageTimestampMs,
        confidence: 0,
      };
    }
    
    return undefined;
  }, [message.traceSteps, message.cognitiveLayer, messageTimestampMs, thinkingProcess, isUser]);
  
  // 判断是否是最后一条正在思考的消息
  const showThinking = !!isThinking;
  
  return (
    <div className={cn("flex w-full gap-4 max-w-4xl mx-auto group", isUser ? "flex-row-reverse" : "flex-row")}>
      <div className={cn(
        "w-8 h-8 rounded-full flex items-center justify-center shrink-0 shadow-sm mt-1",
        isUser 
            ? "bg-zinc-200 dark:bg-zinc-800" 
            : "bg-gradient-to-br from-blue-600 to-indigo-600 shadow-lg shadow-blue-500/20"
      )}>
        {isUser ? (
            <User className="w-5 h-5 text-zinc-600 dark:text-zinc-300" />
        ) : (
            <Bot className="w-5 h-5 text-white" />
        )}
      </div>

      <div className="flex-1 min-w-0 space-y-3">
        {/* 思考过程 - 在每条 assistant 消息前显示 */}
        {!isUser && messageThinkingProcess && (
          <EnhancedThinkingVisualization
            process={messageThinkingProcess}
            isThinking={showThinking || false}
            onIntervene={(request) => { if (import.meta.env.DEV) console.debug('Intervention:', request); }}
            onSuggestionClick={(suggestion) => { if (import.meta.env.DEV) console.debug('Suggestion:', suggestion); }}
            onSendMessage={onSendMessage}
            lastUserMessage={lastUserMessage}
            lastAssistantMessage={typeof message.content === 'string' ? message.content : ''}
            conversationHistory={conversationHistory}
          />
        )}
        
        {/* 正在思考指示器 - 仅在最后一条消息且正在思考时显示 */}
        {showThinking && (
          <CrabThinking message="小螃蟹正在努力思考..." />
        )}

      <div className={cn(
        "rounded-2xl px-6 py-4 shadow-sm relative transition-all duration-200",
        isUser
          ? "bg-blue-600 text-white rounded-tr-sm"
          : "bg-zinc-100 text-zinc-900 dark:bg-zinc-800 dark:text-zinc-100 border border-zinc-200 dark:border-zinc-700 shadow-md shadow-zinc-200/50 dark:shadow-black/20 rounded-tl-sm"
      )}>
        {/* User Actions */}
        {isUser && !isEditing && (
            <div className="absolute top-2 left-2 opacity-0 group-hover:opacity-100 transition-opacity flex gap-1">
                <button onClick={startEditing} className="p-1 hover:bg-white/20 rounded text-white/70 hover:text-white" title="Edit">
                    <Pencil className="w-3.5 h-3.5" />
                </button>
                <button onClick={() => message.id && onDelete?.(message.id)} className="p-1 hover:bg-white/20 rounded text-white/70 hover:text-white" title="Delete">
                    <Trash2 className="w-3.5 h-3.5" />
                </button>
            </div>
        )}

        {/* Assistant Actions */}
        {!isUser && (
            <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity flex gap-1">
                <button 
                    onClick={handleCopy}
                    className="p-1.5 hover:bg-zinc-200 dark:hover:bg-zinc-700 rounded-md text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300 transition-colors"
                    title={copied ? "Copied" : "Copy"}
                >
                    {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                </button>
                {/* Optional: Add Delete for Assistant too */}
                <button 
                    onClick={() => message.id && onDelete?.(message.id)}
                    className="p-1.5 hover:bg-zinc-200 dark:hover:bg-zinc-700 rounded-md text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300 transition-colors"
                    title="Delete"
                >
                    <Trash2 className="w-3.5 h-3.5" />
                </button>
            </div>
        )}
        {!isUser && message.cognitiveLayer && message.cognitiveLayer !== 'unknown' && (
          <div className="mb-2">
            <span className="inline-flex items-center px-2 py-0.5 rounded-md text-[10px] font-medium bg-indigo-100 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-300">
              {cognitiveLayerLabel(message.cognitiveLayer)}
            </span>
          </div>
        )}
        <div className={cn(
            "prose prose-sm max-w-none break-words leading-relaxed prose-zinc dark:prose-zinc",
            "prose-p:my-2 prose-headings:my-3",
            "prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded-md prose-code:before:content-none prose-code:after:content-none",
            isUser 
                ? "prose-p:text-white prose-headings:text-white prose-strong:text-white prose-li:text-white prose-code:text-white prose-code:bg-blue-700" 
                : "prose-p:text-zinc-800 dark:prose-p:text-zinc-100 prose-headings:text-zinc-900 dark:prose-headings:text-zinc-100 prose-strong:text-zinc-900 dark:prose-strong:text-zinc-100 prose-li:text-zinc-800 dark:prose-li:text-zinc-200 prose-code:text-zinc-900 dark:prose-code:text-zinc-100 prose-code:bg-zinc-200 dark:prose-code:bg-zinc-700"
        )}>
          {renderedContent}
        </div>
        {!isUser && Array.isArray(message.citations) && message.citations.length > 0 && (
          <div className="mt-3 space-y-2">
            <div className="text-[11px] text-zinc-500 dark:text-zinc-400 font-medium">引用来源</div>
            {message.citations.map((c, idx) => (
              <div key={`${c.source}-${idx}`} className="rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white/70 dark:bg-zinc-900/60 p-2">
                <div className="text-[11px] text-zinc-500 dark:text-zinc-400">
                  {c.source} · score {Number(c.score || 0).toFixed(3)}
                </div>
                <div className="text-xs text-zinc-700 dark:text-zinc-300 mt-1">{c.snippet}</div>
                <div className="mt-2">
                  <a href={knowledgeJump(c.source, c.snippet)} className="inline-flex text-[11px] px-2 py-1 rounded border border-zinc-300 dark:border-zinc-600 text-zinc-600 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-800">
                    查看来源
                  </a>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
      </div>
    </div>
  );
});
