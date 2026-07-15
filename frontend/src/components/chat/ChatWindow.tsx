import { useState, useRef, useEffect, useMemo } from 'react';
import { useChatStore } from '../../store/chatStore';
import type { ExtendedMessage } from '../../store/chatStore';
import { useStreamingChat } from '../../hooks/useStreamingChat';
import { useKeyboard } from '../../hooks/useKeyboard';
import { useFileAttachments } from '../../hooks/useFileAttachments';
import { useRetrievalSearch } from '../../hooks/useRetrievalSearch';
import { SkillSlots } from './SkillSlots';
import { MobileHistoryDrawer } from './MobileHistoryDrawer';
import { MessageList } from './MessageList';
import { inferCognitiveLayer, type CognitiveLayer } from '@/utils/cognitive';
import { useModelStore } from '@/store/modelStore';
import { extractFileContent, getFileTypeDescription } from '@/utils/fileContentExtractor';
import toast from 'react-hot-toast';
import { LOCAL_STORAGE_KEYS } from '@/utils/constants';
import { useChatThinking } from '@/hooks/useChatThinking';
import { RagConfigPanel, type RagConfig } from '../rag/RagConfigPanel';
import { ChatHeader } from './ChatHeader';
import { ChatComposer } from './ChatComposer';
import type { RetrievalHit } from './types';

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
  const { sendMessage, cancelMessage } = useStreamingChat();

  const [input, setInput] = useState('');
  const [showMobileHistory, setShowMobileHistory] = useState(false);
  const [priority, setPriority] = useState<'speed' | 'quality' | 'balanced'>('balanced');
  const [showRagConfig, setShowRagConfig] = useState(false);
  const [ragConfig, setRagConfig] = useState<RagConfig | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);

  // ── Sub-hooks ──────────────────────────────────────────────────────────────
  const {
    attachments,
    handlePickFiles,
    archiveAttachment,
    clearAttachments,
  } = useFileAttachments();

  const {
    retrievalHits,
    retrieving,
    selectedRetrieval,
    setSelectedRetrieval,
    clearRetrievalHits,
  } = useRetrievalSearch(input);

  // ── Model selection ────────────────────────────────────────────────────────
  const allProviders = useModelStore((s) => s.providers);
  const providers = useMemo(() => allProviders.filter((p) => p.enabled), [allProviders]);
  const resolveForPrompt = useModelStore((s) => s.resolveForPrompt);
  const setSessionManualProvider = useModelStore((s) => s.setSessionManualProvider);
  const manualMap = useModelStore((s) => s.sessionManualProvider);

  const resolvedModel = useMemo(
    () => resolveForPrompt(sessionId, input || 'general', priority),
    [sessionId, input, priority, resolveForPrompt]
  );

  // ── Cognitive layer inference ──────────────────────────────────────────────
  const inferredLayer = useMemo(() => {
    const lastAssistant = [...messages].reverse().find(
      (message): message is ExtendedMessage =>
        message.role === 'assistant' && message.cognitiveLayer !== undefined
    );
    if (lastAssistant?.cognitiveLayer) return lastAssistant.cognitiveLayer;
    const latestText = [...messages]
      .reverse()
      .find((m) => m.role === 'assistant' && typeof m.content === 'string');
    if (latestText && typeof latestText.content === 'string')
      return inferCognitiveLayer({ text: latestText.content });
    return 'unknown' as CognitiveLayer;
  }, [messages]);

  const currentLayer = currentCognitiveLayer !== 'unknown' ? currentCognitiveLayer : inferredLayer;

  // ── Thinking visualization ─────────────────────────────────────────────────
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

  // ── Priority persistence ───────────────────────────────────────────────────
  useEffect(() => {
    const saved = localStorage.getItem(LOCAL_STORAGE_KEYS.MODEL_PRIORITY);
    if (saved === 'speed' || saved === 'quality' || saved === 'balanced') setPriority(saved);
  }, []);

  useEffect(() => {
    localStorage.setItem(LOCAL_STORAGE_KEYS.MODEL_PRIORITY, priority);
  }, [priority]);

  // ── Send handler ───────────────────────────────────────────────────────────
  const handleSend = async () => {
    if (!input.trim() || isThinking) return;

    const allAttachments = attachments.filter((a) => a.status !== 'failed');
    let fileContents = '';
    const MAX_FILE_CONTENT_LENGTH = 5 * 1024 * 1024;

    for (const attachment of allAttachments) {
      try {
        const result = await extractFileContent(attachment.file, {
          maxLength: MAX_FILE_CONTENT_LENGTH,
          onOcrStart: () => {},
          onProgress: () => {},
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
      .map(
        (r, idx) =>
          `[检索片段${idx + 1}] source=${r.metadata?.source || r.metadata?.source_trace || 'unknown'} score=${Number(r.score || 0).toFixed(3)}\n${String(r.content || '').slice(0, 400)}`
      )
      .join('\n\n');

    let finalPrompt = input;
    if (isDraftMode && !finalPrompt.toLowerCase().startsWith('draft ')) {
      finalPrompt = `draft ${finalPrompt}`;
    }
    if (attachmentSummary) finalPrompt += `\n\n[附件列表]\n${attachmentSummary}`;
    if (fileContents) finalPrompt += `\n\n[文件内容]${fileContents}`;
    if (retrievalSummary) finalPrompt += `\n\n[知识检索上下文]\n${retrievalSummary}`;

    setInput('');
    clearAttachments();
    clearRetrievalHits();

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
    Enter: (e) => {
      if (!e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    'Cmd+Enter': () => handleSend(),
  });

  return (
    <div className="flex flex-col h-full bg-zinc-50 dark:bg-zinc-950 transition-colors duration-200 relative">
      {showMobileHistory && (
        <MobileHistoryDrawer onClose={() => setShowMobileHistory(false)} />
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

      <MessageList
        messages={messages}
        isDraftMode={isDraftMode}
        isThinking={isThinking}
        thinkingProcess={thinkingProcess}
        deleteMessage={deleteMessage}
        editMessage={editMessage}
        sendMessage={sendMessage}
        onSuggestionClick={setInput}
      />

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
        onCancel={cancelMessage}
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
