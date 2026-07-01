import { Button } from '../ui/Button';
import { Bot, History, PlusCircle, Upload as UploadIcon, Download, Workflow as WorkflowIcon } from 'lucide-react';
import clsx from 'clsx';
import { cognitiveLayerLabel, type CognitiveLayer } from '@/utils/cognitive';
import { useRef } from 'react';
import toast from 'react-hot-toast';
import { convertChatToCanvas, downloadWorkflow, readWorkflowFromFile } from '@/utils/chatToCanvas';
import { useNavigate } from 'react-router-dom';
import type { ExtendedMessage } from '../../../store/chatStore';

interface ChatHeaderProps {
  isConnected: boolean;
  currentLayer: CognitiveLayer;
  vendor: string;
  messages: ExtendedMessage[];
  sessionId: string | null;
  onNewChat: () => void;
  onShowMobileHistory: () => void;
}

export const ChatHeader = ({
  isConnected,
  currentLayer,
  vendor,
  messages,
  sessionId,
  onNewChat,
  onShowMobileHistory,
}: ChatHeaderProps) => {
  const navigate = useNavigate();
  const fileInputWorkflowRef = useRef<HTMLInputElement>(null);

  const handleConvertToCanvas = () => {
    if (messages.length === 0) {
      toast.error('No messages to convert');
      return;
    }
    try {
      const workflow = convertChatToCanvas(messages, {
        workflowName: sessionId ? `Chat Workflow - ${sessionId.slice(0, 8)}` : 'Chat Workflow',
      });
      localStorage.setItem('pendingWorkflow', JSON.stringify(workflow));
      toast.success('Chat converted to workflow! Opening Canvas...');
      navigate('/canvas');
    } catch (error) {
      toast.error(`Failed to convert: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
  };

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

  return (
    <div className="px-6 py-3 bg-white/80 dark:bg-zinc-900/80 backdrop-blur-md border-b border-zinc-200 dark:border-zinc-800 flex items-center justify-between shrink-0 z-10 sticky top-0">
      <div className="flex items-center gap-3">
        <button
          className="md:hidden p-2 -ml-2 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800 rounded-md"
          onClick={onShowMobileHistory}
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
              {isConnected ? 'Online' : 'Offline'} · {cognitiveLayerLabel(currentLayer)} · {vendor}
            </p>
          </div>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <input
          ref={fileInputWorkflowRef}
          type="file"
          accept=".json"
          className="hidden"
          onChange={(e) => handleImportWorkflow(e.target.files)}
        />
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
          onClick={onNewChat}
          className="text-zinc-600 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-800 rounded-lg transition-all"
        >
          <PlusCircle className="w-4 h-4 mr-2" />
          New Chat
        </Button>
      </div>
    </div>
  );
};
