import { useState, useCallback, useMemo } from 'react';

interface ActivityItem {
  sessionId: string;
  title: string;
  time: number;
  kind: 'trace' | 'swarm';
  action?: string;
  thought?: string;
  observation?: string;
  input?: string;
  from?: string;
  to?: string;
  eventType?: string;
  content?: string;
}

interface SessionInfo {
  id: string;
  title: string;
}

interface TransitionSession {
  title: string;
  count: number;
}

interface SankeyLink {
  from: string;
  to: string;
  value: number;
}

interface SankeyLinksData {
  links: SankeyLink[];
  total: number;
  max: number;
}

interface CognitiveStats {
  s1: number;
  s2: number;
  s3: number;
  unknown: number;
  latestLayer: string;
}

interface RagMetrics {
  total: number;
  hit: number;
  miss: number;
  hitRate: number;
  graph: number;
  semantic: number;
}

interface RagTimelineItem {
  sessionId: string;
  title: string;
  time: number;
  retrieval: string;
  refsCount: number;
  query: string;
  graphEntities: string[];
  refs: { source: string; score: number }[];
}

interface LayerTransition {
  sessionId: string;
  title: string;
  time: number;
  layer: string;
  reason: string;
}

interface PinnedTransition {
  key: string;
  from: string;
  to: string;
}

export function useActivityState() {
  const [filter, setFilter] = useState<'all' | 'trace' | 'swarm' | 'rag' | 'config'>('all');
  const [sessionFilter, setSessionFilter] = useState('all');
  const [layerFilter, setLayerFilter] = useState('all');
  const [rangeFilter, setRangeFilter] = useState('all');
  const [ragSessionFilter, setRagSessionFilter] = useState('all');
  const [ragRangeFilter, setRagRangeFilter] = useState('all');
  const [selectedTransition, setSelectedTransition] = useState<string | null>(null);
  const [hoverLink, setHoverLink] = useState<SankeyLink | null>(null);
  const [hoverPos, setHoverPos] = useState<{ x: number; y: number } | null>(null);
  const [pinnedTransitions, setPinnedTransitions] = useState<PinnedTransition[]>([]);
  const [allocatorChartScope, setAllocatorChartScope] = useState<'global' | 'session'>('global');

  const sessions: SessionInfo[] = [];
  const items: ActivityItem[] = [];
  const filtered = items;

  const traceCount = items.filter((i) => i.kind === 'trace').length;
  const swarmCount = items.filter((i) => i.kind === 'swarm').length;
  const switchCount = 0;
  const configCount = 0;
  const ragCount = items.filter((i) => i.action === 'rag_retrieve').length;

  const cognitiveStats: CognitiveStats = { s1: 0, s2: 0, s3: 0, unknown: 0, latestLayer: 'system1' };
  const ragMetrics: RagMetrics = { total: 0, hit: 0, miss: 0, hitRate: 0, graph: 0, semantic: 0 };
  const ragTimeline: RagTimelineItem[] = [];
  const timelineByTransition: LayerTransition[] = [];

  const sankeyLinks: SankeyLinksData = { links: [], total: 0, max: 1 };
  const flowMatrix: Record<string, number> = {};
  const flowMatrix24h: Record<string, number> = {};
  const transitionSessions: Record<string, TransitionSession[]> = {};

  const getAllocatorMode = (_sessionId: string) => 'bar' as const;
  const setAllocatorModeFor = (_mode: 'bar' | 'stacked', _sessionId: string) => {};
  const exportRagSnapshot = () => {};
  const copyPinnedSummary = () => {};
  const exportPinnedCsv = () => {};

  return {
    filter, setFilter,
    sessionFilter, setSessionFilter,
    layerFilter, setLayerFilter,
    rangeFilter, setRangeFilter,
    ragSessionFilter, setRagSessionFilter,
    ragRangeFilter, setRagRangeFilter,
    selectedTransition, setSelectedTransition,
    hoverLink, setHoverLink,
    hoverPos, setHoverPos,
    pinnedTransitions, setPinnedTransitions,
    allocatorChartScope, setAllocatorChartScope,
    sessions,
    filtered,
    traceCount,
    swarmCount,
    switchCount,
    configCount,
    ragCount,
    cognitiveStats,
    ragMetrics,
    ragTimeline,
    timelineByTransition,
    sankeyLinks,
    flowMatrix,
    flowMatrix24h,
    transitionSessions,
    getAllocatorMode,
    setAllocatorModeFor,
    exportRagSnapshot,
    copyPinnedSummary,
    exportPinnedCsv,
  };
}
