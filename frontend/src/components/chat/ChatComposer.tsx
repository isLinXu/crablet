import { Button } from '../ui/Button';
import { Send, StopCircle, Upload, Settings, BookOpen, PenLine } from 'lucide-react';
import clsx from 'clsx';
import type { CognitiveLayer } from '@/utils/cognitive';
import type { AgentParadigm } from './EnhancedThinkingVisualization';
import type { RagConfig } from '../rag/RagConfigPanel';
import type { PendingAttachment, RetrievalHit } from './types';

interface ProviderOption {
  id: string;
  vendor: string;
  model: string;
}

interface ChatComposerProps {
  input: string;
  setInput: (v: string) => void;
  isThinking: boolean;
  isDraftMode: boolean;
  onSend: () => void;
  onPickFiles: (files: FileList | null) => void;
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  attachments: PendingAttachment[];
  onArchiveAttachment: (item: PendingAttachment) => void;
  retrievalHits: RetrievalHit[];
  retrieving: boolean;
  selectedRetrieval: number[];
  setSelectedRetrieval: React.Dispatch<React.SetStateAction<number[]>>;
  priority: 'speed' | 'quality' | 'balanced';
  setPriority: (v: 'speed' | 'quality' | 'balanced') => void;
  providers: ProviderOption[];
  sessionId: string | null;
  manualMap: Record<string, string>;
  setSessionManualProvider: (sessionId: string, providerId: string | null) => void;
  isManualMode: boolean;
  toggleManualMode: (v: boolean) => void;
  manualLayer: CognitiveLayer;
  setManualLayerSelected: (v: CognitiveLayer) => void;
  manualParadigm: AgentParadigm;
  setManualParadigmSelected: (v: AgentParadigm) => void;
  ragConfig: RagConfig | null;
  onShowRagConfig: () => void;
  onToggleDraftMode: () => void;
  getRetrievalSource: (hit: RetrievalHit) => string;
}

export const ChatComposer = ({
  input,
  setInput,
  isThinking,
  isDraftMode,
  onSend,
  onPickFiles,
  fileInputRef,
  attachments,
  onArchiveAttachment,
  retrievalHits,
  retrieving,
  selectedRetrieval,
  setSelectedRetrieval,
  priority,
  setPriority,
  providers,
  sessionId,
  manualMap,
  setSessionManualProvider,
  isManualMode,
  toggleManualMode,
  manualLayer,
  setManualLayerSelected,
  manualParadigm,
  setManualParadigmSelected,
  ragConfig,
  onShowRagConfig,
  onToggleDraftMode,
  getRetrievalSource,
}: ChatComposerProps) => {
  return (
    <div className="p-4 bg-transparent shrink-0 z-10">
      <div className="max-w-4xl mx-auto space-y-3">
        {/* 设置条 */}
        <div className="flex items-center justify-between px-1">
          <div className="flex items-center gap-2">
            <input ref={fileInputRef} type="file" multiple className="hidden" onChange={(e) => onPickFiles(e.target.files)} />
            <Button
              variant="ghost"
              size="sm"
              onClick={() => fileInputRef.current?.click()}
              className="text-zinc-500 hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200"
            >
              <Upload className="w-4 h-4 mr-1.5" />
              上传文件
            </Button>
            <div className="h-4 w-px bg-zinc-300 dark:bg-zinc-700" />
            <select
              className="h-7 rounded-md border-0 bg-transparent text-xs text-zinc-500 hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200 cursor-pointer focus:ring-0"
              value={sessionId ? (manualMap[sessionId] || '') : ''}
              onChange={(e) => sessionId && setSessionManualProvider(sessionId, e.target.value || null)}
            >
              <option value="">自动路由</option>
              {providers.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.vendor} · {p.model}
                </option>
              ))}
            </select>
            <div className="h-4 w-px bg-zinc-300 dark:bg-zinc-700" />
            <select
              className="h-7 rounded-md border-0 bg-transparent text-xs text-zinc-500 hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200 cursor-pointer focus:ring-0"
              value={priority}
              onChange={(e) => setPriority(e.target.value as 'speed' | 'quality' | 'balanced')}
            >
              <option value="balanced">平衡</option>
              <option value="speed">速度优先</option>
              <option value="quality">质量优先</option>
            </select>
            <div className="h-4 w-px bg-zinc-300 dark:bg-zinc-700" />

            {/* RAG配置按钮 */}
            <button
              onClick={onShowRagConfig}
              className={clsx(
                "flex items-center gap-1.5 px-2 py-1 rounded-md text-xs transition-colors",
                ragConfig
                  ? "bg-amber-500/20 text-amber-600 dark:text-amber-400"
                  : "text-zinc-500 hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200"
              )}
              title="配置RAG检索策略"
            >
              <BookOpen className="w-3.5 h-3.5" />
              <span>RAG</span>
              {ragConfig && <span className="w-1.5 h-1.5 rounded-full bg-amber-500" />}
            </button>

            <div className="h-4 w-px bg-zinc-300 dark:bg-zinc-700" />

            {/* Draft Mode Switch */}
            <DraftModeSwitch isDraftMode={isDraftMode} onToggle={onToggleDraftMode} />

            <div className="h-4 w-px bg-zinc-300 dark:bg-zinc-700" />

            {/* 手动控制开关 */}
            <ManualControlSwitch
              isManualMode={isManualMode}
              toggleManualMode={toggleManualMode}
              manualLayer={manualLayer}
              setManualLayerSelected={setManualLayerSelected}
              manualParadigm={manualParadigm}
              setManualParadigmSelected={setManualParadigmSelected}
            />
          </div>
          <span className="text-[10px] text-zinc-400 font-medium hidden sm:inline-block">
            Shift + Enter 换行
          </span>
        </div>

        {/* 输入框 */}
        <div className="relative bg-zinc-100/95 dark:bg-zinc-900/95 border border-zinc-300 dark:border-zinc-700 rounded-2xl shadow-xl shadow-zinc-200/30 dark:shadow-black/30 focus-within:ring-2 focus-within:ring-blue-500/20 focus-within:border-blue-500/50 transition-all">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                onSend();
              }
            }}
            placeholder={isDraftMode ? "Describe what to draft..." : "Message Crablet..."}
            disabled={isThinking}
            className="w-full bg-transparent border-none text-zinc-900 dark:text-zinc-100 placeholder:text-zinc-500 dark:placeholder:text-zinc-500 focus:ring-0 resize-none max-h-48 min-h-[56px] py-4 px-4 text-[15px] leading-relaxed scrollbar-thin scrollbar-thumb-zinc-400 dark:scrollbar-thumb-zinc-700 rounded-2xl"
            rows={1}
            style={{ height: 'auto', minHeight: '56px' }}
          />
          <div className="absolute bottom-3 right-3">
            <Button
              onClick={onSend}
              disabled={isThinking || !input.trim()}
              size="icon"
              className={clsx(
                "h-9 w-9 rounded-xl transition-all duration-200",
                input.trim()
                  ? (isDraftMode ? "bg-emerald-600 hover:bg-emerald-700 text-white shadow-md shadow-emerald-500/20" : "bg-blue-600 hover:bg-blue-700 text-white shadow-md shadow-blue-500/20")
                  : "bg-zinc-200 dark:bg-zinc-800 text-zinc-400 dark:text-zinc-500"
              )}
            >
              {isThinking ? <StopCircle className="w-4 h-4" /> : <Send className="w-4 h-4" />}
            </Button>
          </div>
        </div>

        {/* 附件列表 */}
        {attachments.length > 0 && (
          <AttachmentList attachments={attachments} onArchive={onArchiveAttachment} />
        )}

        {/* 知识检索结果 */}
        {(retrieving || retrievalHits.length > 0) && (
          <RetrievalResults
            retrieving={retrieving}
            retrievalHits={retrievalHits}
            selectedRetrieval={selectedRetrieval}
            setSelectedRetrieval={setSelectedRetrieval}
            getRetrievalSource={getRetrievalSource}
          />
        )}

        {/* 底部提示 */}
        <div className="text-center text-[11px] text-zinc-400 dark:text-zinc-600 font-medium">
          Crablet can make mistakes. Consider checking important information.
        </div>
      </div>
    </div>
  );
};

// --- Inline sub-components ---

function DraftModeSwitch({ isDraftMode, onToggle }: { isDraftMode: boolean; onToggle: () => void }) {
  return (
    <button
      onClick={onToggle}
      className={clsx(
        "flex items-center gap-1.5 px-2 py-1 rounded-md text-xs transition-colors",
        isDraftMode
          ? "bg-emerald-500/20 text-emerald-600 dark:text-emerald-400"
          : "text-zinc-500 hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200"
      )}
      title="Draft Mode: High-quality drafting & background refinement"
    >
      <PenLine className="w-3.5 h-3.5" />
      <span>草稿模式</span>
      <span className={clsx("w-1.5 h-1.5 rounded-full", isDraftMode ? "bg-emerald-500" : "bg-zinc-400")} />
    </button>
  );
}

function ManualControlSwitch({
  isManualMode,
  toggleManualMode,
  manualLayer,
  setManualLayerSelected,
  manualParadigm,
  setManualParadigmSelected,
}: {
  isManualMode: boolean;
  toggleManualMode: (v: boolean) => void;
  manualLayer: CognitiveLayer;
  setManualLayerSelected: (v: CognitiveLayer) => void;
  manualParadigm: AgentParadigm;
  setManualParadigmSelected: (v: AgentParadigm) => void;
}) {
  return (
    <div className="flex items-center gap-2">
      <button
        onClick={() => toggleManualMode(!isManualMode)}
        className={clsx(
          "flex items-center gap-1.5 px-2 py-1 rounded-md text-xs transition-colors",
          isManualMode
            ? "bg-blue-500/20 text-blue-600 dark:text-blue-400"
            : "text-zinc-500 hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200"
        )}
        title="手动选择思考系统和Agent范式"
      >
        <Settings className="w-3.5 h-3.5" />
        <span>手动</span>
        <span className={clsx("w-1.5 h-1.5 rounded-full", isManualMode ? "bg-blue-500" : "bg-zinc-400")} />
      </button>
      {isManualMode && (
        <>
          <select
            className="h-7 rounded-md border-0 bg-transparent text-xs cursor-pointer focus:ring-0"
            value={manualLayer}
            onChange={(e) => setManualLayerSelected(e.target.value as CognitiveLayer)}
            style={{
              color: manualLayer === 'system1' ? '#eab308' : manualLayer === 'system2' ? '#3b82f6' : manualLayer === 'system3' ? '#a855f7' : '#6b7280'
            }}
          >
            <option value="system1">System 1</option>
            <option value="system2">System 2</option>
            <option value="system3">System 3</option>
          </select>
          <select
            className="h-7 rounded-md border-0 bg-transparent text-xs text-zinc-500 hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200 cursor-pointer focus:ring-0"
            value={manualParadigm}
            onChange={(e) => setManualParadigmSelected(e.target.value as AgentParadigm)}
          >
            <option value="single-turn">Single-Turn</option>
            <option value="react">ReAct</option>
            <option value="reflexion">Reflexion</option>
            <option value="plan-and-execute">Plan & Execute</option>
            <option value="swarm">Swarm</option>
          </select>
        </>
      )}
    </div>
  );
}

function AttachmentList({ attachments, onArchive }: { attachments: PendingAttachment[]; onArchive: (a: PendingAttachment) => void }) {
  return (
    <div className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white/70 dark:bg-zinc-900/60 p-2 space-y-1">
      {attachments.map((a) => (
        <div key={a.id} className="flex items-center justify-between gap-2 text-xs">
          <div className="flex items-center gap-2 flex-1 min-w-0">
            <div className="truncate">{a.file.name}</div>
            {a.isOcr && (
              <span className="px-1.5 py-0.5 bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-400 rounded text-[10px]">
                OCR
              </span>
            )}
          </div>
          <div className="flex items-center gap-2 shrink-0">
            {a.status === 'processing' && a.ocrProgress !== undefined && (
              <span className="text-amber-600 dark:text-amber-400">OCR {a.ocrProgress}%</span>
            )}
            <span className={clsx(
              a.status === 'failed' ? 'text-red-500' :
              a.status === 'processing' ? 'text-amber-500' :
              'text-zinc-500'
            )}>
              {a.status === 'uploading' ? `${a.progress}%` :
               a.status === 'processing' ? '处理中' :
               a.status}
            </span>
            {a.status !== 'uploaded' && a.status !== 'processing' && (
              <Button size="sm" variant="secondary" onClick={() => onArchive(a)}>
                添加到知识库
              </Button>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}

function RetrievalResults({
  retrieving,
  retrievalHits,
  selectedRetrieval,
  setSelectedRetrieval,
  getRetrievalSource,
}: {
  retrieving: boolean;
  retrievalHits: RetrievalHit[];
  selectedRetrieval: number[];
  setSelectedRetrieval: React.Dispatch<React.SetStateAction<number[]>>;
  getRetrievalSource: (hit: RetrievalHit) => string;
}) {
  return (
    <div className="rounded-xl border border-zinc-200 dark:border-zinc-800 bg-white/70 dark:bg-zinc-900/60 p-2 space-y-1">
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
            source: {getRetrievalSource(r)} · score: {Number(r.score || 0).toFixed(3)}
          </div>
          <div className="text-zinc-700 dark:text-zinc-300 line-clamp-2">
            {String(r.content || '').slice(0, 180)}
          </div>
        </div>
      ))}
    </div>
  );
}
