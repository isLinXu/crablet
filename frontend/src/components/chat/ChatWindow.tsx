import { useState, useRef, useEffect, useMemo } from 'react';
import { useChatStore } from '../../store/chatStore';
import { useWebSocket } from '../../hooks/useWebSocket';
import { useStreamingChat } from '../../hooks/useStreamingChat';
import { useKeyboard } from '../../hooks/useKeyboard';
import { Send, Bot, Loader2, StopCircle, History, X, PlusCircle, Upload } from 'lucide-react';
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso';
import { Button } from '../ui/Button';
import { MessageBubble } from './MessageBubble';
import { SessionList } from './SessionList';
import clsx from 'clsx';
import { cognitiveLayerLabel, inferCognitiveLayer, type CognitiveLayer } from '@/utils/cognitive';
import { useModelStore } from '@/store/modelStore';
import { validateFile, heuristicSecurityScan, computeFileHash, extractTagsByName } from '@/utils/filePipeline';
import { knowledgeService } from '@/services/knowledgeService';
import { archiveIndexService } from '@/services/archiveIndexService';
import toast from 'react-hot-toast';
import { LOCAL_STORAGE_KEYS } from '@/utils/constants';

interface PendingAttachment {
  id: string;
  file: File;
  progress: number;
  status: 'pending' | 'uploading' | 'uploaded' | 'failed';
  hash?: string;
}
interface RetrievalHit {
  content: string;
  score: number;
  metadata?: Record<string, any>;
}

export const ChatWindow = () => {
  const { messages, isConnected, isThinking, createSession, currentCognitiveLayer, sessionId } = useChatStore();
  const { systemLogs = [] } = useWebSocket() as any;
  const { sendMessage } = useStreamingChat();
  const [input, setInput] = useState('');
  const [showMobileHistory, setShowMobileHistory] = useState(false);
  const [attachments, setAttachments] = useState<PendingAttachment[]>([]);
  const [priority, setPriority] = useState<'speed' | 'quality' | 'balanced'>('balanced');
  const [retrievalHits, setRetrievalHits] = useState<RetrievalHit[]>([]);
  const [retrieving, setRetrieving] = useState(false);
  const [selectedRetrieval, setSelectedRetrieval] = useState<number[]>([]);
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const allProviders = useModelStore((s) => s.providers);
  const providers = useMemo(() => allProviders.filter((p) => p.enabled), [allProviders]);
  const resolveForPrompt = useModelStore((s) => s.resolveForPrompt);
  const setSessionManualProvider = useModelStore((s) => s.setSessionManualProvider);
  const manualMap = useModelStore((s) => s.sessionManualProvider);

  const handleSend = async () => {
    if (!input.trim() || isThinking) return;
    const attachmentSummary = attachments
      .filter((a) => a.status === 'uploaded')
      .map((a) => `[文件] ${a.file.name}`)
      .join('\n');
    const picked = selectedRetrieval
      .map((idx) => retrievalHits[idx])
      .filter(Boolean)
      .slice(0, 3);
    const retrievalSummary = picked
      .map((r, idx) => `[检索片段${idx + 1}] source=${r.metadata?.source || r.metadata?.source_trace || 'unknown'} score=${Number(r.score || 0).toFixed(3)}\n${String(r.content || '').slice(0, 400)}`)
      .join('\n\n');
    const finalPromptBase = attachmentSummary ? `${input}\n\n${attachmentSummary}` : input;
    const finalPrompt = retrievalSummary ? `${finalPromptBase}\n\n[知识检索上下文]\n${retrievalSummary}` : finalPromptBase;
    await sendMessage(
      finalPrompt,
      picked.map((r) => ({
        source: r.metadata?.source || r.metadata?.source_trace || 'unknown',
        score: Number(r.score || 0),
        snippet: String(r.content || '').slice(0, 240),
      }))
    );
    setInput('');
  };

  const handleNewChat = () => {
    createSession('New Chat');
    setInput('');
  };

  useKeyboard({
    'Enter': (e) => {
       // Only send if not holding Shift (allow multiline)
       if (!e.shiftKey) {
           e.preventDefault();
           handleSend();
       }
    },
    'Cmd+Enter': () => handleSend(), // Also support Cmd+Enter
  });

  // Auto-scroll to bottom when messages change
  useEffect(() => {
    if (virtuosoRef.current) {
        // virtuosoRef.current.scrollToIndex({ index: messages.length - 1, behavior: 'smooth' });
        // 'followOutput' prop handles this usually, but explicit scroll helps on load
    }
  }, [messages.length]);

  const inferredLayer = (() => {
    for (let i = systemLogs.length - 1; i >= 0; i -= 1) {
      const e = systemLogs[i] as any;
      if (e?.kind === 'cognitive_layer' && e?.layer) return e.layer;
      const layer = inferCognitiveLayer({ text: e?.thought || e?.message || e?.output || e?.args || '' });
      if (layer !== 'unknown') return layer;
    }
    const lastAssistant = [...messages].reverse().find((m: any) => m.role === 'assistant' && m.cognitiveLayer);
    return ((lastAssistant as any)?.cognitiveLayer || 'unknown') as CognitiveLayer;
  })();
  const currentLayer = currentCognitiveLayer !== 'unknown' ? currentCognitiveLayer : inferredLayer;
  const resolvedModel = resolveForPrompt(sessionId, input || 'general', priority);

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
        setSelectedRetrieval(hits.map((_: any, i: number) => i));
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

      {/* Header */}
      <div className="px-6 py-3 bg-white/80 dark:bg-zinc-900/80 backdrop-blur-md border-b border-zinc-200 dark:border-zinc-800 flex items-center justify-between shrink-0 z-10 sticky top-0">
        <div className="flex items-center gap-3">
          <button 
            className="md:hidden p-2 -ml-2 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800 rounded-md"
            onClick={() => setShowMobileHistory(true)}
          >
            <History className="w-5 h-5" />
          </button>
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 bg-gradient-to-br from-blue-600 to-indigo-600 rounded-xl flex items-center justify-center shadow-lg shadow-blue-500/20">
                <Bot className="w-5 h-5 text-white" />
            </div>
            <div>
                <h1 className="text-sm font-semibold text-zinc-900 dark:text-zinc-100 leading-none tracking-tight">Crablet Agent</h1>
                <p className="text-xs text-zinc-500 dark:text-zinc-400 mt-1 flex items-center gap-1.5 font-medium">
                    <span className={clsx("w-2 h-2 rounded-full", isConnected ? "bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.4)]" : "bg-rose-500")}></span>
                    {isConnected ? 'Online' : 'Offline'} · {cognitiveLayerLabel(currentLayer)}
                </p>
                <p className="text-[11px] text-zinc-500 dark:text-zinc-400 mt-1">
                    {resolvedModel.vendor} · {resolvedModel.model} · {resolvedModel.version}
                </p>
            </div>
          </div>
        </div>
        
        <div className="flex items-center gap-2">
            <Button 
                variant="ghost" 
                size="sm" 
                onClick={handleNewChat}
                className="text-zinc-600 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-800 rounded-lg transition-all"
            >
                <PlusCircle className="w-4 h-4 mr-2" />
                New Chat
            </Button>
        </div>
      </div>

      {/* Messages Area */}
      <div className="flex-1 relative bg-zinc-50 dark:bg-zinc-950">
        <div className="absolute inset-0 max-w-5xl mx-auto w-full">
            {messages.length === 0 ? (
                <div className="h-full flex flex-col items-center justify-center text-zinc-400 dark:text-zinc-600 p-8 text-center animate-in fade-in duration-500">
                    <div className="w-20 h-20 bg-white dark:bg-zinc-900 rounded-3xl shadow-xl shadow-zinc-200/50 dark:shadow-black/20 flex items-center justify-center mb-8 border border-zinc-100 dark:border-zinc-800">
                        <Bot className="w-10 h-10 text-blue-600 dark:text-blue-500" />
                    </div>
                    <h3 className="text-2xl font-semibold text-zinc-900 dark:text-zinc-100 mb-3 tracking-tight">How can I help you today?</h3>
                    <p className="text-base text-zinc-500 max-w-md mx-auto leading-relaxed">I'm Crablet, your AI assistant. I can help you write code, answer questions, and analyze data.</p>
                </div>
            ) : (
                <Virtuoso
                ref={virtuosoRef}
                style={{ height: '100%' }}
                data={messages}
                followOutput="auto"
                className="scroller-content"
                itemContent={(_, msg) => (
                    <div className="py-8 px-4 md:px-8">
                        <MessageBubble message={msg} />
                    </div>
                )}
                components={{
                    Footer: () => isThinking ? (
                    <div className="py-8 px-4 md:px-8">
                        <div className="flex gap-4 max-w-4xl mx-auto pl-2">
                            <div className="w-8 h-8 rounded-full bg-zinc-100 dark:bg-zinc-800 flex items-center justify-center shrink-0">
                                <Bot className="w-5 h-5 text-zinc-400 dark:text-zinc-500" />
                            </div>
                            <div className="flex items-center gap-2 text-zinc-500 dark:text-zinc-400 text-sm h-8">
                                <Loader2 className="w-4 h-4 animate-spin" />
                                <span>Thinking...</span>
                            </div>
                        </div>
                    </div>
                    ) : <div className="h-8" />
                }}
                />
            )}
        </div>
      </div>

      {/* Input Area */}
      <div className="p-6 bg-transparent shrink-0 z-10">
        <div className="max-w-4xl mx-auto">
            <div className="relative flex flex-col gap-2 bg-zinc-100/95 dark:bg-zinc-900/95 border border-zinc-300 dark:border-zinc-700 rounded-2xl p-3 shadow-2xl shadow-zinc-200/50 dark:shadow-black/50 focus-within:ring-2 focus-within:ring-blue-500/20 focus-within:border-blue-500/50 transition-all">
                <textarea
                    value={input}
                    onChange={(e) => setInput(e.target.value)}
                    onKeyDown={(e) => {
                        if (e.key === 'Enter' && !e.shiftKey) {
                            e.preventDefault();
                            handleSend();
                        }
                    }}
                    placeholder="Message Crablet..."
                    disabled={isThinking}
                    className="flex-1 bg-transparent border-none text-zinc-900 dark:text-zinc-100 placeholder:text-zinc-500 dark:placeholder:text-zinc-500 focus:ring-0 resize-none max-h-48 min-h-[44px] py-2 px-2 text-[15px] leading-relaxed scrollbar-thin scrollbar-thumb-zinc-400 dark:scrollbar-thumb-zinc-700"
                    rows={1}
                    style={{ height: 'auto', minHeight: '44px' }} 
                />
                <div className="flex justify-between items-center px-2 pt-2 border-t border-zinc-300/80 dark:border-zinc-800/80">
                    <div className="flex gap-2">
                        <input ref={fileInputRef} type="file" multiple className="hidden" onChange={(e) => handlePickFiles(e.target.files)} />
                        <Button variant="secondary" size="sm" onClick={() => fileInputRef.current?.click()}>
                            <Upload className="w-4 h-4 mr-1" />
                            上传文件
                        </Button>
                        <select
                            className="h-8 rounded-md border border-zinc-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 text-xs px-2"
                            value={sessionId ? (manualMap[sessionId] || '') : ''}
                            onChange={(e) => sessionId && setSessionManualProvider(sessionId, e.target.value || null)}
                        >
                            <option value="">自动路由</option>
                            {providers.map((p) => (
                                <option key={p.id} value={p.id}>
                                    {p.vendor} / {p.model} / {p.version}
                                </option>
                            ))}
                        </select>
                        <select
                            className="h-8 rounded-md border border-zinc-300 dark:border-zinc-700 bg-white dark:bg-zinc-900 text-xs px-2"
                            value={priority}
                            onChange={(e) => setPriority(e.target.value as any)}
                        >
                            <option value="balanced">平衡</option>
                            <option value="speed">速度优先</option>
                            <option value="quality">质量优先</option>
                        </select>
                    </div>
                    <div className="flex items-center gap-3">
                         <span className="text-[10px] text-zinc-400 font-medium hidden sm:inline-block">Use Shift + Enter for new line</span>
                         <Button
                            onClick={handleSend}
                            disabled={isThinking || !input.trim()}
                            size="icon"
                            className={clsx(
                                "h-8 w-8 rounded-lg transition-all duration-200",
                                input.trim() 
                                    ? "bg-blue-600 hover:bg-blue-700 text-white shadow-md shadow-blue-500/20" 
                                    : "bg-zinc-100 dark:bg-zinc-800 text-zinc-400 dark:text-zinc-500"
                            )}
                        >
                            {isThinking ? <StopCircle className="w-4 h-4" /> : <Send className="w-4 h-4" />}
                        </Button>
                    </div>
                </div>
            </div>
            {attachments.length > 0 && (
              <div className="mt-2 rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white/70 dark:bg-zinc-900/60 p-2 space-y-1">
                {attachments.map((a) => (
                  <div key={a.id} className="flex items-center justify-between gap-2 text-xs">
                    <div className="truncate">{a.file.name}</div>
                    <div className="flex items-center gap-2">
                      <span className={clsx(a.status === 'failed' ? 'text-red-500' : 'text-zinc-500')}>
                        {a.status === 'uploading' ? `${a.progress}%` : a.status}
                      </span>
                      {a.status !== 'uploaded' && (
                        <Button size="sm" variant="secondary" onClick={() => archiveAttachment(a)}>
                          添加到知识库
                        </Button>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
            {(retrieving || retrievalHits.length > 0) && (
              <div className="mt-2 rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white/70 dark:bg-zinc-900/60 p-2 space-y-1">
                <div className="text-[11px] text-zinc-500 font-medium">
                  {retrieving ? '检索知识库中...' : `检索命中 ${retrievalHits.length} 条（可勾选注入，最多发送3条）`}
                </div>
                {!retrieving && retrievalHits.length > 0 && (
                  <div className="flex items-center gap-2 text-[11px]">
                    <Button size="sm" variant="secondary" onClick={() => setSelectedRetrieval(retrievalHits.map((_, i) => i))}>全选</Button>
                    <Button size="sm" variant="secondary" onClick={() => setSelectedRetrieval([])}>清空</Button>
                    <span className="text-zinc-500">已选 {selectedRetrieval.length}</span>
                  </div>
                )}
                {!retrieving && retrievalHits.map((r, i) => (
                  <div key={`${i}-${r.score}`} className="text-xs border border-zinc-200 dark:border-zinc-800 rounded p-1.5">
                    <label className="flex items-center gap-2 mb-1">
                      <input
                        type="checkbox"
                        checked={selectedRetrieval.includes(i)}
                        onChange={(e) =>
                          setSelectedRetrieval((prev) =>
                            e.target.checked ? [...new Set([...prev, i])].sort((a, b) => a - b) : prev.filter((x) => x !== i)
                          )
                        }
                      />
                      <span className="text-zinc-500">注入此片段</span>
                    </label>
                    <div className="text-zinc-500">
                      source: {r.metadata?.source || r.metadata?.source_trace || 'unknown'} · score: {Number(r.score || 0).toFixed(3)}
                    </div>
                    <div className="text-zinc-700 dark:text-zinc-300 line-clamp-2">
                      {String(r.content || '').slice(0, 180)}
                    </div>
                  </div>
                ))}
              </div>
            )}
            <div className="text-center mt-3 text-[11px] text-zinc-400 dark:text-zinc-600 font-medium">
                Crablet can make mistakes. Consider checking important information.
            </div>
        </div>
      </div>
    </div>
  );
};
