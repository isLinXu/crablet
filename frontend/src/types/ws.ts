export type WsNormalizedEvent =
  | { kind: 'pong' }
  | { kind: 'user_input'; content: string }
  | { kind: 'thought'; thought: string }
  | { kind: 'cognitive_layer'; layer: 'system1' | 'system2' | 'system3' | 'unknown' }
  | { kind: 'tool_start'; tool: string; args: string }
  | { kind: 'tool_finish'; output: string }
  | { kind: 'swarm_activity'; taskId: string; from: string; to: string; messageType: string; content: string }
  | { kind: 'graph_rag_mode_changed'; fromMode: string; toMode: string }
  | { kind: 'response'; content: string }
  | { kind: 'error'; message: string }
  | { kind: 'unknown' };

type LegacyJson = {
  type?: string;
  UserInput?: { content?: string };
  ThoughtGenerated?: string;
  CognitiveLayer?: { layer?: string };
  ToolExecutionStarted?: { tool?: string; args?: string };
  ToolExecutionFinished?: { output?: string };
  SwarmActivity?: { task_id?: string; from?: string; to?: string; message_type?: string; content?: string };
  GraphRagEntityModeChanged?: { from_mode?: string; to_mode?: string };
  ResponseGenerated?: { content?: string };
  Error?: { message?: string };
};

export const parseWsEvent = (raw: string): WsNormalizedEvent => {
  try {
    const data = JSON.parse(raw) as LegacyJson;
    if (data.type === 'pong') return { kind: 'pong' };
    if (data.UserInput?.content) return { kind: 'user_input', content: data.UserInput.content };
    if (typeof data.ThoughtGenerated === 'string') return { kind: 'thought', thought: data.ThoughtGenerated };
    if (typeof data.CognitiveLayer?.layer === 'string') {
      const raw = data.CognitiveLayer.layer.toLowerCase();
      const layer = raw === 'system1' || raw === 'system2' || raw === 'system3' ? raw : 'unknown';
      return { kind: 'cognitive_layer', layer };
    }
    if (data.ToolExecutionStarted?.tool) {
      return { kind: 'tool_start', tool: data.ToolExecutionStarted.tool, args: data.ToolExecutionStarted.args ?? '' };
    }
    if (typeof data.ToolExecutionFinished?.output === 'string') {
      return { kind: 'tool_finish', output: data.ToolExecutionFinished.output };
    }
    if (data.SwarmActivity?.task_id) {
      return {
        kind: 'swarm_activity',
        taskId: data.SwarmActivity.task_id,
        from: data.SwarmActivity.from ?? '',
        to: data.SwarmActivity.to ?? '',
        messageType: data.SwarmActivity.message_type ?? '',
        content: data.SwarmActivity.content ?? '',
      };
    }
    if (typeof data.GraphRagEntityModeChanged?.from_mode === 'string' && typeof data.GraphRagEntityModeChanged?.to_mode === 'string') {
      return {
        kind: 'graph_rag_mode_changed',
        fromMode: data.GraphRagEntityModeChanged.from_mode,
        toMode: data.GraphRagEntityModeChanged.to_mode,
      };
    }
    if (typeof data.ResponseGenerated?.content === 'string') return { kind: 'response', content: data.ResponseGenerated.content };
    if (typeof data.Error?.message === 'string') return { kind: 'error', message: data.Error.message };
  } catch {
    if (raw.startsWith('RESPONSE:')) return { kind: 'response', content: raw.slice(9) };
    if (raw.startsWith('THOUGHT:')) return { kind: 'thought', thought: raw.slice(8) };
    if (raw.startsWith('COGNITIVE_LAYER:')) {
      const layer = raw.slice(16).trim().toLowerCase();
      const normalized = layer === 'system1' || layer === 'system2' || layer === 'system3' ? layer : 'unknown';
      return { kind: 'cognitive_layer', layer: normalized };
    }
    if (raw.startsWith('ERROR:')) return { kind: 'error', message: raw.slice(6) };
  }
  return { kind: 'unknown' };
};
