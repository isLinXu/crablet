import React, { useEffect, useMemo, useState } from 'react';
import { Activity, Bot, Workflow } from 'lucide-react';
import { useChatStore } from '@/store/chatStore';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { cognitiveLayerLabel, inferCognitiveLayer } from '@/utils/cognitive';

type ActivityItem =
  | { kind: 'trace'; sessionId: string; title: string; time: number; thought: string; action: string; input: string; observation: string }
  | { kind: 'swarm'; sessionId: string; title: string; time: number; from: string; to: string; eventType: string; content: string };
type LayerSwitchItem = { sessionId: string; title: string; time: number; layer: string; reason: string };
type SankeyLink = { from: string; to: string; value: number };
type SankeySessionItem = { title: string; count: number };
type PinnedTransition = { key: string; from: string; to: string };
type AllocatorCandidate = {
  role: string;
  final_score: number;
  expertise_match: number;
  ucb_bonus: number;
  performance_bonus: number;
  preferred_bonus: number;
  load_penalty: number;
};
type AllocatorDecisionView = { selectedRole: string; requestedRole: string; candidates: AllocatorCandidate[] };
type RagRefView = { source: string; score: number; content: string };
type RagObservationView = { retrieval: string; refs_count: number; graph_entities: string[]; refs: RagRefView[] };
type RagTimelineItem = {
  sessionId: string;
  title: string;
  time: number;
  query: string;
  retrieval: string;
  refsCount: number;
  refs: RagRefView[];
  graphEntities: string[];
};
const PINNED_TRANSITIONS_KEY = 'crablet-activity-pinned-transitions';
const ALLOCATOR_CHART_MODE_KEY = 'crablet-activity-allocator-chart-mode';
const ALLOCATOR_CHART_SCOPE_KEY = 'crablet-activity-allocator-chart-scope';
const ALLOCATOR_SESSION_MODE_KEY = 'crablet-activity-allocator-session-chart-modes';

const parseAllocatorDecision = (eventType: string, content: string): AllocatorDecisionView | null => {
  if (eventType !== 'AllocatorDecision') return null;
  try {
    const data = JSON.parse(content || '{}');
    const candidates = (Array.isArray(data?.candidates) ? data.candidates : [])
      .map((c: any) => ({
        role: String(c?.role || 'unknown'),
        final_score: Number(c?.final_score || 0),
        expertise_match: Number(c?.expertise_match || 0),
        ucb_bonus: Number(c?.ucb_bonus || 0),
        performance_bonus: Number(c?.performance_bonus || 0),
        preferred_bonus: Number(c?.preferred_bonus || 0),
        load_penalty: Number(c?.load_penalty || 0),
      }))
      .sort((a: AllocatorCandidate, b: AllocatorCandidate) => b.final_score - a.final_score)
      .slice(0, 5);
    return {
      selectedRole: String(data?.selected_role || 'unknown'),
      requestedRole: String(data?.requested_role || 'unknown'),
      candidates,
    };
  } catch {
    return null;
  }
};

const formatSwarmContent = (eventType: string, content: string) => {
  const parsed = parseAllocatorDecision(eventType, content);
  if (!parsed) return content || '无详细内容';
  const top = parsed.candidates
      .slice(0, 3)
      .map((c) => `${c.role}(${c.final_score.toFixed(2)})`)
      .join(', ');
  return `智能分配: ${parsed.requestedRole} -> ${parsed.selectedRole}${top ? ` | Top: ${top}` : ''}`;
};

const parseRagObservation = (raw: string): RagObservationView | null => {
  if (!raw) return null;
  try {
    const data = JSON.parse(raw || '{}');
    const refs = (Array.isArray(data?.refs) ? data.refs : []).map((x: any) => ({
      source: String(x?.source || 'unknown'),
      score: Number(x?.score || 0),
      content: String(x?.content || ''),
    }));
    return {
      retrieval: String(data?.retrieval || 'unknown'),
      refs_count: Number(data?.refs_count || refs.length),
      graph_entities: Array.isArray(data?.graph_entities) ? data.graph_entities.map((x: any) => String(x)) : [],
      refs,
    };
  } catch {
    return null;
  }
};

const parseRagQuery = (raw: string): string => {
  if (!raw) return '';
  try {
    const parsed = JSON.parse(raw);
    return String(parsed?.query || '');
  } catch {
    return raw;
  }
};

const ragModeLabel = (mode: string): string => {
  if (mode === 'graph_rag') return 'GraphRAG';
  if (mode === 'semantic_search') return 'Semantic';
  if (mode === 'none') return 'None';
  return mode || 'Unknown';
};

export const ActivityCenter: React.FC = () => {
  const sessions = useChatStore((state) => state.sessions);
  const sessionMessages = useChatStore((state) => state.sessionMessages);
  const [filter, setFilter] = useState<'all' | 'trace' | 'swarm' | 'config' | 'rag'>('all');
  const [sessionFilter, setSessionFilter] = useState<string>('all');
  const [layerFilter, setLayerFilter] = useState<'all' | 'system1' | 'system2' | 'system3'>('all');
  const [rangeFilter, setRangeFilter] = useState<'all' | '24h' | '7d' | '30d'>('all');
  const [ragSessionFilter, setRagSessionFilter] = useState<string>('all');
  const [ragRangeFilter, setRagRangeFilter] = useState<'all' | '24h' | '7d' | '30d'>('all');
  const [hoverLink, setHoverLink] = useState<SankeyLink | null>(null);
  const [hoverPos, setHoverPos] = useState<{ x: number; y: number } | null>(null);
  const [selectedTransition, setSelectedTransition] = useState<string | null>(null);
  const [pinnedTransitions, setPinnedTransitions] = useState<PinnedTransition[]>([]);
  const [allocatorChartMode, setAllocatorChartMode] = useState<'bar' | 'stacked'>('bar');
  const [allocatorChartScope, setAllocatorChartScope] = useState<'global' | 'session'>('global');
  const [sessionAllocatorModes, setSessionAllocatorModes] = useState<Record<string, 'bar' | 'stacked'>>({});
  useEffect(() => {
    try {
      const raw = localStorage.getItem(PINNED_TRANSITIONS_KEY);
      if (!raw) return;
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return;
      const normalized = parsed
        .map((x: any) => ({
          key: String(x?.key || ''),
          from: String(x?.from || ''),
          to: String(x?.to || ''),
        }))
        .filter((x) => x.key && x.from && x.to);
      setPinnedTransitions(normalized);
    } catch {}
  }, []);
  useEffect(() => {
    try {
      localStorage.setItem(PINNED_TRANSITIONS_KEY, JSON.stringify(pinnedTransitions));
    } catch {}
  }, [pinnedTransitions]);
  useEffect(() => {
    try {
      const raw = localStorage.getItem(ALLOCATOR_CHART_MODE_KEY);
      if (raw === 'bar' || raw === 'stacked') {
        setAllocatorChartMode(raw);
      }
    } catch {}
  }, []);
  useEffect(() => {
    try {
      localStorage.setItem(ALLOCATOR_CHART_MODE_KEY, allocatorChartMode);
    } catch {}
  }, [allocatorChartMode]);
  useEffect(() => {
    try {
      const raw = localStorage.getItem(ALLOCATOR_CHART_SCOPE_KEY);
      if (raw === 'global' || raw === 'session') {
        setAllocatorChartScope(raw);
      }
    } catch {}
  }, []);
  useEffect(() => {
    try {
      localStorage.setItem(ALLOCATOR_CHART_SCOPE_KEY, allocatorChartScope);
    } catch {}
  }, [allocatorChartScope]);
  useEffect(() => {
    try {
      const raw = localStorage.getItem(ALLOCATOR_SESSION_MODE_KEY);
      if (!raw) return;
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== 'object') return;
      const normalized: Record<string, 'bar' | 'stacked'> = {};
      Object.entries(parsed).forEach(([k, v]) => {
        if (v === 'bar' || v === 'stacked') normalized[String(k)] = v;
      });
      setSessionAllocatorModes(normalized);
    } catch {}
  }, []);
  useEffect(() => {
    try {
      localStorage.setItem(ALLOCATOR_SESSION_MODE_KEY, JSON.stringify(sessionAllocatorModes));
    } catch {}
  }, [sessionAllocatorModes]);

  const getAllocatorMode = (sessionId: string): 'bar' | 'stacked' => {
    if (allocatorChartScope === 'session') {
      return sessionAllocatorModes[sessionId] || allocatorChartMode;
    }
    return allocatorChartMode;
  };
  const setAllocatorModeFor = (mode: 'bar' | 'stacked', sessionId: string) => {
    if (allocatorChartScope === 'session') {
      setSessionAllocatorModes((prev) => ({ ...prev, [sessionId]: mode }));
    } else {
      setAllocatorChartMode(mode);
    }
  };

  const items = useMemo<ActivityItem[]>(() => {
    const byId = new Map(sessions.map((s) => [s.id, s.title]));
    const all: ActivityItem[] = [];
    Object.entries(sessionMessages).forEach(([sessionId, messages]) => {
      const title = byId.get(sessionId) || 'Untitled';
      messages.forEach((msg) => {
        const t = msg.timestamp ? new Date(msg.timestamp).getTime() : Date.now();
        (msg.traceSteps || []).forEach((step) => {
          all.push({
            kind: 'trace',
            sessionId,
            title,
            time: t,
            thought: step.thought || '',
            action: step.action || '',
            input: step.input || '',
            observation: step.observation || '',
          });
        });
        (msg.swarmEvents || []).forEach((event) => {
          all.push({
            kind: 'swarm',
            sessionId,
            title,
            time: event.timestamp || t,
            from: event.from || '',
            to: event.to || '',
            eventType: event.type || '',
            content: event.content || '',
          });
        });
      });
    });
    return all.sort((a, b) => b.time - a.time);
  }, [sessions, sessionMessages]);

  const filtered = useMemo(() => {
    if (filter === 'all') return items;
    if (filter === 'config') {
      return items.filter((i) => i.kind === 'trace' && i.action === 'graph_rag_mode_changed');
    }
    if (filter === 'rag') {
      return items.filter((i) => i.kind === 'trace' && i.action === 'rag_retrieve');
    }
    return items.filter((i) => i.kind === filter);
  }, [items, filter]);

  const traceCount = useMemo(() => items.filter((i) => i.kind === 'trace').length, [items]);
  const swarmCount = useMemo(() => items.filter((i) => i.kind === 'swarm').length, [items]);
  const configCount = useMemo(
    () => items.filter((i) => i.kind === 'trace' && i.action === 'graph_rag_mode_changed').length,
    [items],
  );
  const ragCount = useMemo(
    () => items.filter((i) => i.kind === 'trace' && i.action === 'rag_retrieve').length,
    [items],
  );
  const ragTimeline = useMemo<RagTimelineItem[]>(() => {
    const now = Date.now();
    const minTime =
      ragRangeFilter === '24h' ? now - 24 * 3600 * 1000 :
      ragRangeFilter === '7d' ? now - 7 * 24 * 3600 * 1000 :
      ragRangeFilter === '30d' ? now - 30 * 24 * 3600 * 1000 :
      0;
    return items
      .filter((i): i is Extract<ActivityItem, { kind: 'trace' }> => i.kind === 'trace' && i.action === 'rag_retrieve')
      .filter((i) => (ragSessionFilter === 'all' ? true : i.sessionId === ragSessionFilter))
      .filter((i) => i.time >= minTime)
      .map((i) => {
        const obs = parseRagObservation(i.observation);
        return {
          sessionId: i.sessionId,
          title: i.title,
          time: i.time,
          query: parseRagQuery(i.input),
          retrieval: obs?.retrieval || 'unknown',
          refsCount: obs?.refs_count || 0,
          refs: obs?.refs || [],
          graphEntities: obs?.graph_entities || [],
        };
      });
  }, [items, ragRangeFilter, ragSessionFilter]);
  const ragMetrics = useMemo(() => {
    const total = ragTimeline.length;
    const hit = ragTimeline.filter((x) => x.refsCount > 0).length;
    const graph = ragTimeline.filter((x) => x.retrieval === 'graph_rag').length;
    const semantic = ragTimeline.filter((x) => x.retrieval === 'semantic_search').length;
    const none = ragTimeline.filter((x) => x.retrieval === 'none').length;
    return {
      total,
      hit,
      miss: Math.max(0, total - hit),
      hitRate: total > 0 ? Math.round((hit / total) * 100) : 0,
      graph,
      semantic,
      none,
    };
  }, [ragTimeline]);
  const exportRagSnapshot = () => {
    const snapshot = {
      exported_at: new Date().toISOString(),
      filters: { ragSessionFilter, ragRangeFilter },
      metrics: ragMetrics,
      items: ragTimeline.slice(0, 500),
    };
    const blob = new Blob([JSON.stringify(snapshot, null, 2)], { type: 'application/json;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `rag-snapshot-${Date.now()}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  };
  const cognitiveStats = useMemo(() => {
    let s1 = 0;
    let s2 = 0;
    let s3 = 0;
    let unknown = 0;
    let latestLayer = 'unknown';
    Object.values(sessionMessages).forEach((messages) => {
      messages.forEach((msg) => {
        const steps = msg.traceSteps || [];
        if (steps.length === 0) {
          const fromMsg = inferCognitiveLayer({ text: typeof msg.content === 'string' ? msg.content : '' });
          if (fromMsg === 'system1') s1 += 1;
          else if (fromMsg === 'system2') s2 += 1;
          else if (fromMsg === 'system3') s3 += 1;
          else unknown += 1;
          if (fromMsg !== 'unknown') latestLayer = fromMsg;
        } else {
          steps.forEach((step) => {
            const layer = inferCognitiveLayer(step);
            if (layer === 'system1') s1 += 1;
            else if (layer === 'system2') s2 += 1;
            else if (layer === 'system3') s3 += 1;
            else unknown += 1;
            if (layer !== 'unknown') latestLayer = layer;
          });
        }
      });
    });
    return { s1, s2, s3, unknown, latestLayer };
  }, [sessionMessages]);

  const layerTimelineRaw = useMemo<LayerSwitchItem[]>(() => {
    const byId = new Map(sessions.map((s) => [s.id, s.title]));
    const raw: LayerSwitchItem[] = [];
    Object.entries(sessionMessages).forEach(([sessionId, messages]) => {
      const title = byId.get(sessionId) || 'Untitled';
      messages.forEach((msg) => {
        const time = msg.timestamp ? new Date(msg.timestamp).getTime() : Date.now();
        const direct = (msg as any).cognitiveLayer;
        if (direct && direct !== 'unknown') {
          raw.push({ sessionId, title, time, layer: direct, reason: 'response' });
        }
        (msg.traceSteps || []).forEach((step) => {
          const layer = inferCognitiveLayer(step);
          if (layer !== 'unknown') {
            raw.push({
              sessionId,
              title,
              time,
              layer,
              reason: step.thought || step.action || step.observation || 'trace',
            });
          }
        });
      });
    });
    raw.sort((a, b) => a.time - b.time);
    const deduped: LayerSwitchItem[] = [];
    for (const item of raw) {
      const prev = deduped[deduped.length - 1];
      if (prev && prev.sessionId === item.sessionId && prev.layer === item.layer) continue;
      deduped.push(item);
    }
    return deduped;
  }, [sessions, sessionMessages]);

  const layerTimeline = useMemo(() => {
    const now = Date.now();
    const rangeMs = rangeFilter === '24h' ? 24 * 60 * 60 * 1000 : rangeFilter === '7d' ? 7 * 24 * 60 * 60 * 1000 : rangeFilter === '30d' ? 30 * 24 * 60 * 60 * 1000 : null;
    return layerTimelineRaw
      .filter((item) => (sessionFilter === 'all' ? true : item.sessionId === sessionFilter))
      .filter((item) => (layerFilter === 'all' ? true : item.layer === layerFilter))
      .filter((item) => (rangeMs == null ? true : item.time >= now - rangeMs))
      .slice()
      .reverse();
  }, [layerTimelineRaw, sessionFilter, layerFilter, rangeFilter]);
  const timelineByTransition = useMemo(() => {
    if (!selectedTransition) return layerTimeline;
    const [from, to] = selectedTransition.split('->');
    const asc = layerTimeline.slice().reverse();
    const picked: LayerSwitchItem[] = [];
    for (let i = 1; i < asc.length; i += 1) {
      const prev = asc[i - 1];
      const curr = asc[i];
      if (prev.sessionId === curr.sessionId && prev.layer === from && curr.layer === to) {
        picked.push(curr);
      }
    }
    return picked.reverse();
  }, [layerTimeline, selectedTransition]);

  const switchCount = layerTimeline.length;
  const flowMatrix = useMemo(() => {
    const now = Date.now();
    const rangeMs = rangeFilter === '24h' ? 24 * 60 * 60 * 1000 : rangeFilter === '7d' ? 7 * 24 * 60 * 60 * 1000 : rangeFilter === '30d' ? 30 * 24 * 60 * 60 * 1000 : null;
    const keys = ['system1', 'system2', 'system3'];
    const matrix: Record<string, number> = {};
    keys.forEach((from) => keys.forEach((to) => { matrix[`${from}->${to}`] = 0; }));
    const grouped = new Map<string, LayerSwitchItem[]>();
    layerTimelineRaw.forEach((item) => {
      const arr = grouped.get(item.sessionId) || [];
      arr.push(item);
      grouped.set(item.sessionId, arr);
    });
    grouped.forEach((arr, sid) => {
      const scoped = arr
        .filter(() => (sessionFilter === 'all' ? true : sid === sessionFilter))
        .filter((item) => (layerFilter === 'all' ? true : item.layer === layerFilter))
        .filter((item) => (rangeMs == null ? true : item.time >= now - rangeMs));
      for (let i = 1; i < scoped.length; i += 1) {
        const from = scoped[i - 1].layer;
        const to = scoped[i].layer;
        const key = `${from}->${to}`;
        if (matrix[key] != null) matrix[key] += 1;
      }
    });
    return matrix;
  }, [layerTimelineRaw, sessionFilter, layerFilter, rangeFilter]);
  const flowMatrix24h = useMemo(() => {
    const now = Date.now();
    const rangeMs = 24 * 60 * 60 * 1000;
    const keys = ['system1', 'system2', 'system3'];
    const matrix: Record<string, number> = {};
    keys.forEach((from) => keys.forEach((to) => { matrix[`${from}->${to}`] = 0; }));
    const grouped = new Map<string, LayerSwitchItem[]>();
    layerTimelineRaw.forEach((item) => {
      const arr = grouped.get(item.sessionId) || [];
      arr.push(item);
      grouped.set(item.sessionId, arr);
    });
    grouped.forEach((arr, sid) => {
      const scoped = arr
        .filter(() => (sessionFilter === 'all' ? true : sid === sessionFilter))
        .filter((item) => (layerFilter === 'all' ? true : item.layer === layerFilter))
        .filter((item) => item.time >= now - rangeMs);
      for (let i = 1; i < scoped.length; i += 1) {
        const from = scoped[i - 1].layer;
        const to = scoped[i].layer;
        const key = `${from}->${to}`;
        if (matrix[key] != null) matrix[key] += 1;
      }
    });
    return matrix;
  }, [layerTimelineRaw, sessionFilter, layerFilter]);
  const sankeyLinks = useMemo(() => {
    const layers = ['system1', 'system2', 'system3'];
    const links = layers.flatMap((from) => layers.map((to) => ({
      from,
      to,
      value: flowMatrix[`${from}->${to}`] || 0,
    }))).filter((x) => x.value > 0);
    const max = Math.max(1, ...links.map((x) => x.value), 1);
    const total = links.reduce((sum, x) => sum + x.value, 0);
    return { links, max, total };
  }, [flowMatrix]);
  const transitionSessions = useMemo(() => {
    const now = Date.now();
    const rangeMs = rangeFilter === '24h' ? 24 * 60 * 60 * 1000 : rangeFilter === '7d' ? 7 * 24 * 60 * 60 * 1000 : rangeFilter === '30d' ? 30 * 24 * 60 * 60 * 1000 : null;
    const buckets: Record<string, Record<string, number>> = {};
    const grouped = new Map<string, LayerSwitchItem[]>();
    layerTimelineRaw.forEach((item) => {
      const arr = grouped.get(item.sessionId) || [];
      arr.push(item);
      grouped.set(item.sessionId, arr);
    });
    grouped.forEach((arr, sid) => {
      const scoped = arr
        .filter(() => (sessionFilter === 'all' ? true : sid === sessionFilter))
        .filter((item) => (layerFilter === 'all' ? true : item.layer === layerFilter))
        .filter((item) => (rangeMs == null ? true : item.time >= now - rangeMs));
      for (let i = 1; i < scoped.length; i += 1) {
        const prev = scoped[i - 1];
        const curr = scoped[i];
        const key = `${prev.layer}->${curr.layer}`;
        if (!buckets[key]) buckets[key] = {};
        buckets[key][curr.title] = (buckets[key][curr.title] || 0) + 1;
      }
    });
    const result: Record<string, SankeySessionItem[]> = {};
    Object.entries(buckets).forEach(([k, v]) => {
      result[k] = Object.entries(v)
        .map(([title, count]) => ({ title, count }))
        .sort((a, b) => b.count - a.count)
        .slice(0, 5);
    });
    return result;
  }, [layerTimelineRaw, sessionFilter, layerFilter, rangeFilter]);
  const filterSnapshot = useMemo(() => {
    const sessionText = sessionFilter === 'all' ? '全部会话' : (sessions.find((s) => s.id === sessionFilter)?.title || sessionFilter);
    const layerText = layerFilter === 'all' ? '全部层级' : cognitiveLayerLabel(layerFilter as any);
    const rangeText = rangeFilter === 'all' ? '全部时间' : rangeFilter === '24h' ? '近24小时' : rangeFilter === '7d' ? '近7天' : '近30天';
    return `session=${sessionText}; layer=${layerText}; range=${rangeText}`;
  }, [sessionFilter, layerFilter, rangeFilter, sessions]);
  const copyPinnedSummary = async () => {
    if (pinnedTransitions.length === 0) return;
    const lines = pinnedTransitions.map((pin) => {
      const count = flowMatrix[pin.key] || 0;
      const ratio = sankeyLinks.total > 0 ? Math.round((count / sankeyLinks.total) * 100) : 0;
      const c24 = flowMatrix24h[pin.key] || 0;
      return `${cognitiveLayerLabel(pin.from as any)} -> ${cognitiveLayerLabel(pin.to as any)} | count=${count} | ratio=${ratio}% | 24h=${c24}`;
    });
    const text = [`导出时间: ${new Date().toLocaleString()}`, `筛选快照: ${filterSnapshot}`, ...lines].join('\n');
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const ta = document.createElement('textarea');
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      ta.remove();
    }
  };
  const exportPinnedCsv = () => {
    if (pinnedTransitions.length === 0) return;
    const esc = (v: string) => `"${String(v).replace(/"/g, '""')}"`;
    const exportedAt = new Date().toISOString();
    const lines = ['exported_at,filter_snapshot,transition,count,ratio_percent,count_24h,top_sessions'];
    pinnedTransitions.forEach((pin) => {
      const count = flowMatrix[pin.key] || 0;
      const ratio = sankeyLinks.total > 0 ? Math.round((count / sankeyLinks.total) * 100) : 0;
      const c24 = flowMatrix24h[pin.key] || 0;
      const top = (transitionSessions[pin.key] || []).slice(0, 5).map((s) => `${s.title}:${s.count}`).join('; ');
      lines.push([
        esc(exportedAt),
        esc(filterSnapshot),
        esc(`${cognitiveLayerLabel(pin.from as any)} -> ${cognitiveLayerLabel(pin.to as any)}`),
        count,
        ratio,
        c24,
        esc(top),
      ].join(','));
    });
    const blob = new Blob([lines.join('\n')], { type: 'text/csv;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'activity-pinned-transitions.csv';
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="h-full p-6 overflow-y-auto bg-gray-50 dark:bg-gray-900">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100 flex items-center gap-2">
          <Activity className="w-6 h-6" />
          Activity
        </h1>
        <div className="flex gap-2">
          <Button variant={filter === 'all' ? 'primary' : 'secondary'} size="sm" onClick={() => setFilter('all')}>
            全部
          </Button>
          <Button variant={filter === 'trace' ? 'primary' : 'secondary'} size="sm" onClick={() => setFilter('trace')}>
            Trace
          </Button>
          <Button variant={filter === 'swarm' ? 'primary' : 'secondary'} size="sm" onClick={() => setFilter('swarm')}>
            Swarm
          </Button>
          <Button variant={filter === 'rag' ? 'primary' : 'secondary'} size="sm" onClick={() => setFilter('rag')}>
            RAG
          </Button>
          <Button variant={filter === 'config' ? 'primary' : 'secondary'} size="sm" onClick={() => setFilter('config')}>
            配置
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-6 gap-4 mb-6">
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-gray-500 dark:text-gray-400">会话数</div>
            <div className="text-2xl font-semibold">{sessions.length}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-gray-500 dark:text-gray-400">Trace 事件</div>
            <div className="text-2xl font-semibold">{traceCount}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-gray-500 dark:text-gray-400">Swarm 事件</div>
            <div className="text-2xl font-semibold">{swarmCount}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-gray-500 dark:text-gray-400">层级切换</div>
            <div className="text-2xl font-semibold">{switchCount}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-gray-500 dark:text-gray-400">配置事件</div>
            <div className="text-2xl font-semibold">{configCount}</div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-sm text-gray-500 dark:text-gray-400">RAG 检索</div>
            <div className="text-2xl font-semibold">{ragCount}</div>
          </CardContent>
        </Card>
      </div>

      <Card className="mb-6">
        <CardHeader>
          <div className="flex items-center justify-between gap-2">
            <CardTitle>RAG 流程时间线</CardTitle>
            <Button variant="secondary" size="sm" onClick={exportRagSnapshot}>
              导出RAG快照
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
            <select value={ragSessionFilter} onChange={(e) => setRagSessionFilter(e.target.value)} className="h-9 rounded-md border border-gray-300 bg-white px-2 text-xs dark:bg-gray-800 dark:border-gray-700">
              <option value="all">全部会话</option>
              {sessions.map((s) => (
                <option key={s.id} value={s.id}>{s.title}</option>
              ))}
            </select>
            <select value={ragRangeFilter} onChange={(e) => setRagRangeFilter(e.target.value as any)} className="h-9 rounded-md border border-gray-300 bg-white px-2 text-xs dark:bg-gray-800 dark:border-gray-700">
              <option value="all">全部时间</option>
              <option value="24h">近24小时</option>
              <option value="7d">近7天</option>
              <option value="30d">近30天</option>
            </select>
          </div>
          <div className="grid grid-cols-2 md:grid-cols-6 gap-2">
            <div className="rounded border border-gray-200 dark:border-gray-700 p-2"><div className="text-[11px] text-gray-500">检索总数</div><div className="text-sm font-semibold">{ragMetrics.total}</div></div>
            <div className="rounded border border-gray-200 dark:border-gray-700 p-2"><div className="text-[11px] text-gray-500">命中数</div><div className="text-sm font-semibold">{ragMetrics.hit}</div></div>
            <div className="rounded border border-gray-200 dark:border-gray-700 p-2"><div className="text-[11px] text-gray-500">未命中</div><div className="text-sm font-semibold">{ragMetrics.miss}</div></div>
            <div className="rounded border border-gray-200 dark:border-gray-700 p-2"><div className="text-[11px] text-gray-500">命中率</div><div className="text-sm font-semibold">{ragMetrics.hitRate}%</div></div>
            <div className="rounded border border-gray-200 dark:border-gray-700 p-2"><div className="text-[11px] text-gray-500">GraphRAG</div><div className="text-sm font-semibold">{ragMetrics.graph}</div></div>
            <div className="rounded border border-gray-200 dark:border-gray-700 p-2"><div className="text-[11px] text-gray-500">Semantic</div><div className="text-sm font-semibold">{ragMetrics.semantic}</div></div>
          </div>
          {ragTimeline.length === 0 ? (
            <div className="text-sm text-gray-500 dark:text-gray-400">暂无RAG流程数据</div>
          ) : (
            <div className="space-y-2">
              {ragTimeline.slice(0, 80).map((item, idx) => (
                <div key={`${item.sessionId}-${item.time}-${idx}`} className="rounded border border-gray-200 dark:border-gray-700 p-2 bg-white dark:bg-gray-800">
                  <div className="flex items-center justify-between">
                    <div className="text-xs font-medium text-gray-800 dark:text-gray-100">{item.title}</div>
                    <div className="text-[11px] text-gray-500">{new Date(item.time).toLocaleString()}</div>
                  </div>
                  <div className="mt-1 text-xs text-gray-600 dark:text-gray-300">
                    模式 {ragModeLabel(item.retrieval)} · 命中 {item.refsCount} · Query: {item.query || 'N/A'}
                  </div>
                  {item.graphEntities.length > 0 && (
                    <div className="mt-1 text-[11px] text-gray-500">图实体：{item.graphEntities.slice(0, 8).join('、')}</div>
                  )}
                  {item.refs.length > 0 && (
                    <div className="mt-1 text-[11px] text-gray-500">
                      Top Ref：{item.refs.slice(0, 2).map((r) => `${r.source}(${r.score.toFixed(2)})`).join('，')}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>认知系统检查</CardTitle>
        </CardHeader>
        <CardContent className="grid grid-cols-1 md:grid-cols-5 gap-3">
          <div className="rounded border border-gray-200 dark:border-gray-700 p-3">
            <div className="text-xs text-gray-500">System 1</div>
            <div className="text-xl font-semibold">{cognitiveStats.s1}</div>
          </div>
          <div className="rounded border border-gray-200 dark:border-gray-700 p-3">
            <div className="text-xs text-gray-500">System 2</div>
            <div className="text-xl font-semibold">{cognitiveStats.s2}</div>
          </div>
          <div className="rounded border border-gray-200 dark:border-gray-700 p-3">
            <div className="text-xs text-gray-500">System 3</div>
            <div className="text-xl font-semibold">{cognitiveStats.s3}</div>
          </div>
          <div className="rounded border border-gray-200 dark:border-gray-700 p-3">
            <div className="text-xs text-gray-500">未分类</div>
            <div className="text-xl font-semibold">{cognitiveStats.unknown}</div>
          </div>
          <div className="rounded border border-gray-200 dark:border-gray-700 p-3">
            <div className="text-xs text-gray-500">当前层级</div>
            <div className="text-xl font-semibold">{cognitiveLayerLabel(cognitiveStats.latestLayer as any)}</div>
          </div>
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>层级切换时间线</CardTitle>
        </CardHeader>
        <CardContent className="space-y-2">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
            <select value={sessionFilter} onChange={(e) => setSessionFilter(e.target.value)} className="h-9 rounded-md border border-gray-300 bg-white px-2 text-xs dark:bg-gray-800 dark:border-gray-700">
              <option value="all">全部会话</option>
              {sessions.map((s) => (
                <option key={s.id} value={s.id}>{s.title}</option>
              ))}
            </select>
            <select value={layerFilter} onChange={(e) => setLayerFilter(e.target.value as any)} className="h-9 rounded-md border border-gray-300 bg-white px-2 text-xs dark:bg-gray-800 dark:border-gray-700">
              <option value="all">全部层级</option>
              <option value="system1">System 1</option>
              <option value="system2">System 2</option>
              <option value="system3">System 3</option>
            </select>
            <select value={rangeFilter} onChange={(e) => setRangeFilter(e.target.value as any)} className="h-9 rounded-md border border-gray-300 bg-white px-2 text-xs dark:bg-gray-800 dark:border-gray-700">
              <option value="all">全部时间</option>
              <option value="24h">近24小时</option>
              <option value="7d">近7天</option>
              <option value="30d">近30天</option>
            </select>
          </div>
          {timelineByTransition.length === 0 ? (
            <div className="text-sm text-gray-500 dark:text-gray-400">暂无层级切换记录</div>
          ) : (
            timelineByTransition.slice(0, 120).map((item, idx) => (
              <div key={`${item.sessionId}-${item.time}-${idx}`} className="rounded border border-gray-200 dark:border-gray-700 p-2 bg-white dark:bg-gray-800">
                <div className="flex items-center justify-between">
                  <div className="text-xs font-medium text-gray-800 dark:text-gray-100">{item.title}</div>
                  <div className="text-[11px] text-gray-500">{new Date(item.time).toLocaleString()}</div>
                </div>
                <div className="mt-1 text-xs text-gray-600 dark:text-gray-300">
                  切换到 <span className="font-semibold">{cognitiveLayerLabel(item.layer as any)}</span> · {item.reason}
                </div>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      <Card className="mb-6">
        <CardHeader>
          <CardTitle>S1/S2/S3迁移流向</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="text-xs text-gray-500 flex items-center justify-between">
            <span>总迁移量：{sankeyLinks.total}</span>
            {selectedTransition ? (
              <button className="px-2 py-0.5 rounded border border-gray-300 dark:border-gray-700" onClick={() => setSelectedTransition(null)}>
                清除连线过滤
              </button>
            ) : null}
          </div>
          {sankeyLinks.links.length === 0 ? (
            <div className="text-sm text-gray-500 dark:text-gray-400">暂无迁移数据</div>
          ) : (
            <div className="relative rounded border border-gray-200 dark:border-gray-700 p-2 bg-white dark:bg-gray-800 overflow-x-auto">
              <svg width="100%" height="220" viewBox="0 0 760 220" preserveAspectRatio="xMidYMid meet">
                {['system1', 'system2', 'system3'].map((layer, idx) => (
                  <g key={`left-${layer}`}>
                    <rect x="24" y={24 + idx * 62} width="90" height="30" rx="6" className="fill-blue-100 dark:fill-blue-900/30" />
                    <text x="69" y={44 + idx * 62} textAnchor="middle" className="fill-gray-700 dark:fill-gray-200 text-[11px]">{cognitiveLayerLabel(layer as any)}</text>
                  </g>
                ))}
                {['system1', 'system2', 'system3'].map((layer, idx) => (
                  <g key={`right-${layer}`}>
                    <rect x="646" y={24 + idx * 62} width="90" height="30" rx="6" className="fill-emerald-100 dark:fill-emerald-900/30" />
                    <text x="691" y={44 + idx * 62} textAnchor="middle" className="fill-gray-700 dark:fill-gray-200 text-[11px]">{cognitiveLayerLabel(layer as any)}</text>
                  </g>
                ))}
                {sankeyLinks.links.map((link) => {
                  const fromIdx = link.from === 'system1' ? 0 : link.from === 'system2' ? 1 : 2;
                  const toIdx = link.to === 'system1' ? 0 : link.to === 'system2' ? 1 : 2;
                  const y1 = 39 + fromIdx * 62;
                  const y2 = 39 + toIdx * 62;
                  const strokeW = 2 + (link.value / sankeyLinks.max) * 12;
                  const d = `M 114 ${y1} C 260 ${y1}, 500 ${y2}, 646 ${y2}`;
                  const selected = selectedTransition === `${link.from}->${link.to}`;
                  return (
                    <g key={`${link.from}-${link.to}`}>
                      <path
                        d={d}
                        fill="none"
                        stroke="currentColor"
                        className={selected ? 'text-fuchsia-500' : 'text-violet-500/70'}
                        strokeWidth={strokeW}
                        strokeLinecap="round"
                        onMouseEnter={(e) => {
                          setHoverLink(link);
                          setHoverPos({ x: e.clientX, y: e.clientY });
                        }}
                        onMouseMove={(e) => setHoverPos({ x: e.clientX, y: e.clientY })}
                        onMouseLeave={() => {
                          setHoverLink(null);
                          setHoverPos(null);
                        }}
                        onClick={() => setSelectedTransition((prev) => (prev === `${link.from}->${link.to}` ? null : `${link.from}->${link.to}`))}
                        onContextMenu={(e) => {
                          e.preventDefault();
                          const key = `${link.from}->${link.to}`;
                          setPinnedTransitions((prev) => {
                            const exists = prev.some((p) => p.key === key);
                            if (exists) return prev.filter((p) => p.key !== key);
                            return [...prev, { key, from: link.from, to: link.to }];
                          });
                        }}
                        style={{ cursor: 'pointer' }}
                      />
                      <text x="380" y={(y1 + y2) / 2 - 3} textAnchor="middle" className="fill-gray-600 dark:fill-gray-300 text-[10px]">
                        {link.value}
                      </text>
                    </g>
                  );
                })}
              </svg>
              {hoverLink && hoverPos ? (
                <div
                  className="fixed z-50 text-[11px] rounded border border-gray-300 dark:border-gray-700 bg-white/95 dark:bg-gray-900/95 px-2 py-1 shadow-lg min-w-[180px]"
                  style={{ left: hoverPos.x + 12, top: hoverPos.y + 12 }}
                >
                  <div className="font-semibold">
                    {cognitiveLayerLabel(hoverLink.from as any)} → {cognitiveLayerLabel(hoverLink.to as any)} · {hoverLink.value}
                  </div>
                  <div className="text-gray-500">
                    占比 {sankeyLinks.total > 0 ? Math.round((hoverLink.value / sankeyLinks.total) * 100) : 0}% · 24h {flowMatrix24h[`${hoverLink.from}->${hoverLink.to}`] || 0}
                  </div>
                  <div className="mt-1 text-gray-500">来源会话</div>
                  {(transitionSessions[`${hoverLink.from}->${hoverLink.to}`] || []).slice(0, 3).map((s) => (
                    <div key={`${s.title}-${s.count}`} className="flex items-center justify-between gap-2">
                      <span className="truncate max-w-[130px]">{s.title}</span>
                      <span>{s.count}</span>
                    </div>
                  ))}
                </div>
              ) : null}
            </div>
          )}
          {pinnedTransitions.length > 0 ? (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
              <div className="md:col-span-2 flex justify-end gap-2">
                <button className="text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-700" onClick={copyPinnedSummary}>
                  复制固定卡片摘要
                </button>
                <button className="text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-700" onClick={exportPinnedCsv}>
                  导出固定卡片CSV
                </button>
                <button className="text-xs px-2 py-1 rounded border border-gray-300 dark:border-gray-700" onClick={() => setPinnedTransitions([])}>
                  清空固定卡片
                </button>
              </div>
              {pinnedTransitions.map((pin) => {
                const value = flowMatrix[pin.key] || 0;
                const ratio = sankeyLinks.total > 0 ? Math.round((value / sankeyLinks.total) * 100) : 0;
                const list = transitionSessions[pin.key] || [];
                return (
                  <div key={pin.key} className="rounded border border-gray-200 dark:border-gray-700 p-2 bg-white dark:bg-gray-800">
                    <div className="flex items-center justify-between">
                      <div className="text-xs font-semibold">{cognitiveLayerLabel(pin.from as any)} → {cognitiveLayerLabel(pin.to as any)}</div>
                      <button className="text-[11px] px-2 py-0.5 rounded border border-gray-300 dark:border-gray-700" onClick={() => setPinnedTransitions((prev) => prev.filter((p) => p.key !== pin.key))}>
                        移除
                      </button>
                    </div>
                    <div className="text-xs text-gray-500 mt-1">当前 {value} · 占比 {ratio}% · 24h {flowMatrix24h[pin.key] || 0}</div>
                    <div className="mt-1 space-y-0.5">
                      {list.slice(0, 3).map((s) => (
                        <div key={`${pin.key}-${s.title}`} className="text-[11px] flex items-center justify-between">
                          <span className="truncate max-w-[180px]">{s.title}</span>
                          <span>{s.count}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          ) : null}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
            {['system1', 'system2', 'system3'].map((from) => (
              <div key={from} className="rounded border border-gray-200 dark:border-gray-700 p-2">
                <div className="text-xs text-gray-500 mb-1">{cognitiveLayerLabel(from as any)} 出发</div>
                {['system1', 'system2', 'system3'].map((to) => {
                  const value = flowMatrix[`${from}->${to}`] || 0;
                  const ratio = sankeyLinks.total > 0 ? Math.round((value / sankeyLinks.total) * 100) : 0;
                  return (
                    <div key={`${from}-${to}`} className="text-xs flex items-center justify-between">
                      <span>{cognitiveLayerLabel(to as any)}</span>
                      <span className="font-semibold">{value} · {ratio}%</span>
                    </div>
                  );
                })}
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>最近活动</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          {filtered.length === 0 ? (
            <div className="text-sm text-gray-500 dark:text-gray-400">暂无活动数据，可先在 Chat 或 Swarm 中触发一次执行。</div>
          ) : (
            filtered.slice(0, 120).map((item, idx) => (
              <div key={`${item.sessionId}-${item.time}-${idx}`} className="rounded border border-gray-200 dark:border-gray-700 p-3 bg-white dark:bg-gray-800">
                <div className="flex items-center justify-between mb-1">
                  <div className="text-sm font-medium text-gray-900 dark:text-gray-100">{item.title}</div>
                  <div className="text-xs text-gray-500">{new Date(item.time).toLocaleString()}</div>
                </div>
                {item.kind === 'trace' ? (
                  <div className="text-xs text-gray-600 dark:text-gray-300 space-y-1">
                    {item.action === 'rag_retrieve' ? (
                      <div className="space-y-2">
                        <div className="inline-flex items-center gap-1 mr-2 px-2 py-0.5 rounded bg-fuchsia-100 dark:bg-fuchsia-900/30 text-fuchsia-700 dark:text-fuchsia-300">
                          <Activity className="w-3 h-3" /> RAG 检索流程
                        </div>
                        {(() => {
                          const parsed = parseRagObservation(item.observation);
                          if (!parsed) {
                            return <div>{item.thought || item.action || item.observation || '无详细内容'}</div>;
                          }
                          return (
                            <div className="space-y-1">
                              <div>检索模式：{parsed.retrieval} · 命中：{parsed.refs_count}</div>
                              <div>Query：{item.input || 'N/A'}</div>
                              {parsed.graph_entities.length > 0 && (
                                <div>图实体：{parsed.graph_entities.slice(0, 6).join('、')}</div>
                              )}
                              {parsed.refs.length > 0 && (
                                <div className="mt-1 rounded border border-fuchsia-200 dark:border-fuchsia-800 p-2 bg-fuchsia-50/60 dark:bg-fuchsia-900/20 space-y-1">
                                  {parsed.refs.slice(0, 3).map((ref, refIdx) => (
                                    <div key={`${item.sessionId}-${item.time}-${refIdx}`}>
                                      <span className="font-medium">[{ref.source}] {ref.score.toFixed(2)}</span>
                                      <span> · {ref.content}</span>
                                    </div>
                                  ))}
                                </div>
                              )}
                            </div>
                          );
                        })()}
                      </div>
                    ) : item.action === 'graph_rag_mode_changed' ? (
                      <div className="inline-flex items-center gap-1 mr-2 px-2 py-0.5 rounded bg-violet-100 dark:bg-violet-900/30 text-violet-700 dark:text-violet-300">
                        <Activity className="w-3 h-3" /> GraphRAG 模式切换
                      </div>
                    ) : (
                      <div className="inline-flex items-center gap-1 mr-2 px-2 py-0.5 rounded bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300">
                        <Bot className="w-3 h-3" /> Trace
                      </div>
                    )}
                    {item.action !== 'rag_retrieve' && (
                      <div>{item.thought || item.action || item.observation || '无详细内容'}</div>
                    )}
                  </div>
                ) : (
                  <div className="text-xs text-gray-600 dark:text-gray-300 space-y-1">
                    <div className="inline-flex items-center gap-1 mr-2 px-2 py-0.5 rounded bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300">
                      <Workflow className="w-3 h-3" /> Swarm
                    </div>
                    <div>{item.from} → {item.to} · {item.eventType}</div>
                    <div>{formatSwarmContent(item.eventType, item.content)}</div>
                    {item.eventType === 'AllocatorDecision' && (() => {
                      const parsed = parseAllocatorDecision(item.eventType, item.content);
                      if (!parsed || parsed.candidates.length === 0) return null;
                      const chartMode = getAllocatorMode(item.sessionId);
                      const maxScore = Math.max(...parsed.candidates.map((c) => c.final_score), 0.0001);
                      const maxContribution = Math.max(
                        ...parsed.candidates.map((c) =>
                          c.expertise_match + c.ucb_bonus + c.performance_bonus + c.preferred_bonus + c.load_penalty
                        ),
                        0.0001,
                      );
                      return (
                        <div className="mt-2 rounded border border-gray-200 dark:border-gray-700 p-2 bg-gray-50 dark:bg-gray-900/40 space-y-1">
                          <div className="flex items-center justify-between">
                            <div className="text-[11px] text-gray-500">分配评分细节</div>
                            <div className="inline-flex rounded border border-gray-300 dark:border-gray-700 overflow-hidden">
                              <button
                                className={`px-2 py-0.5 text-[10px] ${allocatorChartScope === 'global' ? 'bg-indigo-500 text-white' : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-300'}`}
                                onClick={() => setAllocatorChartScope('global')}
                              >
                                全局
                              </button>
                              <button
                                className={`px-2 py-0.5 text-[10px] border-l border-gray-300 dark:border-gray-700 ${allocatorChartScope === 'session' ? 'bg-indigo-500 text-white' : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-300'}`}
                                onClick={() => setAllocatorChartScope('session')}
                              >
                                会话
                              </button>
                            </div>
                            <div className="inline-flex rounded border border-gray-300 dark:border-gray-700 overflow-hidden">
                              <button
                                className={`px-2 py-0.5 text-[10px] ${chartMode === 'bar' ? 'bg-emerald-500 text-white' : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-300'}`}
                                onClick={() => setAllocatorModeFor('bar', item.sessionId)}
                              >
                                单柱
                              </button>
                              <button
                                className={`px-2 py-0.5 text-[10px] border-l border-gray-300 dark:border-gray-700 ${chartMode === 'stacked' ? 'bg-emerald-500 text-white' : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-300'}`}
                                onClick={() => setAllocatorModeFor('stacked', item.sessionId)}
                              >
                                堆叠
                              </button>
                            </div>
                          </div>
                          {parsed.candidates.map((c) => {
                            const width = Math.max(4, Math.round((c.final_score / maxScore) * 100));
                            const positiveTotal = c.expertise_match + c.ucb_bonus + c.performance_bonus + c.preferred_bonus;
                            const expertiseW = Math.max(1, Math.round((c.expertise_match / maxContribution) * 100));
                            const ucbW = Math.max(1, Math.round((c.ucb_bonus / maxContribution) * 100));
                            const perfW = Math.max(1, Math.round((c.performance_bonus / maxContribution) * 100));
                            const prefW = Math.max(1, Math.round((c.preferred_bonus / maxContribution) * 100));
                            const loadW = Math.max(1, Math.round((c.load_penalty / maxContribution) * 100));
                            return (
                              <div key={`${item.sessionId}-${item.time}-${c.role}`} className="space-y-0.5">
                                <div className="flex items-center justify-between text-[11px]">
                                  <span className="font-medium">{c.role}</span>
                                  <span className="text-gray-500">
                                    final {c.final_score.toFixed(2)} · match {c.expertise_match.toFixed(2)} · ucb {c.ucb_bonus.toFixed(2)} · perf {c.performance_bonus.toFixed(2)} · pref {c.preferred_bonus.toFixed(2)} · load -{c.load_penalty.toFixed(2)}
                                  </span>
                                </div>
                                {chartMode === 'bar' ? (
                                  <div className="h-1.5 rounded bg-gray-200 dark:bg-gray-700 overflow-hidden">
                                    <div className="h-full bg-emerald-500 dark:bg-emerald-400" style={{ width: `${width}%` }} />
                                  </div>
                                ) : (
                                  <div className="space-y-0.5">
                                    <div className="h-1.5 rounded bg-gray-200 dark:bg-gray-700 overflow-hidden flex">
                                      <div className="h-full bg-blue-500" style={{ width: `${expertiseW}%` }} title={`match ${c.expertise_match.toFixed(2)}`} />
                                      <div className="h-full bg-violet-500" style={{ width: `${ucbW}%` }} title={`ucb ${c.ucb_bonus.toFixed(2)}`} />
                                      <div className="h-full bg-emerald-500" style={{ width: `${perfW}%` }} title={`perf ${c.performance_bonus.toFixed(2)}`} />
                                      <div className="h-full bg-cyan-500" style={{ width: `${prefW}%` }} title={`pref ${c.preferred_bonus.toFixed(2)}`} />
                                      <div className="h-full bg-rose-500" style={{ width: `${loadW}%` }} title={`load -${c.load_penalty.toFixed(2)}`} />
                                    </div>
                                    <div className="text-[10px] text-gray-500">
                                      ({positiveTotal.toFixed(2)}) - ({c.load_penalty.toFixed(2)}) = {c.final_score.toFixed(2)}
                                    </div>
                                  </div>
                                )}
                              </div>
                            );
                          })}
                        </div>
                      );
                    })()}
                  </div>
                )}
              </div>
            ))
          )}
        </CardContent>
      </Card>
    </div>
  );
};
