/**
 * MessageList — 虚拟滚动消息列表
 *   空状态展示欢迎卡片 + 建议按钮；非空时用 react-virtuoso 虚拟渲染
 */
import { useRef } from 'react';
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso';
import { MessageBubble } from './MessageBubble';
import { CrabEmptyState } from '../ui/CrabElements';
import type { ExtendedMessage } from '@/store/chatStore';
import type { CitationItem } from '@/store/chatStore';

interface MessageListProps {
  messages: ExtendedMessage[];
  isDraftMode: boolean;
  isThinking: boolean;
  // thinkingProcess type is inferred from useAgentThinking — use unknown to avoid coupling
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  thinkingProcess: any;
  deleteMessage: (id: string) => void;
  editMessage: (id: string, content: string) => void;
  sendMessage: (content: string, citations?: CitationItem[]) => Promise<void>;
  onSuggestionClick: (text: string) => void;
}

const DRAFT_SUGGESTIONS = ['写一篇技术博客', '设计系统架构', '起草产品需求', '编写开发文档'];
const CHAT_SUGGESTIONS = ['解释代码', '生成文档', '调试错误', '优化性能'];

export const MessageList = ({
  messages,
  isDraftMode,
  isThinking,
  thinkingProcess,
  deleteMessage,
  editMessage,
  sendMessage,
  onSuggestionClick,
}: MessageListProps) => {
  const virtuosoRef = useRef<VirtuosoHandle>(null);

  return (
    <div className="flex-1 relative bg-zinc-50 dark:bg-zinc-950">
      <div className="absolute inset-0 max-w-5xl mx-auto w-full">
        {messages.length === 0 ? (
          <CrabEmptyState
            title={isDraftMode ? '准备好开始创作了' : '小螃蟹准备好了'}
            description={
              isDraftMode
                ? '草稿模式已开启。我将通过调研、撰写、审阅和精炼的闭环流程，为您生成高质量的内容。'
                : '我是 Crablet，你的 AI 助手。我可以帮你写代码、解答问题、分析数据。开始你的第一次对话吧！'
            }
            action={
              <div className="flex flex-wrap justify-center gap-2 mt-4">
                {(isDraftMode ? DRAFT_SUGGESTIONS : CHAT_SUGGESTIONS).map((suggestion) => (
                  <button
                    key={suggestion}
                    onClick={() => onSuggestionClick(suggestion)}
                    className="px-3 py-1.5 text-xs bg-zinc-100 dark:bg-zinc-800 hover:bg-zinc-200 dark:hover:bg-zinc-700 text-zinc-600 dark:text-zinc-300 rounded-lg transition-colors"
                  >
                    {suggestion}
                  </button>
                ))}
              </div>
            }
          />
        ) : (
          <Virtuoso
            ref={virtuosoRef}
            style={{ height: '100%' }}
            data={messages}
            followOutput="auto"
            className="scroller-content"
            itemContent={(index, msg) => {
              const isLastMessage = index === messages.length - 1;
              const isAssistant = msg.role === 'assistant';
              return (
                <div className="py-8 px-4 md:px-8">
                  <MessageBubble
                    message={msg}
                    onDelete={deleteMessage}
                    onEdit={editMessage}
                    thinkingProcess={isLastMessage && isAssistant ? thinkingProcess : undefined}
                    isThinking={isLastMessage && isAssistant ? isThinking : false}
                    onSendMessage={sendMessage}
                    conversationHistory={[]}
                    lastUserMessage=""
                  />
                </div>
              );
            }}
            components={{
              Footer: () => <div className="h-8" />,
            }}
          />
        )}
      </div>
    </div>
  );
};
