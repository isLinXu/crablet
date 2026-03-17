import { useState, useRef, useEffect, useMemo } from 'react';
import { useChatStore } from '../../store/chatStore';
import type { ExtendedMessage } from '../../store/chatStore';
import { useWebSocket } from '../../hooks/useWebSocket';
import { useStreamingChat } from '../../hooks/useStreamingChat';
import { useKeyboard } from '../../hooks/useKeyboard';
import { Send, Bot, Loader2, StopCircle, History, X, PlusCircle, Upload, Workflow as WorkflowIcon, Download, Upload as UploadIcon, Settings, BookOpen, PenLine } from 'lucide-react';
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso';
import { Button } from '../ui/Button';
import { MessageBubble } from './MessageBubble';
import { SessionList } from './SessionList';
import { SkillSlots } from './SkillSlots';
import { CrabEmptyState, CrabThinking } from '../ui/CrabElements';
import clsx from 'clsx';
import { cognitiveLayerLabel, inferCognitiveLayer, type CognitiveLayer } from '@/utils/cognitive';
import { useModelStore } from '@/store/modelStore';
import { validateFile, heuristicSecurityScan, computeFileHash, extractTagsByName } from '@/utils/filePipeline';
import { extractFileContent, getFileTypeDescription } from '@/utils/fileContentExtractor';
import { knowledgeService } from '@/services/knowledgeService';
import { archiveIndexService } from '@/services/archiveIndexService';
import toast from 'react-hot-toast';
import { LOCAL_STORAGE_KEYS } from '@/utils/constants';
import { convertChatToCanvas, downloadWorkflow, readWorkflowFromFile } from '@/utils/chatToCanvas';
import type { Workflow } from '@/utils/chatToCanvas';
import { useNavigate } from 'react-router-dom';
import { useAgentThinking } from '@/hooks/useAgentThinking';
import type { ThinkingProcess } from './EnhancedThinkingVisualization';
import { EnhancedThinkingVisualization } from './EnhancedThinkingVisualization';
import type { InterventionRequest } from '../cognitive/ThinkingIntervention';
import type { Suggestion } from '../cognitive/SmartSuggestions';
import { RagConfigPanel, type RagConfig } from '../rag/RagConfigPanel';

interface PendingAttachment {
  id: string;
  file: File;
  progress: number;
  status: 'pending' | 'uploading' | 'uploaded' | 'failed' | 'processing';
  hash?: string;
  isOcr?: boolean; // 标记是否使用了OCR
  ocrProgress?: number; // OCR进度
}
interface RetrievalHit {
  content: string;
  score: number;
  metadata?: Record<string, any>;
}

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
    setDraftMode
  } = useChatStore();
  const { systemLogs = [] } = useWebSocket('ws://localhost:8080/ws/logs') as any;
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
    
    // Optimistic update: Add user message immediately
    // Note: Actual addMessage is handled by store/hook, but for "real-time" feel we rely on store update.
    // useStreamingChat calls addMessage internally.
    
    // 读取已上传文件的内容（最大约 100万 tokens ≈ 5MB 文本内容）
    // 包含所有附件，无论是否已归档到知识库
    const allAttachments = attachments.filter((a) => a.status !== 'failed');
    let fileContents = '';
    const MAX_FILE_CONTENT_LENGTH = 5 * 1024 * 1024; // 5MB，约支持 100万+ tokens
    
    console.log(`[Chat] 处理 ${allAttachments.length} 个附件...`);
    
    for (const attachment of allAttachments) {
      try {
        // 更新状态为处理中
        setAttachments(prev => prev.map(a => 
          a.id === attachment.id ? { ...a, status: 'processing' } : a
        ));

        console.log(`[Chat] 提取文件内容: ${attachment.file.name} (${attachment.file.size} bytes)`);
        
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

        console.log(`[Chat] 文件提取结果: success=${result.success}, length=${result.text.length}, isOcr=${result.isOcr}`);

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
    
    // 构建最终 prompt，包含文件内容
    let finalPrompt = input;
    
    // Draft Mode Override
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
    
    // 调试：输出最终prompt的长度和结构
    console.log(`[Chat] 最终Prompt长度: ${finalPrompt.length} 字符`);
    console.log(`[Chat] 文件内容长度: ${fileContents.length} 字符`);
    console.log(`[Chat] 检索上下文长度: ${retrievalSummary.length} 字符`);
    
    // 显示文件内容预览（前500字符）
    if (fileContents) {
      console.log(`[Chat] 文件内容预览:\n${fileContents.slice(0, 500)}...`);
    }
    
    setInput(''); // Clear input immediately for responsiveness
    setAttachments([]); // Clear attachments
    setRetrievalHits([]); // Clear retrieval hits
    
    await sendMessage(
      finalPrompt,
      picked.map((r) => ({
        source: r.metadata?.source || r.metadata?.source_trace || 'unknown',
        score: Number(r.score || 0),
        snippet: String(r.content || '').slice(0, 240),
      }))
    );
  };

  const handleNewChat = () => {
    createSession('New Chat');
    setInput('');
  };

  const navigate = useNavigate();
  const fileInputWorkflowRef = useRef<HTMLInputElement>(null);

  // Convert current chat to Canvas workflow
  const handleConvertToCanvas = () => {
    if (messages.length === 0) {
      toast.error('No messages to convert');
      return;
    }

    try {
      const workflow = convertChatToCanvas(messages, {
        workflowName: sessionId ? `Chat Workflow - ${sessionId.slice(0, 8)}` : 'Chat Workflow',
      });
      
      // Store workflow in localStorage for Canvas to pick up
      localStorage.setItem('pendingWorkflow', JSON.stringify(workflow));
      
      toast.success('Chat converted to workflow! Opening Canvas...');
      navigate('/canvas');
    } catch (error) {
      toast.error(`Failed to convert: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
  };

  // Export current chat as workflow JSON
  const handleExportChat = () => {
    if (messages.length === 0) {
      toast.error('No messages to export');
      return;
    }

    try {
      const workflow = convertChatToCanvas(messages, {
        workflowName: sessionId ? `Chat Workflow - ${sessionId.slice(0, 8)}` : 'Chat Workflow',
      });
      downloadWorkflow(workflow);
      toast.success('Workflow exported successfully');
    } catch (error) {
      toast.error(`Failed to export: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
  };

  // Import workflow from file
  const handleImportWorkflow = async (files: FileList | null) => {
    if (!files?.length) return;
    
    const file = files[0];
    if (!file.name.endsWith('.json')) {
      toast.error('Please select a JSON file');
      return;
    }

    try {
      const workflow = await readWorkflowFromFile(file);
      localStorage.setItem('pendingWorkflow', JSON.stringify(workflow));
      toast.success('Workflow imported! Opening Canvas...');
      navigate('/canvas');
    } catch (error) {
      toast.error(`Failed to import: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
    
    if (fileInputWorkflowRef.current) {
      fileInputWorkflowRef.current.value = '';
    }
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
  
  // 使用 useMemo 缓存 resolvedModel，避免每次渲染都创建新对象
  const resolvedModel = useMemo(() => 
    resolveForPrompt(sessionId, input || 'general', priority),
    [sessionId, priority, resolveForPrompt] // 注意：不依赖 input，避免输入时频繁重新计算
  );
  
  // 使用新的 Agent Thinking Hook
  const {
    process: thinkingProcess,
    isThinking: isAgentThinking,
    isManualMode,
    manualLayer,
    manualParadigm,
    startThinking,
    endThinking,
    addRoutingStep,
    addSystemStep,
    addParadigmStep,
    addReasoningStep,
    addToolCallStep,
    completeToolCall,
    addConfidenceStep,
    switchLayer,
    switchParadigm,
    pushStack,
    popStack,
    toggleManualMode,
    setManualLayerSelected,
    setManualParadigmSelected,
  } = useAgentThinking({
    sessionId,
    model: resolvedModel?.model || 'unknown',
    vendor: resolvedModel?.vendor || 'unknown',
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

  // 使用新的 Agent Thinking Hook 生成思考过程
  useEffect(() => {
    if (isThinking && messages.length > 0) {
      const lastMessage = messages[messages.length - 1];
      if (lastMessage?.role === 'assistant') {
        // 开始新的思考过程
        startThinking();
        
        // 添加路由选择步骤
        if (resolvedModel) {
          addRoutingStep(
            resolvedModel.providerId,
            resolvedModel.model,
            resolvedModel.vendor,
            resolvedModel.reason,
            0.5 // 复杂度评分
          );
        }
        
        // 添加系统选择步骤 - 手动模式下使用手动选择的值
        // 如果没有明确的认知层，基于消息内容推断
        const inferLayerFromMessage = (msg: string): CognitiveLayer => {
          const lower = msg.toLowerCase();
          // 问候语检测
          const greetings = ['你好', '您好', '嗨', 'hello', 'hi', 'hey', '早上好', '下午好', '晚上好', '在吗', '在么'];
          if (greetings.some(g => lower.trim() === g || lower.startsWith(g + ' '))) {
            return 'system1';
          }
          // 人设/身份查询
          const personaPatterns = [
            '你是谁', '你是什么', '你叫什么', '介绍一下', '你是干嘛的', '你是做什么的',
            '你的身份', '你的角色', '你是ai吗', '你是人工智能吗', '你是机器人吗',
            'who are you', 'what are you', 'your name', 'introduce yourself', 'tell me about yourself'
          ];
          if (personaPatterns.some(p => lower.includes(p))) {
            return 'system1';
          }
          // 闲聊/社交对话
          const chatPatterns = [
            '你好吗', '最近怎么样', '很高兴认识你', '谢谢', '多谢', '哈哈', '呵呵', '嘿嘿',
            'how are you', 'what\'s up', 'how\'s it going', 'nice to meet you', 'thank you', 'thanks'
          ];
          if (chatPatterns.some(p => lower.trim() === p || lower.startsWith(p))) {
            return 'system1';
          }
          // 简单个人问题
          const personalPatterns = [
            '你多大了', '你几岁了', '你喜欢什么', '你的爱好', '你喜欢', 'how old are you',
            'where are you from', 'what do you like', 'your favorite'
          ];
          if (personalPatterns.some(p => lower.includes(p))) {
            return 'system1';
          }
          // 简单帮助请求
          if (lower.includes('help') || lower.includes('帮助') || lower.includes('怎么用') || lower.includes('如何使用')) {
            return 'system1';
          }
          // 默认使用 system2
          return 'system2';
        };
        
        const lastUserMsg = messages[messages.length - 2]?.content as string || '';
        const effectiveLayer = isManualMode 
          ? manualLayer 
          : (currentLayer !== 'unknown' ? currentLayer : inferLayerFromMessage(lastUserMsg));
        const systemPrompts: Record<string, string> = {
          system1: '快速直觉响应模式 - 适用于简单直接的问题',
          system2: '深度分析推理模式 - 适用于需要逻辑思考的问题',
          system3: '元认知反思模式 - 适用于复杂的多步骤任务',
        };
        addSystemStep(
          effectiveLayer,
          systemPrompts[effectiveLayer] || '默认系统提示',
          isManualMode ? '手动选择' : (currentLayer !== 'unknown' ? '基于问题复杂度自动选择' : '使用默认系统')
        );
        
        // 切换认知层
        switchLayer(
          effectiveLayer,
          isManualMode ? `手动切换至 ${effectiveLayer}` : (currentLayer !== 'unknown' ? `自动切换至 ${effectiveLayer}` : `默认使用 ${effectiveLayer}`),
          isManualMode ? 'manual-override' : 'complexity-analysis',
          0.85
        );
        
        // 添加范式选择步骤 - 手动模式下使用手动选择的值
        const paradigm: any = isManualMode ? manualParadigm : (
                            effectiveLayer === 'system1' ? 'single-turn' : 
                            effectiveLayer === 'system2' ? 'react' : 
                            effectiveLayer === 'system3' ? 'reflexion' : 'react'
        );
        addParadigmStep(
          paradigm,
          isManualMode ? `手动选择 ${paradigm} 范式` : `基于 ${effectiveLayer} 认知层选择对应范式`
        );
        switchParadigm(paradigm, isManualMode ? '手动选择范式' : `切换至 ${paradigm} 范式`, isManualMode ? 'manual-override' : 'layer-paradigm-mapping');
        
        // 添加调用栈帧 - 模拟函数调用过程
        const processMessageFrame = pushStack('processMessage', { 
          sessionId, 
          messageLength: lastMessage.content?.length || 0,
          layer: effectiveLayer,
          paradigm,
          isManualMode,
          manualLayer,
          manualParadigm
        });
        
        const inferenceFrame = pushStack('inference.generate', { 
          model: resolvedModel?.model,
          provider: resolvedModel?.providerId,
          temperature: 0.7,
          maxTokens: 2048
        });
        
        // 添加来自 trace 的步骤
        if (lastMessage.traceSteps) {
          lastMessage.traceSteps.forEach((trace) => {
            addReasoningStep(
              trace.thought,
              trace.action,
              trace.observation
            );
          });
        }
        
        // 完成调用栈帧
        popStack(inferenceFrame, { tokens: lastMessage.content?.length || 0, status: 'success' });
        popStack(processMessageFrame, { completed: true, totalSteps: lastMessage.traceSteps?.length || 0 });
      }
    } else if (!isThinking && isAgentThinking) {
      // 思考结束
      endThinking();
    }
  }, [isThinking, messages, resolvedModel, currentLayer, isManualMode, manualLayer, manualParadigm, startThinking, endThinking, addRoutingStep, addSystemStep, addParadigmStep, addReasoningStep, switchLayer, switchParadigm, isAgentThinking, pushStack, popStack]);

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
                <h1 className="text-sm font-semibold text-zinc-900 dark:text-zinc-100 leading-none tracking-tight">Crablet</h1>
                <p className="text-xs text-zinc-500 dark:text-zinc-400 mt-1 flex items-center gap-1.5 font-medium">
                    <span className={clsx("w-2 h-2 rounded-full", isConnected ? "bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.4)]" : "bg-rose-500")}></span>
                    {isConnected ? 'Online' : 'Offline'} · {cognitiveLayerLabel(currentLayer)} · {resolvedModel.vendor}
                </p>
            </div>
          </div>
        </div>
        
        <div className="flex items-center gap-2">
            {/* Hidden file input for workflow import */}
            <input 
                ref={fileInputWorkflowRef} 
                type="file" 
                accept=".json" 
                className="hidden" 
                onChange={(e) => handleImportWorkflow(e.target.files)} 
            />
            
            {/* Import Workflow Button */}
            <Button 
                variant="ghost" 
                size="sm" 
                onClick={() => fileInputWorkflowRef.current?.click()}
                className="text-zinc-600 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-800 rounded-lg transition-all"
                title="Import Workflow"
            >
                <UploadIcon className="w-4 h-4 mr-2" />
                Import
            </Button>
            
            {/* Export Chat as Workflow Button */}
            <Button 
                variant="ghost" 
                size="sm" 
                onClick={handleExportChat}
                disabled={messages.length === 0}
                className="text-zinc-600 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-800 rounded-lg transition-all disabled:opacity-50"
                title="Export as Workflow JSON"
            >
                <Download className="w-4 h-4 mr-2" />
                Export
            </Button>
            
            {/* Convert to Canvas Button */}
            <Button 
                variant="ghost" 
                size="sm" 
                onClick={handleConvertToCanvas}
                disabled={messages.length === 0}
                className="text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/20 rounded-lg transition-all disabled:opacity-50"
                title="Convert Chat to Canvas Workflow"
            >
                <WorkflowIcon className="w-4 h-4 mr-2" />
                To Canvas
            </Button>
            
            <div className="w-px h-6 bg-zinc-300 dark:bg-zinc-700 mx-1" />
            
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
                    // 使用 useMemo 缓存计算结果，避免每次渲染都重新计算
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
                                // 只传递必要的历史记录，避免传递整个 messages 数组
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

      {/* Input Area - 分离布局 */}
      <div className="p-4 bg-transparent shrink-0 z-10">
        <div className="max-w-4xl mx-auto space-y-3">
            {/* 技能插槽预览 */}
            <SkillSlots
                skills={[
                    { id: '1', name: '代码审查', icon: 'code', description: '分析代码质量和潜在问题', color: 'blue' },
                    { id: '2', name: '文档生成', icon: 'file', description: '自动生成代码文档', color: 'emerald' },
                    { id: '3', name: '智能搜索', icon: 'search', description: '深度检索知识库', color: 'amber' },
                ]}
                onSkillAdd={() => toast('技能选择器开发中...', { icon: '🔧' })}
            />
            
            {/* 设置条 - 独立显示在输入框上方 */}
            <div className="flex items-center justify-between px-1">
                <div className="flex items-center gap-2">
                    <input ref={fileInputRef} type="file" multiple className="hidden" onChange={(e) => handlePickFiles(e.target.files)} />
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
                        onChange={(e) => setPriority(e.target.value as any)}
                    >
                        <option value="balanced">平衡</option>
                        <option value="speed">速度优先</option>
                        <option value="quality">质量优先</option>
                    </select>
                    <div className="h-4 w-px bg-zinc-300 dark:bg-zinc-700" />
                    
                    {/* RAG配置按钮 */}
                    <button
                        onClick={() => setShowRagConfig(true)}
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
                    <button
                        onClick={() => setDraftMode(!isDraftMode)}
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
                        <span className={clsx(
                            "w-1.5 h-1.5 rounded-full",
                            isDraftMode ? "bg-emerald-500" : "bg-zinc-400"
                        )} />
                    </button>
                    
                    <div className="h-4 w-px bg-zinc-300 dark:bg-zinc-700" />
                    
                    {/* 手动控制开关 */}
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
                            <span className={clsx(
                                "w-1.5 h-1.5 rounded-full",
                                isManualMode ? "bg-blue-500" : "bg-zinc-400"
                            )} />
                        </button>
                        
                        {/* 手动模式下的系统选择 */}
                        {isManualMode && (
                            <>
                                <select
                                    className="h-7 rounded-md border-0 bg-transparent text-xs cursor-pointer focus:ring-0"
                                    value={manualLayer}
                                    onChange={(e) => setManualLayerSelected(e.target.value as any)}
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
                                    onChange={(e) => setManualParadigmSelected(e.target.value as any)}
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
                </div>
                <span className="text-[10px] text-zinc-400 font-medium hidden sm:inline-block">
                    Shift + Enter 换行
                </span>
            </div>
            
            {/* 输入框 - 独立容器 */}
            <div className="relative bg-zinc-100/95 dark:bg-zinc-900/95 border border-zinc-300 dark:border-zinc-700 rounded-2xl shadow-xl shadow-zinc-200/30 dark:shadow-black/30 focus-within:ring-2 focus-within:ring-blue-500/20 focus-within:border-blue-500/50 transition-all">
                <textarea
                    value={input}
                    onChange={(e) => setInput(e.target.value)}
                    onKeyDown={(e) => {
                        if (e.key === 'Enter' && !e.shiftKey) {
                            e.preventDefault();
                            handleSend();
                        }
                    }}
                    placeholder={isDraftMode ? "Describe what to draft..." : "Message Crablet..."}
                    disabled={isThinking}
                    className="w-full bg-transparent border-none text-zinc-900 dark:text-zinc-100 placeholder:text-zinc-500 dark:placeholder:text-zinc-500 focus:ring-0 resize-none max-h-48 min-h-[56px] py-4 px-4 text-[15px] leading-relaxed scrollbar-thin scrollbar-thumb-zinc-400 dark:scrollbar-thumb-zinc-700 rounded-2xl"
                    rows={1}
                    style={{ height: 'auto', minHeight: '56px' }} 
                />
                {/* 发送按钮 - 绝对定位在右下角 */}
                <div className="absolute bottom-3 right-3">
                    <Button
                        onClick={handleSend}
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
                        <span className="text-amber-600 dark:text-amber-400">
                          OCR {a.ocrProgress}%
                        </span>
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
                        <Button size="sm" variant="secondary" onClick={() => archiveAttachment(a)}>
                          添加到知识库
                        </Button>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
            
            {/* 知识检索结果 */}
            {(retrieving || retrievalHits.length > 0) && (
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
                      source: {r.metadata?.source || r.metadata?.source_trace || 'unknown'} · score: {Number(r.score || 0).toFixed(3)}
                    </div>
                    <div className="text-zinc-700 dark:text-zinc-300 line-clamp-2">
                      {String(r.content || '').slice(0, 180)}
                    </div>
                  </div>
                ))}
              </div>
            )}
            
            {/* 底部提示 */}
            <div className="text-center text-[11px] text-zinc-400 dark:text-zinc-600 font-medium">
                Crablet can make mistakes. Consider checking important information.
            </div>
        </div>
      </div>
      
      {/* RAG配置面板 */}
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
