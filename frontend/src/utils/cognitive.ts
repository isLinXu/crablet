export type CognitiveLayer = 'system1' | 'system2' | 'system3' | 'unknown';

const toLayer = (text: string): CognitiveLayer => {
  const v = text.toLowerCase();
  if (v.includes('system 1') || v.includes('system1') || v.includes('trie hit') || v.includes('fastrespond')) return 'system1';
  if (v.includes('system 2') || v.includes('system2') || v.includes('reason') || v.includes('deliberate')) return 'system2';
  if (v.includes('system 3') || v.includes('system3') || v.includes('plan') || v.includes('planner') || v.includes('verify')) return 'system3';
  return 'unknown';
};

export const inferCognitiveLayer = (input: {
  thought?: string;
  action?: string;
  observation?: string;
  text?: string;
}): CognitiveLayer => {
  const joined = [input.text || '', input.thought || '', input.action || '', input.observation || ''].join(' ').trim();
  if (!joined) return 'unknown';
  return toLayer(joined);
};

export const cognitiveLayerLabel = (layer: CognitiveLayer) => {
  if (layer === 'system1') return 'System 1';
  if (layer === 'system2') return 'System 2';
  if (layer === 'system3') return 'System 3';
  return 'Unknown';
};
