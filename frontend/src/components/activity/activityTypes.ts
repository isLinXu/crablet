// Activity types and helpers for ActivityCenter

export interface RagRef { source: string; score: number; content: string; }
export interface ParsedRagObservation {
  retrieval: string;
  refs_count: number;
  graph_entities: string[];
  refs: RagRef[];
}
export interface AllocatorCandidate {
  role: string;
  final_score: number;
  expertise_match: number;
  ucb_bonus: number;
  performance_bonus: number;
  preferred_bonus: number;
  load_penalty: number;
}
export interface ParsedAllocatorDecision {
  candidates: AllocatorCandidate[];
}

export function parseRagObservation(observation: string | undefined | null): ParsedRagObservation | null {
  if (!observation) return null;
  try {
    return JSON.parse(observation) as ParsedRagObservation;
  } catch {
    return null;
  }
}

export function parseAllocatorDecision(_eventType: string, content: string | undefined | null): ParsedAllocatorDecision | null {
  if (!content) return null;
  try {
    return JSON.parse(content) as ParsedAllocatorDecision;
  } catch {
    return null;
  }
}

export function ragModeLabel(mode: string): string {
  const labels: Record<string, string> = {
    semantic: '语义检索',
    graph: 'GraphRAG',
    hybrid: '混合检索',
    keyword: '关键词',
  };
  return labels[mode] || mode;
}

export function formatSwarmContent(eventType: string, content: string | undefined | null): string {
  if (!content) return '';
  try {
    const parsed = JSON.parse(content);
    if (eventType === 'AllocatorDecision' && parsed.candidates) {
      return `分配到 ${parsed.candidates[0]?.role || 'unknown'}（评分 ${parsed.candidates[0]?.final_score?.toFixed(2) || 'N/A'}）`;
    }
    return typeof parsed === 'string' ? parsed : JSON.stringify(parsed, null, 2).slice(0, 200);
  } catch {
    return content.slice(0, 200);
  }
}
