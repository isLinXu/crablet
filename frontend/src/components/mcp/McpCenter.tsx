import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { AlertTriangle, Plug, RefreshCw } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { settingsService } from '@/services/settingsService';
import type { McpOverview } from '@/types/domain';
import toast from 'react-hot-toast';

export const McpCenter: React.FC = () => {
  const [overview, setOverview] = useState<McpOverview | null>(null);
  const [loading, setLoading] = useState(false);
  const [history, setHistory] = useState<Array<{ time: number; status: string; reason?: string }>>([]);
  const [promptGroupMode, setPromptGroupMode] = useState<'server' | 'prefix' | 'custom'>('server');
  const [promptRules, setPromptRules] = useState<Array<{ match: string; group: string }>>([]);
  const [newRuleMatch, setNewRuleMatch] = useState('');
  const [newRuleGroup, setNewRuleGroup] = useState('');

  const detectServer = useCallback((value: string) => {
    const v = value.trim();
    if (!v) return 'default';
    if (v.includes('://')) return v.split('://')[0] || 'default';
    if (v.includes(':')) return v.split(':')[0] || 'default';
    if (v.includes('/')) return v.split('/')[0] || 'default';
    return 'default';
  }, []);

  useEffect(() => {
    try {
      const raw = localStorage.getItem('crablet-mcp-prompt-group-rules');
      if (!raw) return;
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) {
        const normalized = parsed
          .filter((r) => r && typeof r === 'object')
          .map((r: any) => ({ match: String(r.match || '').trim(), group: String(r.group || '').trim() }))
          .filter((r) => r.match && r.group);
        setPromptRules(normalized);
      }
    } catch {}
  }, []);

  useEffect(() => {
    try {
      localStorage.setItem('crablet-mcp-prompt-group-rules', JSON.stringify(promptRules));
    } catch {}
  }, [promptRules]);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const data = await settingsService.getMcpOverview();
      setOverview(data);
      setHistory((prev) => {
        const next = [...prev, { time: Date.now(), status: data.status || 'unknown' }];
        return next.slice(-30);
      });
    } catch (e: any) {
      const reason = e?.response?.data?.message || e?.message || 'unknown error';
      setHistory((prev) => {
        const next = [...prev, { time: Date.now(), status: 'error', reason }];
        return next.slice(-30);
      });
      toast.error('加载 MCP 状态失败');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    const timer = setInterval(() => {
      load();
    }, 15000);
    return () => clearInterval(timer);
  }, [load]);

  const statusText = useMemo(() => {
    if (!overview) return 'unknown';
    return overview.status || 'ok';
  }, [overview]);

  const recentFailCount = useMemo(() => {
    return history.slice(-10).filter((h) => {
      const s = h.status.toLowerCase();
      return s.includes('fail') || s.includes('error') || s.includes('down');
    }).length;
  }, [history]);

  const failReasonAgg = useMemo(() => {
    const map = new Map<string, number>();
    history.forEach((h) => {
      if (!h.reason) return;
      const key = h.reason.length > 80 ? `${h.reason.slice(0, 80)}...` : h.reason;
      map.set(key, (map.get(key) || 0) + 1);
    });
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1]).slice(0, 6);
  }, [history]);

  const spark = useMemo(() => {
    const items = history.slice(-24);
    return items.map((h, i) => {
      const s = h.status.toLowerCase();
      const bad = s.includes('fail') || s.includes('error') || s.includes('down');
      return { key: `${h.time}-${i}`, bad };
    });
  }, [history]);

  const groupedResources = useMemo(() => {
    const groups = new Map<string, Array<{ uri: string; name?: string; description?: string }>>();
    (overview?.resource_items || []).forEach((item) => {
      const key = detectServer(item.uri || item.name || '');
      const arr = groups.get(key) || [];
      arr.push(item);
      groups.set(key, arr);
    });
    return Array.from(groups.entries()).sort((a, b) => a[0].localeCompare(b[0]));
  }, [overview, detectServer]);

  const groupedPrompts = useMemo(() => {
    const groups = new Map<string, Array<{ name: string; description?: string }>>();
    (overview?.prompt_items || []).forEach((item) => {
      const name = item.name || '';
      const key = (() => {
        if (promptGroupMode === 'server') return detectServer(name);
        if (promptGroupMode === 'prefix') return name.split(/[\/:_\-.]/)[0] || 'default';
        const matched = promptRules.find((r) => name.startsWith(r.match));
        return matched?.group || 'other';
      })();
      const arr = groups.get(key) || [];
      arr.push(item);
      groups.set(key, arr);
    });
    return Array.from(groups.entries()).sort((a, b) => a[0].localeCompare(b[0]));
  }, [overview, detectServer, promptGroupMode, promptRules]);

  const trendPath = useMemo(() => {
    const points = history.slice(-24).map((h, idx, arr) => {
      const s = h.status.toLowerCase();
      const bad = s.includes('fail') || s.includes('error') || s.includes('down');
      const x = arr.length <= 1 ? 4 : (idx / (arr.length - 1)) * 316 + 4;
      const y = bad ? 30 : 6;
      return { x, y };
    });
    if (points.length < 2) return '';
    return points.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x} ${p.y}`).join(' ');
  }, [history]);

  return (
    <div className="h-full p-6 overflow-y-auto bg-gray-50 dark:bg-gray-900">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100 flex items-center gap-2">
          <Plug className="w-6 h-6" />
          MCP
        </h1>
        <Button variant="secondary" size="sm" onClick={load} loading={loading}>
          <RefreshCw className="w-4 h-4 mr-2" />
          刷新
        </Button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
        <Card><CardContent className="p-4"><div className="text-sm text-gray-500">状态</div><div className="text-2xl font-semibold">{statusText}</div></CardContent></Card>
        <Card><CardContent className="p-4"><div className="text-sm text-gray-500">Tools</div><div className="text-2xl font-semibold">{overview?.mcp_tools ?? '-'}</div></CardContent></Card>
        <Card><CardContent className="p-4"><div className="text-sm text-gray-500">Resources</div><div className="text-2xl font-semibold">{overview?.resources ?? '-'}</div></CardContent></Card>
        <Card><CardContent className="p-4"><div className="text-sm text-gray-500">Prompts</div><div className="text-2xl font-semibold">{overview?.prompts ?? '-'}</div></CardContent></Card>
      </div>

      {recentFailCount > 0 && (
        <Card className="mb-6 border-amber-300 dark:border-amber-700">
          <CardContent className="p-4 flex items-center gap-2 text-amber-700 dark:text-amber-300">
            <AlertTriangle className="w-4 h-4" />
            最近 10 次健康检查中有 {recentFailCount} 次异常，请检查 MCP 服务状态。
          </CardContent>
        </Card>
      )}

      <Card className="mb-6">
        <CardHeader><CardTitle>健康检查历史</CardTitle></CardHeader>
        <CardContent className="space-y-2">
          <div className="rounded border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-2">
            {spark.length === 0 ? (
              <div className="text-xs text-gray-500">暂无趋势数据</div>
            ) : (
              <svg width="100%" height="40" viewBox="0 0 324 36" preserveAspectRatio="none">
                <line x1="4" y1="30" x2="320" y2="30" stroke="currentColor" className="text-gray-300 dark:text-gray-700" />
                <line x1="4" y1="6" x2="320" y2="6" stroke="currentColor" className="text-gray-200 dark:text-gray-800" />
                {trendPath ? <path d={trendPath} fill="none" stroke="currentColor" className="text-blue-500" strokeWidth="2" /> : null}
                {spark.map((p, i) => {
                  const x = spark.length <= 1 ? 4 : (i / (spark.length - 1)) * 316 + 4;
                  const y = p.bad ? 30 : 6;
                  return <circle key={p.key} cx={x} cy={y} r="2.5" className={p.bad ? 'text-red-500 fill-current' : 'text-emerald-500 fill-current'} />;
                })}
              </svg>
            )}
          </div>
          {history.length === 0 ? (
            <div className="text-sm text-gray-500">暂无历史记录</div>
          ) : (
            history.slice().reverse().map((h, idx) => (
              <div key={`${h.time}-${idx}`} className="text-sm rounded border border-gray-200 dark:border-gray-700 p-2 flex items-center justify-between">
                <span className="text-gray-500">{new Date(h.time).toLocaleTimeString()}</span>
                <span className="font-medium">{h.status}{h.reason ? ` · ${h.reason}` : ''}</span>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader><CardTitle>失败原因聚合</CardTitle></CardHeader>
        <CardContent className="space-y-2">
          {failReasonAgg.length === 0 ? (
            <div className="text-sm text-gray-500">暂无失败原因</div>
          ) : (
            failReasonAgg.map(([reason, count]) => (
              <div key={reason} className="text-sm rounded border border-gray-200 dark:border-gray-700 p-2 flex justify-between gap-3">
                <span className="text-gray-600 dark:text-gray-300">{reason}</span>
                <span className="font-medium">{count}</span>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Card>
          <CardHeader><CardTitle>Resource 分组</CardTitle></CardHeader>
          <CardContent className="space-y-2">
            {groupedResources.length === 0 ? (
              <div className="text-sm text-gray-500">暂无 resource</div>
            ) : (
              groupedResources.map(([server, items]) => (
                <div key={server} className="rounded border border-gray-200 dark:border-gray-700 p-2">
                  <div className="text-xs text-gray-500 mb-2">{server} · {items.length}</div>
                  <div className="space-y-2">
                    {items.map((r) => (
                      <div key={r.uri} className="text-sm rounded border border-gray-200 dark:border-gray-700 p-2">
                        <div className="font-medium">{r.name || r.uri}</div>
                        <div className="text-xs text-gray-500 break-all">{r.uri}</div>
                      </div>
                    ))}
                  </div>
                </div>
              ))
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Prompt 分组</CardTitle>
            <div className="mt-2">
              <select
                value={promptGroupMode}
                onChange={(e) => setPromptGroupMode(e.target.value as 'server' | 'prefix' | 'custom')}
                className="h-8 rounded-md border border-gray-300 bg-white px-2 text-xs dark:bg-gray-800 dark:border-gray-700"
              >
                <option value="server">按服务分组</option>
                <option value="prefix">按名称前缀分组</option>
                <option value="custom">按规则分组</option>
              </select>
            </div>
          </CardHeader>
          <CardContent className="space-y-2">
            {promptGroupMode === 'custom' && (
              <div className="rounded border border-gray-200 dark:border-gray-700 p-3 bg-white dark:bg-gray-800 space-y-2">
                <div className="text-xs text-gray-500">规则按前缀匹配，优先使用最先匹配的规则</div>
                <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
                  <Input value={newRuleMatch} onChange={(e) => setNewRuleMatch(e.target.value)} placeholder="匹配前缀，如 openai/" />
                  <Input value={newRuleGroup} onChange={(e) => setNewRuleGroup(e.target.value)} placeholder="分组名称，如 OpenAI" />
                  <Button
                    variant="secondary"
                    onClick={() => {
                      const match = newRuleMatch.trim();
                      const group = newRuleGroup.trim();
                      if (!match || !group) return;
                      setPromptRules((prev) => [...prev, { match, group }]);
                      setNewRuleMatch('');
                      setNewRuleGroup('');
                    }}
                  >
                    添加规则
                  </Button>
                </div>
                {promptRules.length === 0 ? (
                  <div className="text-sm text-gray-500">暂无规则</div>
                ) : (
                  <div className="space-y-1">
                    {promptRules.map((r, idx) => (
                      <div key={`${r.match}-${idx}`} className="text-xs rounded border border-gray-200 dark:border-gray-700 p-2 flex items-center justify-between gap-3">
                        <div className="text-gray-700 dark:text-gray-200 break-all">{r.match} → {r.group}</div>
                        <Button variant="ghost" size="sm" onClick={() => setPromptRules((prev) => prev.filter((_, i) => i !== idx))}>
                          删除
                        </Button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
            {groupedPrompts.length === 0 ? (
              <div className="text-sm text-gray-500">暂无 prompt</div>
            ) : (
              groupedPrompts.map(([server, items]) => (
                <div key={server} className="rounded border border-gray-200 dark:border-gray-700 p-2">
                  <div className="text-xs text-gray-500 mb-2">{server} · {items.length}</div>
                  <div className="space-y-2">
                    {items.map((p) => (
                      <div key={p.name} className="text-sm rounded border border-gray-200 dark:border-gray-700 p-2">
                        <div className="font-medium">{p.name}</div>
                        <div className="text-xs text-gray-500">{p.description || '无描述'}</div>
                      </div>
                    ))}
                  </div>
                </div>
              ))
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
};
