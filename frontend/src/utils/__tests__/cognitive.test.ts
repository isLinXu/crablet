import { describe, expect, it } from 'vitest';
import { cognitiveLayerLabel, inferCognitiveLayer } from '../cognitive';

describe('cognitive utils', () => {
  it('infers system1 signals from fast-path text', () => {
    expect(inferCognitiveLayer({ text: 'FastRespond due to trie hit' })).toBe('system1');
  });

  it('infers system2 signals from reasoning traces', () => {
    expect(
      inferCognitiveLayer({ thought: 'Need to reason carefully before answering this prompt' })
    ).toBe('system2');
  });

  it('infers system3 signals from planning and verification traces', () => {
    expect(
      inferCognitiveLayer({ action: 'planner', observation: 'verify delegated subtasks' })
    ).toBe('system3');
  });

  it('falls back to unknown when no signal exists', () => {
    expect(inferCognitiveLayer({})).toBe('unknown');
    expect(cognitiveLayerLabel('unknown')).toBe('Unknown');
  });

  it('returns readable labels for known layers', () => {
    expect(cognitiveLayerLabel('system1')).toBe('System 1');
    expect(cognitiveLayerLabel('system2')).toBe('System 2');
    expect(cognitiveLayerLabel('system3')).toBe('System 3');
  });
});
