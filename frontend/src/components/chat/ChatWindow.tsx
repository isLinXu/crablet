import { useState, useRef, useEffect, useMemo } from 'react';
import { useChatStore } from '../../store/chatStore';
import type { ExtendedMessage } from '../../store/chatStore';
import { useStreamingChat } from '../../hooks/useStreamingChat';
import { useKeyboard } from '../../hooks/useKeyboard';
import { X } from 'lucide-react';
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso';
import { MessageBubble } from './MessageBubble';
import { SessionList } from './SessionList';
import { SkillSlots } from './SkillSlots';
import { CrabEmptyState } from '../ui/CrabElements';
import { inferCognitiveLayer, type CognitiveLayer } from '@/utils/cognitive';
import { useModelStore } from '@/store/modelStore';
import { validateFile, heuristicSecurityScan, computeFileHash, extractTagsByName } from '@/utils/filePipeline';
import { extractFileContent, getFileTypeDescription } from '@/utils/fileContentExtractor';
import { knowledgeService } from '@/services/knowledgeService';
import { archiveIndexService } from '@/services/archiveIndexService';
import toast from 'react-hot-toast';
import { LOCAL_STORAGE_KEYS } from '@/utils/constants';
import { useChatThinking } from '@/hooks/useChatThinking';
import { RagConfigPanel, type RagConfig } from '../rag/RagConfigPanel';
import { ChatHeader } from './ChatHeader';
import { ChatComposer } from './ChatComposer';
import type { PendingAttachment, RetrievalHit } from './types';

const getRetrievalSource = (hit: RetrievalHit) =>
  String(hit.metadata?.source ?? hit.metadata?.source_trace ?? 'unknown');

export const ChatWindow = () => {
  const {
    messages,
    isConnected,
    isThinking,
    isDraftMode,
    createSession,
    currentCognitiveLayer,
    sessionId,
    deleteMessage,
    editMessage,
    setDraftMode,
  } = useChatStore();
  const { sendMessage } = useStreamingChat();
  const [input, setInput] = useState('');
  const [showMobileHistory, setShowMobileHistory] = useState(false);
  const [attachments, setAttachments] = useState<PendingAttachment[]>([]);
  const [priority, setPriority] = useState<'speed' | 'quality' | 'balanced'>('balanced');
  const [retrievalHits, setRetrievalHits] = useState<RetrievalHit[]>([]);
  const [retrieving, setRetrieving] = useState(false);
  const [selectedRetrieval, setSelectedRetrieval] = useState<number[]>([]);
  const [showRagConfig, setShowRagConfig] = useState(false);
  const [ragConfig, setRagConfig] = useState<RagConfig | null>(null);
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const allProviders = useModelStore((s) => s.providers);
  const providers = useMemo(() => allProviders.filter((p) => p.enabled), [allProviders]);
  const resolveForPrompt = useModelStore((s) => s.resolveForPrompt);
  const setSessionManualProvider = useModelStore((s) => s.setSessionManualProvider);
  const manualMap = useModelStore((s) => s.sessionManualProvider);

  const handleSend = async () => {
    if (!input.trim() || isThinking) return;

    const allAttachments = attachments.filter((a) => a.status !== 'failed');
    let fileContents = '';
    const MAX_FILE_CONTENT_LENGTH = 5 * 1024 * 1024;

    for (const attachment of allAttachments) {
      try {
        setAttachments(prev => prev.map(a =>
          a.id === attachment.id ? { ...a, status: 'processing' } : a
        ));

        const result = await extractFileContent(attachment.file, {
          maxLength: MAX_FILE_CONTENT_LENGTH,
          onOcrStart: () => {
            setAttachments(prev => prev.map(a =>
              a.id === attachment.id ? { ...a, isOcr: true } : a
            ));
          },
          onProgress: (progress) => {
            setAttachments(prev => prev.map(a =>
              a.id === attachment.id ? { ...a, ocrProgress: progress } : a
            ));
          },
        });

        if (result.success) {
          const fileType = getFileTypeDescription(attachment.file.name);
          const archiveStatus = attachment.status === 'uploaded' ? '' : ' (未归档)';
          const ocrStatus = result.isOcr ? ' (OCR)' : '';
          fileContents += `\n\n[${fileType}: ${attachment.file.name}]${archiveStatus}${ocrStatus}${result.truncated ? ' (内容已截断)' : ''}\n${result.text}`;
        } else {
          fileContents += `\n\n[文件: ${attachment.file.name}] (无法读取内容: ${result.error || '未知错误'})`;
        }
      } catch (e) {
        console.error(`[Chat] 文件提取失败:`, e);
        fileContents += `\n\n[文件: ${attachment.file.name}] (无法读取内容)`;
      }
    }

    const attachmentSummary = allAttachments
      .map((a) => `[文件${a.status === 'uploaded' ? '' : ' (未归档)'}] ${a.file.name}`)
      .join('\n');
    const picked = selectedRetrieval
      .map((idx) => retrievalHits[idx])
      .filter(Boolean)
      .slice(0, 3);
    const retrievalSummary = picked
      .map((r, idx) => `[检索片段${idx + 1}] source=${r.metadata?.source || r.metadata?.source_trace || 'unknown'} score=${Number(r.score || 0).toFixed(3)}\n${String(r.content || '').slice(0, 400)}`)
      .join('\n\n');

    let finalPrompt = input;

    if (isDraftMode && !finalPrompt.toLowerCase().startsWith('draft ')) {
      finalPrompt = `draft ${finalPrompt}`;
    }

    if (attachmentSummary) {
      finalPrompt += `\n\n[附件列表]\n${attachmentSummary}`;
    }
    if (fileContents) {
      finalPrompt += `\n\n[文件内容]${fileContents}`;
    }
    if (retrievalSummary) {
      finalPrompt += `\n\n[知识检索上下文]\n${retrievalSummary}`;
    }

    setInput('');
    setAttachments([]);
    setRetrievalHits([]);

    await sendMessage(
      finalPrompt,
      picked.map((r) => ({
        source: getRetrievalSource(r),
        score: Number(r.score || 0),
        snippet: String(r.content || '').slice(0, 240),
      }))
    );
  };

  const handleNewChat = () => {
    createSession('New Chat');
    setInput('');
  };

  useKeyboard({
    'Enter': (e) => {
      if (!e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    'Cmd+Enter': () => handleSend(),
  });

  const inferredLayer = useMemo(() => {
    const lastAssistant = [...messages].reverse().find(
      (message): message is ExtendedMessage =>
        message.role === 'assistant' && message.cognitiveLayer !== undefined
    );
    if (lastAssistant?.cognitiveLayer) {
      return lastAssistant.cognitiveLayer;
    }
    const latestAssistantText = [...messages]
      .reverse()
      .find((message) => message.role === 'assistant' && typeof message.content === 'string');
    if (latestAssistantText && typeof latestAssistantText.content === 'string') {
      return inferCognitiveLayer({ text: latestAssistantText.content });
    }
    return 'unknown' as CognitiveLayer;
  }, [messages]);

  const currentLayer = currentCognitiveLayer !== 'unknown' ? currentCognitiveLayer : inferredLayer;

  const resolvedModel = useMemo(() =>
    resolveForPrompt(sessionId, input || 'general', priority),
    [sessionId, input, priority, resolveForPrompt]
  );

  const {
    thinkingProcess,
    isManualMode,
    manualLayer,
    manualParadigm,
    toggleManualMode,
    setManualLayerSelected,
    setManualParadigmSelected,
  } = useChatThinking({
    isThinking,
    messages,
    resolvedModel,
    currentLayer,
    sessionId,
  });

  const handlePickFiles = async (files: FileList | null) => {
    if (!files?.length) return;
    const next: PendingAttachment[] = [];
    for (const file of Array.from(files)) {
      const check = validateFile(file);
      if (!check.ok) {
        toast.error(`${file.name}: ${check.reason}`);
        continue;
      }
      const security = await heuristicSecurityScan(file);
      if (!security.safe) {
        toast.error(`${file.name}: ${security.reason}`);
        continue;
      }
      const hash = await computeFileHash(file);
      next.push({ id: `${Date.now()}-${Math.random()}`, file, progress: 0, status: 'pending', hash });
    }
    setAttachments((prev) => [...prev, ...next]);
    if (fileInputRef.current) fileInputRef.current.value = '';
  };

  const archiveAttachment = async (item: PendingAttachment, autoRule = 'manual') => {
    setAttachments((prev) => prev.map((x) => (x.id === item.id ? { ...x, status: 'uploading', progress: 1 } : x)));
    try {
      const tags = extractTagsByName(item.file.name);
      await knowledgeService.uploadFile(item.file, {
        tags,
        archivePath: '/default',
        autoRule,
        onProgress: (p) => setAttachments((prev) => prev.map((x) => (x.id === item.id ? { ...x, progress: p } : x))),
      });
      archiveIndexService.upsert(item.hash || '', item.file.name, 'file', tags);
      setAttachments((prev) => prev.map((x) => (x.id === item.id ? { ...x, status: 'uploaded', progress: 100 } : x)));
      toast.success(`${item.file.name} 已归档到知识库`);
    } catch {
      setAttachments((prev) => prev.map((x) => (x.id === item.id ? { ...x, status: 'failed' } : x)));
      toast.error(`${item.file.name} 归档失败`);
    }
  };

  useEffect(() => {
    const saved = localStorage.getItem(LOCAL_STORAGE_KEYS.MODEL_PRIORITY);
    if (saved === 'speed' || saved === 'quality' || saved === 'balanced') setPriority(saved);
  }, []);

  useEffect(() => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.MODEL_PRIORITY, priority);
  }, [priority]);

  useEffect(() => {
    const enabled = localStorage.getItem('crablet-auto-archive-enabled') === '1';
    if (!enabled) return;
    attachments
      .filter((a) => a.status === 'pending')
      .forEach((a) => {
        if (/\.(pdf|doc|docx|txt|md|csv|xls|xlsx)$/i.test(a.file.name)) {
          archiveAttachment(a, 'auto-doc-rule');
        }
      });
  }, [attachments]);

  useEffect(() => {
    const q = input.trim();
    if (q.length < 6) {
      setRetrievalHits([]);
      setSelectedRetrieval([]);
      return;
    }
    const timer = setTimeout(async () => {
      try {
        setRetrieving(true);
        const results = await knowledgeService.search(q);
        const hits = (Array.isArray(results) ? results : []).slice(0, 5);
        setRetrievalHits(hits);
        setSelectedRetrieval(hits.map((_, i) => i));
      } catch {
        setRetrievalHits([]);
        setSelectedRetrieval([]);
      } finally {
        setRetrieving(false);
      }
    }, 280);
    return () => clearTimeout(timer);
  }, [input]);

  return (
    <div className="flex flex-col h-full bg-zinc-50 dark:bg-zinc-950 transition-colors duration-200 relative">
      {/* Mobile History Drawer */}
      {showMobileHistory && (
        <div className="absolute inset-0 z-50 flex md:hidden">
          <div className="w-64 bg-white dark:bg-zinc-900 border-r border-zinc-200 dark:border-zinc-800 h-full shadow-xl">
            <div className="p-4 border-b border-zinc-200 dark:border-zinc-800 flex justify-between items-center">
              <h2 className="font-semibold text-zinc-700 dark:text-zinc-200">History</h2>
              <button onClick={() => setShowMobileHistory(false)} className="p-1 hover:bg-zinc-100 dark:hover:bg-zinc-800 rounded">
                <X className="w-5 h-5" />
              </button>
            </div>
            <div className="flex-1 overflow-hidden h-[calc(100%-60px)]">
              <SessionList />
            </div>
          </div>
          <div className="flex-1 bg-black/50 backdrop-blur-sm" onClick={() => setShowMobileHistory(false)} />
        </div>
      )}

      <ChatHeader
        isConnected={isConnected}
        currentLayer={currentLayer}
        vendor={resolvedModel?.vendor || 'unknown'}
        messages={messages}
        sessionId={sessionId}
        onNewChat={handleNewChat}
        onShowMobileHistory={() => setShowMobileHistory(true)}
      />

      {/* Messages Area */}
      <div className="flex-1 relative bg-zinc-50 dark:bg-zinc-950">
        <div className="absolute inset-0 max-w-5xl mx-auto w-full">
          {messages.length === 0 ? (
            <CrabEmptyState
              title={isDraftMode ? "准备好开始创作了" : "小螃蟹准备好了"}
              description={isDraftMode ? "草稿模式已开启。我将通过调研、撰写、审阅和精炼的闭环流程，为您生成高质量的内容。" : "我是 Crablet，你的 AI 助手。我可以帮你写代码、解答问题、分析数据。开始你的第一次对话吧！"}
              action={
                <div className="flex flex-wrap justify-center gap-2 mt-4">
                  {(isDraftMode ? ['写一篇技术博客', '设计系统架构', '起草产品需求', '编写开发文档'] : ['解释代码', '生成文档', '调试错误', '优化性能']).map((suggestion) => (
                    <button
                      key={suggestion}
                      onClick={() => setInput(suggestion)}
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
                Footer: () => <div className="h-8" />
              }}
            />
          )}
        </div>
      </div>

      <SkillSlots
        skills={[
          { id: '1', name: '代码审查', icon: 'code', description: '分析代码质量和潜在问题', color: 'blue' },
          { id: '2', name: '文档生成', icon: 'file', description: '自动生成代码文档', color: 'emerald' },
          { id: '3', name: '智能搜索', icon: 'search', description: '深度检索知识库', color: 'amber' },
        ]}
        onSkillAdd={() => toast('技能选择器开发中...', { icon: '🔧' })}
      />

      <ChatComposer
        input={input}
        setInput={setInput}
        isThinking={isThinking}
        isDraftMode={isDraftMode}
        onSend={handleSend}
        onPickFiles={handlePickFiles}
        fileInputRef={fileInputRef}
        attachments={attachments}
        onArchiveAttachment={archiveAttachment}
        retrievalHits={retrievalHits}
        retrieving={retrieving}
        selectedRetrieval={selectedRetrieval}
        setSelectedRetrieval={setSelectedRetrieval}
        priority={priority}
        setPriority={setPriority}
        providers={providers}
        sessionId={sessionId}
        manualMap={manualMap}
        setSessionManualProvider={setSessionManualProvider}
        isManualMode={isManualMode}
        toggleManualMode={toggleManualMode}
        manualLayer={manualLayer}
        setManualLayerSelected={setManualLayerSelected}
        manualParadigm={manualParadigm}
        setManualParadigmSelected={setManualParadigmSelected}
        ragConfig={ragConfig}
        onShowRagConfig={() => setShowRagConfig(true)}
        onToggleDraftMode={() => setDraftMode(!isDraftMode)}
        getRetrievalSource={getRetrievalSource}
      />

      <RagConfigPanel
        isOpen={showRagConfig}
        onClose={() => setShowRagConfig(false)}
        onConfigChange={(config) => {
          setRagConfig(config);
          toast.success('RAG配置已保存');
        }}
      />
    </div>
  );
};
