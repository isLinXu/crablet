import React, { useMemo, useState } from 'react';
import { Brain } from 'lucide-react';
import { useChatStore } from '@/store/chatStore';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import toast from 'react-hot-toast';

export const MemoryCenter: React.FC = () => {
  const sessions = useChatStore((state) => state.sessions);
  const sessionMessages = useChatStore((state) => state.sessionMessages);
  const deleteSessions = useChatStore((state) => state.deleteSessions);
  const [query, setQuery] = useState('');
  const [range, setRange] = useState<'all' | '7d' | '30d' | '90d'>('all');
  const [keepDays, setKeepDays] = useState(30);
  const [exportMode, setExportMode] = useState<'summary' | 'full'>('summary');
  const [exportIncludeContent, setExportIncludeContent] = useState(true);
  const [exportIncludeTrace, setExportIncludeTrace] = useState(true);
  const [exportIncludeSwarm, setExportIncludeSwarm] = useState(true);

  const filteredSessions = useMemo(() => {
    const q = query.trim().toLowerCase();
    const now = Date.now();
    const rangeDays = range === 'all' ? null : Number(range.replace('d', ''));
    return sessions.filter((s) => {
      if (q && !s.title.toLowerCase().includes(q)) return false;
      if (rangeDays == null) return true;
      const t = new Date(s.updated_at || s.created_at).getTime();
      return t >= now - rangeDays * 24 * 60 * 60 * 1000;
    });
  }, [sessions, query, range]);

  const stats = useMemo(() => {
    const sessionCount = sessions.length;
    let messageCount = 0;
    let traceCount = 0;
    let swarmCount = 0;
    Object.values(sessionMessages).forEach((messages) => {
      messageCount += messages.length;
      messages.forEach((m) => {
        traceCount += (m.traceSteps || []).length;
        swarmCount += (m.swarmEvents || []).length;
      });
    });
    const persisted = localStorage.getItem('chat-storage') || '';
    const kb = Math.round((persisted.length / 1024) * 10) / 10;
    return { sessionCount, messageCount, traceCount, swarmCount, storageKb: kb };
  }, [sessions, sessionMessages]);

  const exportJson = () => {
    const payload = filteredSessions.map((s) => {
      const messages = sessionMessages[s.id] || [];
      if (exportMode === 'summary') {
        const traces = messages.reduce((sum, m) => sum + (m.traceSteps || []).length, 0);
        const swarms = messages.reduce((sum, m) => sum + (m.swarmEvents || []).length, 0);
        return {
          id: s.id,
          title: s.title,
          created_at: s.created_at,
          updated_at: s.updated_at,
          message_count: messages.length,
          trace_count: exportIncludeTrace ? traces : undefined,
          swarm_count: exportIncludeSwarm ? swarms : undefined,
        };
      }
      const filteredMessages = exportIncludeContent
        ? messages
        : messages.map((m: any) => ({ ...m, content: '' }));
      const pruned = filteredMessages.map((m: any) => {
        const next: any = { ...m };
        if (!exportIncludeTrace) delete next.traceSteps;
        if (!exportIncludeSwarm) delete next.swarmEvents;
        return next;
      });
      return { ...s, messages: pruned };
    });
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `memory-export-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const exportMarkdown = () => {
    const lines: string[] = [`# Memory Export (${exportMode})`, ''];
    filteredSessions.forEach((s) => {
      const msgs = sessionMessages[s.id] || [];
      lines.push(`## ${s.title}`);
      lines.push(`- Session ID: ${s.id}`);
      lines.push(`- Updated At: ${s.updated_at || s.created_at}`);
      lines.push(`- Messages: ${msgs.length}`);
      const traces = msgs.reduce((sum, m) => sum + (m.traceSteps || []).length, 0);
      const swarms = msgs.reduce((sum, m) => sum + (m.swarmEvents || []).length, 0);
      if (exportIncludeTrace) lines.push(`- Trace: ${traces}`);
      if (exportIncludeSwarm) lines.push(`- Swarm: ${swarms}`);
      lines.push('');
      if (exportMode === 'summary') return;
      if (!exportIncludeContent) return;
      msgs.forEach((m, idx) => {
        const content = typeof m.content === 'string'
          ? m.content
          : (m.content || []).map((p: any) => (p?.type === 'text' ? p.text : '')).join(' ');
        lines.push(`### ${idx + 1}. ${m.role}`);
        lines.push(content || '(empty)');
        lines.push('');
      });
    });
    const blob = new Blob([lines.join('\n')], { type: 'text/markdown;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `memory-export-${Date.now()}.md`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const cleanupPreview = useMemo(() => {
    const now = Date.now();
    const threshold = now - keepDays * 24 * 60 * 60 * 1000;
    return sessions.filter((s) => new Date(s.updated_at || s.created_at).getTime() < threshold);
  }, [sessions, keepDays]);

  const cleanupOld = () => {
    const toDelete = cleanupPreview.map((s) => s.id);
    if (toDelete.length === 0) {
      toast('无需清理');
      return;
    }
    deleteSessions(toDelete);
    toast.success(`已清理 ${toDelete.length} 个历史会话`);
  };

  return (
    <div className="h-full p-6 overflow-y-auto bg-gray-50 dark:bg-gray-900">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100 flex items-center gap-2 mb-6">
        <Brain className="w-6 h-6" />
        Memory
      </h1>

      <div className="grid grid-cols-1 md:grid-cols-5 gap-4 mb-6">
        <Card><CardContent className="p-4"><div className="text-sm text-gray-500">会话数</div><div className="text-2xl font-semibold">{stats.sessionCount}</div></CardContent></Card>
        <Card><CardContent className="p-4"><div className="text-sm text-gray-500">消息数</div><div className="text-2xl font-semibold">{stats.messageCount}</div></CardContent></Card>
        <Card><CardContent className="p-4"><div className="text-sm text-gray-500">Trace</div><div className="text-2xl font-semibold">{stats.traceCount}</div></CardContent></Card>
        <Card><CardContent className="p-4"><div className="text-sm text-gray-500">Swarm</div><div className="text-2xl font-semibold">{stats.swarmCount}</div></CardContent></Card>
        <Card><CardContent className="p-4"><div className="text-sm text-gray-500">本地存储</div><div className="text-2xl font-semibold">{stats.storageKb} KB</div></CardContent></Card>
      </div>

      <Card className="mb-6">
        <CardHeader><CardTitle>筛选与操作</CardTitle></CardHeader>
        <CardContent className="space-y-3">
          <div className="grid grid-cols-1 md:grid-cols-4 gap-3">
            <Input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="按会话标题搜索" />
            <select value={range} onChange={(e) => setRange(e.target.value as any)} className="h-10 rounded-md border border-gray-300 bg-white px-3 text-sm dark:bg-gray-800 dark:border-gray-700">
              <option value="all">全部时间</option>
              <option value="7d">近 7 天</option>
              <option value="30d">近 30 天</option>
              <option value="90d">近 90 天</option>
            </select>
            <Input type="number" min={1} value={keepDays} onChange={(e) => setKeepDays(Math.max(1, Number(e.target.value || 1)))} placeholder="保留天数" />
            <div className="flex gap-2">
              <select value={exportMode} onChange={(e) => setExportMode(e.target.value as 'summary' | 'full')} className="h-10 rounded-md border border-gray-300 bg-white px-2 text-sm dark:bg-gray-800 dark:border-gray-700">
                <option value="summary">导出概要</option>
                <option value="full">导出全文</option>
              </select>
              <Button variant="secondary" onClick={exportJson}>导出JSON</Button>
              <Button variant="secondary" onClick={exportMarkdown}>导出Markdown</Button>
              <Button variant="danger" onClick={cleanupOld}>清理旧会话</Button>
            </div>
          </div>
          <div className="flex flex-wrap gap-4 text-sm">
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={exportIncludeContent} onChange={(e) => setExportIncludeContent(e.target.checked)} />
              导出消息内容
            </label>
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={exportIncludeTrace} onChange={(e) => setExportIncludeTrace(e.target.checked)} />
              导出 Trace
            </label>
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={exportIncludeSwarm} onChange={(e) => setExportIncludeSwarm(e.target.checked)} />
              导出 Swarm
            </label>
          </div>
          <div className="rounded border border-gray-200 dark:border-gray-700 p-3 bg-white dark:bg-gray-800">
            <div className="text-xs text-gray-500 mb-2">清理预览（将删除）: {cleanupPreview.length}</div>
            {cleanupPreview.length === 0 ? (
              <div className="text-sm text-gray-500">当前无需清理</div>
            ) : (
              <div className="space-y-1 max-h-32 overflow-y-auto">
                {cleanupPreview.slice(0, 20).map((s) => (
                  <div key={s.id} className="text-xs text-gray-600 dark:text-gray-300">
                    {s.title} · {new Date(s.updated_at || s.created_at).toLocaleString()}
                  </div>
                ))}
                {cleanupPreview.length > 20 && (
                  <div className="text-xs text-gray-500">其余 {cleanupPreview.length - 20} 条未展开</div>
                )}
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle>会话内存概览</CardTitle></CardHeader>
        <CardContent className="space-y-2">
          {filteredSessions.length === 0 ? (
            <div className="text-sm text-gray-500">暂无会话数据</div>
          ) : (
            filteredSessions.map((s) => {
              const msgs = sessionMessages[s.id] || [];
              const traces = msgs.reduce((sum, m) => sum + (m.traceSteps || []).length, 0);
              const swarms = msgs.reduce((sum, m) => sum + (m.swarmEvents || []).length, 0);
              return (
                <div key={s.id} className="rounded border border-gray-200 dark:border-gray-700 p-3 bg-white dark:bg-gray-800">
                  <div className="text-sm font-medium text-gray-900 dark:text-gray-100">{s.title}</div>
                  <div className="text-xs text-gray-500 mt-1">消息 {msgs.length} · Trace {traces} · Swarm {swarms} · 更新时间 {new Date(s.updated_at || s.created_at).toLocaleString()}</div>
                </div>
              );
            })
          )}
        </CardContent>
      </Card>
    </div>
  );
};
