import React from 'react';
import { useChatStore } from '../../store/chatStore';
import { Button } from '../ui/Button';
import { MessageSquare, Plus, Trash2, Pencil, Download, CheckSquare, Square } from 'lucide-react';
import { cn } from '../ui/Button';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { format } from 'date-fns';

interface SessionListProps {
  className?: string;
  onSelect?: () => void;
}

const sanitizeFileName = (name: string) => name.replace(/[\\/:*?"<>|]/g, '-').trim() || 'chat';

const contentToText = (content: any) => {
  if (typeof content === 'string') return content;
  if (!Array.isArray(content)) return String(content ?? '');
  return content
    .map((part) => {
      if (part?.type === 'text') return part.text || '';
      if (part?.type === 'image_url') return `![image](${part.image_url?.url || ''})`;
      return '';
    })
    .filter(Boolean)
    .join('\n');
};

const roleLabel = (role: string) => {
  if (role === 'user') return '用户';
  if (role === 'assistant') return '助手';
  if (role === 'system') return '系统';
  if (role === 'tool') return '工具';
  return role;
};

const buildSessionMarkdown = (session: { id: string; title: string; created_at: string; updated_at: string }, messages: any[]) => {
  const header = [
    `# ${session.title || 'Untitled Chat'}`,
    '',
    `- Session ID: ${session.id}`,
    `- Created At: ${session.created_at}`,
    `- Updated At: ${session.updated_at}`,
    `- Exported At: ${new Date().toISOString()}`,
    '',
    '---',
    ''
  ].join('\n');

  const body = messages.map((msg, index) => {
    const text = contentToText(msg.content);
    const ts = msg.timestamp ? ` (${msg.timestamp})` : '';
    return `## ${index + 1}. ${roleLabel(msg.role)}${ts}\n\n${text}\n`;
  }).join('\n');

  return `${header}${body}`.trim() + '\n';
};

export const SessionList: React.FC<SessionListProps> = ({ className, onSelect }) => {
  const {
    sessionId,
    sessions,
    setSessionId,
    createSession,
    renameSession,
    deleteSessions,
    getMessagesBySession
  } = useChatStore();
  const [deleteId, setDeleteId] = React.useState<string | null>(null);
  const [selectedIds, setSelectedIds] = React.useState<string[]>([]);
  const [renamingId, setRenamingId] = React.useState<string | null>(null);
  const [renameText, setRenameText] = React.useState('');

  const handleSelectSession = (id: string) => {
    if (id === sessionId) {
        onSelect?.();
        return;
    }
    setSessionId(id);
    onSelect?.();
  };

  const handleNewSession = () => {
    const id = createSession('New Chat');
    setSessionId(id);
  };

  const handleDeleteSession = () => {
    if (!deleteId) return;
    deleteSessions([deleteId]);
    setSelectedIds((prev) => prev.filter((id) => id !== deleteId));
    setDeleteId(null);
  };

  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => (prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]));
  };

  const toggleSelectAll = () => {
    if (selectedIds.length === sessions.length) {
      setSelectedIds([]);
      return;
    }
    setSelectedIds(sessions.map((s) => s.id));
  };

  const handleBatchDelete = () => {
    if (selectedIds.length === 0) return;
    deleteSessions(selectedIds);
    setSelectedIds([]);
  };

  const startRename = (id: string, title: string) => {
    setRenamingId(id);
    setRenameText(title);
  };

  const confirmRename = () => {
    if (!renamingId) return;
    const name = renameText.trim();
    if (!name) return;
    renameSession(renamingId, name);
    setRenamingId(null);
    setRenameText('');
  };

  const exportSession = (id: string) => {
    const session = sessions.find((s) => s.id === id);
    if (!session) return;
    const messages = getMessagesBySession(id);
    const markdown = buildSessionMarkdown(session, messages);
    const blob = new Blob([markdown], { type: 'text/markdown;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${sanitizeFileName(session.title || 'chat')}-${id}.md`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const exportSelected = () => {
    const chunks = selectedIds
      .map((id) => {
        const session = sessions.find((s) => s.id === id);
        if (!session) return '';
        const messages = getMessagesBySession(id);
        return buildSessionMarkdown(session, messages);
      })
      .filter(Boolean);
    const merged = chunks.join('\n\n---\n\n');
    const blob = new Blob([merged], { type: 'text/markdown;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `chat-sessions-${Date.now()}.md`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div
      className={cn("box-border flex flex-col h-full w-full min-w-0 overflow-x-hidden bg-zinc-950 text-zinc-100 border-r border-zinc-800", className)}
      style={{ width: '100%', minWidth: '100%' }}
    >
      <div className="p-4 space-y-2">
        <Button onClick={handleNewSession} className="w-full justify-start" variant="primary">
          <Plus className="mr-2 h-4 w-4" />
          New Chat
        </Button>
        <div className="grid grid-cols-3 gap-2">
          <Button onClick={toggleSelectAll} variant="secondary" size="sm" className="flex-1 justify-center">
            {selectedIds.length === sessions.length && sessions.length > 0 ? <CheckSquare className="mr-1 h-4 w-4" /> : <Square className="mr-1 h-4 w-4" />}
            全选
          </Button>
          <Button onClick={exportSelected} disabled={selectedIds.length === 0} variant="secondary" size="sm" className="flex-1 justify-center">
            <Download className="mr-1 h-4 w-4" />
            导出MD
          </Button>
          <Button onClick={handleBatchDelete} disabled={selectedIds.length === 0} variant="danger" size="sm" className="flex-1 justify-center">
            <Trash2 className="mr-1 h-4 w-4" />
            删除
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-2 space-y-1">
        {sessions.length === 0 ? (
          <div className="text-center p-4 text-sm text-gray-500">
            No history
          </div>
        ) : (
          sessions.map((session) => (
            (() => {
              const date = new Date(session.updated_at);
              const timeText = Number.isNaN(date.getTime()) ? '--/-- --:--' : format(date, 'MM/dd HH:mm');
              return (
            <div
              key={session.id}
              className={cn(
                "group flex items-center justify-between rounded-md px-3 py-2 text-sm font-medium transition-colors hover:bg-gray-100 dark:hover:bg-gray-800 cursor-pointer",
                sessionId === session.id ? "bg-blue-50 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300" : "text-gray-800 dark:text-gray-200"
              )}
              onClick={() => handleSelectSession(session.id)}
            >
              <div className="flex items-center overflow-hidden gap-2">
                <button
                  className="p-0.5"
                  onClick={(e) => {
                    e.stopPropagation();
                    toggleSelect(session.id);
                  }}
                >
                  {selectedIds.includes(session.id) ? <CheckSquare className="h-4 w-4" /> : <Square className="h-4 w-4 opacity-60" />}
                </button>
                <MessageSquare className="h-4 w-4 shrink-0" />
                <div className="flex flex-col truncate">
                    {renamingId === session.id ? (
                      <input
                        value={renameText}
                        onChange={(e) => setRenameText(e.target.value)}
                        onBlur={confirmRename}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') confirmRename();
                        }}
                        autoFocus
                        className="bg-transparent border border-gray-300 dark:border-gray-600 rounded px-1 text-sm"
                      />
                    ) : (
                      <span className="truncate">{session.title || 'Untitled Chat'}</span>
                    )}
                    <span className="text-[10px] text-gray-500 dark:text-gray-400">{timeText}</span>
                </div>
              </div>
              <div className="opacity-0 group-hover:opacity-100 flex items-center gap-1 transition-opacity">
                <button
                  className="p-1 hover:text-blue-500"
                  onClick={(e) => {
                    e.stopPropagation();
                    startRename(session.id, session.title || 'Untitled Chat');
                  }}
                >
                  <Pencil className="h-3 w-3" />
                </button>
                <button
                  className="p-1 hover:text-emerald-500"
                  onClick={(e) => {
                    e.stopPropagation();
                    exportSession(session.id);
                  }}
                >
                  <Download className="h-3 w-3" />
                </button>
                <button
                  className="p-1 hover:text-red-500"
                  onClick={(e) => {
                    e.stopPropagation();
                    setDeleteId(session.id);
                  }}
                >
                  <Trash2 className="h-3 w-3" />
                </button>
              </div>
            </div>
              );
            })()
          ))
        )}
      </div>

      <ConfirmDialog
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDeleteSession}
        title="Delete Chat"
        description="Are you sure you want to delete this chat session? This action cannot be undone."
        variant="danger"
      />
    </div>
  );
};
