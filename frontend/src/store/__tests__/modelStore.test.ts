import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/services/settingsService', () => ({
  settingsService: {
    getSystemConfig: vi.fn(),
  },
}));

vi.mock('@/utils/constants', () => ({
  getApiBaseUrl: vi.fn(() => 'http://127.0.0.1:18789/api'),
}));

import { settingsService } from '@/services/settingsService';
import { useModelStore, type ModelProvider } from '../modelStore';

const baseProviders: ModelProvider[] = [
  {
    id: 'openai-fast',
    vendor: 'OpenAI',
    model: 'gpt-4o-mini',
    modelType: 'chat',
    version: '2024-07',
    apiBaseUrl: 'https://api.openai.com/v1',
    apiKey: '',
    enabled: true,
    priority: 1,
  },
  {
    id: 'anthropic-deep',
    vendor: 'Anthropic',
    model: 'claude-3-5-sonnet',
    modelType: 'chat',
    version: '2024-10',
    apiBaseUrl: 'https://api.anthropic.com/v1',
    apiKey: 'anthropic-key',
    enabled: true,
    priority: 2,
  },
  {
    id: 'image-pro',
    vendor: 'ImageLab',
    model: 'flux-image',
    modelType: 'image',
    version: '2025-01',
    apiBaseUrl: 'https://image.example.com/v1',
    apiKey: 'image-key',
    enabled: true,
    priority: 3,
  },
];

describe('useModelStore', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.mocked(settingsService.getSystemConfig).mockReset();
    useModelStore.setState({
      providers: structuredClone(baseProviders),
      sessionManualProvider: {},
    });
  });

  it('upserts providers and infers image model types from model names', () => {
    useModelStore.getState().upsertProvider({
      id: 'designer',
      vendor: 'Custom',
      model: 'sdxl-image-pro',
      version: '1',
      apiBaseUrl: 'https://custom.example.com',
      apiKey: '',
      enabled: true,
      priority: 10,
    });

    const provider = useModelStore.getState().providers.find((item) => item.id === 'designer');
    expect(provider?.modelType).toBe('image');
  });

  it('prefers a keyed provider when falling back without a session override', () => {
    const resolution = useModelStore.getState().resolveForPrompt(null, 'hello there');
    expect(resolution.providerId).toBe('anthropic-deep');
    expect(resolution.reason).toBe('no-session-fallback');
  });

  it('respects manual session selections when the provider is enabled', () => {
    useModelStore.getState().setSessionManualProvider('session-1', 'openai-fast');
    const resolution = useModelStore.getState().resolveForPrompt('session-1', 'please analyze');

    expect(resolution.providerId).toBe('anthropic-deep');
    expect(resolution.reason).toBe('manual-selection');
  });

  it('routes image-generation prompts to image providers', () => {
    const resolution = useModelStore
      .getState()
      .resolveForPrompt('session-2', '请帮我生成图片海报', 'balanced');

    expect(resolution.providerId).toBe('image-pro');
    expect(resolution.modelType).toBe('image');
    expect(resolution.reason).toBe('auto-image_gen-balanced');
  });

  it('syncs backend models into the store', async () => {
    vi.mocked(settingsService.getSystemConfig).mockResolvedValue({
      llm_vendor: 'openai',
      openai_model_name: 'gpt-4.1',
      openai_api_base: 'https://api.openai.com/v1',
    });

    await useModelStore.getState().syncFromBackend();

    expect(useModelStore.getState().providers.some((provider) => provider.id === 'backend-gpt-4.1')).toBe(true);
  });
});
