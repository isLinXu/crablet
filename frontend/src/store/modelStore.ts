import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export interface ModelProvider {
  id: string;
  vendor: string;
  model: string;
  modelType?: 'chat' | 'image';
  version: string;
  apiBaseUrl: string;
  apiKey: string;
  enabled: boolean;
  priority: number;
}

export interface ModelResolution {
  providerId: string;
  vendor: string;
  model: string;
  modelType: 'chat' | 'image';
  apiBaseUrl: string;
  apiKey: string;
  version: string;
  reason: string;
}

interface ModelState {
  providers: ModelProvider[];
  sessionManualProvider: Record<string, string>;
  upsertProvider: (provider: ModelProvider) => void;
  removeProvider: (providerId: string) => void;
  setSessionManualProvider: (sessionId: string, providerId: string | null) => void;
  resolveForPrompt: (sessionId: string | null, prompt: string, priority?: 'speed' | 'quality' | 'balanced') => ModelResolution;
}

const defaultProviders: ModelProvider[] = [
  {
    id: 'openai-gpt-4o-mini',
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
    id: 'anthropic-claude-sonnet',
    vendor: 'Anthropic',
    model: 'claude-3-5-sonnet',
    modelType: 'chat',
    version: '2024-10',
    apiBaseUrl: 'https://api.anthropic.com/v1',
    apiKey: '',
    enabled: true,
    priority: 2,
  },
  {
    id: 'google-gemini-1.5-pro',
    vendor: 'Google',
    model: 'gemini-1.5-pro',
    modelType: 'chat',
    version: '2024-09',
    apiBaseUrl: 'https://generativelanguage.googleapis.com/v1beta',
    apiKey: '',
    enabled: true,
    priority: 3,
  },
];

const inferModelType = (provider: Pick<ModelProvider, 'modelType' | 'model' | 'vendor'>): 'chat' | 'image' => {
  if (provider.modelType === 'chat' || provider.modelType === 'image') return provider.modelType;
  const model = String(provider.model || '').toLowerCase();
  const vendor = String(provider.vendor || '').toLowerCase();
  if (/(qwen-image|doubao-image|stable-diffusion|flux|sdxl|image)/i.test(model)) return 'image';
  if (/(image|绘图|画图|文生图)/i.test(vendor)) return 'image';
  return 'chat';
};

const classifyPrompt = (prompt: string) => {
  const t = prompt.toLowerCase();
  if (/(draw|绘图|画图|画一个|生成图片|生成图像|海报|插画|image generation|text-to-image)/.test(t)) return 'image_gen';
  if (/(image|图片|ocr|图像|video|视频|audio|音频|multimodal|多模态)/.test(t)) return 'multimodal';
  if (/(code|代码|debug|bug|refactor|typescript|rust|python|java)/.test(t)) return 'coding';
  if (/(analyze|分析|reason|推理|总结|对比|长文)/.test(t)) return 'analysis';
  return 'general';
};

const pickFallback = (providers: ModelProvider[]) =>
  [...providers]
    .filter((p) => p.enabled)
    .sort((a, b) => a.priority - b.priority)[0];

export const useModelStore = create<ModelState>()(
  persist(
    (set, get) => ({
      providers: defaultProviders,
      sessionManualProvider: {},
      upsertProvider: (provider: ModelProvider) =>
        set((state: ModelState) => {
          const normalizedProvider = { ...provider, modelType: inferModelType(provider) };
          const exists = state.providers.some((p: ModelProvider) => p.id === provider.id);
          return {
            providers: exists
              ? state.providers.map((p: ModelProvider) => (p.id === provider.id ? normalizedProvider : p))
              : [...state.providers, normalizedProvider],
          };
        }),
      removeProvider: (providerId: string) =>
        set((state: ModelState) => ({
          providers: state.providers.filter((p: ModelProvider) => p.id !== providerId),
        })),
      setSessionManualProvider: (sessionId: string, providerId: string | null) =>
        set((state: ModelState) => {
          const next = { ...state.sessionManualProvider };
          if (!providerId) delete next[sessionId];
          else next[sessionId] = providerId;
          return { sessionManualProvider: next };
        }),
      resolveForPrompt: (sessionId: string | null, prompt: string, priority: 'speed' | 'quality' | 'balanced' = 'balanced') => {
        const { providers, sessionManualProvider } = get();
        const enabled = providers.filter((p: ModelProvider) => p.enabled);
        const fallback = pickFallback(enabled) || defaultProviders[0];
        if (!sessionId) {
          return {
            providerId: fallback.id,
            vendor: fallback.vendor,
            model: fallback.model,
            modelType: inferModelType(fallback),
            apiBaseUrl: fallback.apiBaseUrl,
            apiKey: fallback.apiKey,
            version: fallback.version,
            reason: 'no-session-fallback',
          };
        }
        const manual = sessionManualProvider[sessionId];
        if (manual) {
          const selected = providers.find((p: ModelProvider) => p.id === manual && p.enabled) || fallback;
          return {
            providerId: selected.id,
            vendor: selected.vendor,
            model: selected.model,
            modelType: inferModelType(selected),
            apiBaseUrl: selected.apiBaseUrl,
            apiKey: selected.apiKey,
            version: selected.version,
            reason: 'manual-selection',
          };
        }
        const q = classifyPrompt(prompt);
        const byVendor = (vendor: string) => enabled.find((p: ModelProvider) => p.vendor.toLowerCase() === vendor.toLowerCase());
        const imageProviders = enabled.filter((p: ModelProvider) => inferModelType(p) === 'image');
        let selected = fallback;
        if (q === 'image_gen' && imageProviders.length > 0) {
          selected = [...imageProviders].sort((a, b) => a.priority - b.priority)[0] || fallback;
        }
        if (q === 'multimodal') selected = byVendor('Google') || fallback;
        if (q === 'coding') selected = byVendor('OpenAI') || fallback;
        if (q === 'analysis') selected = byVendor('Anthropic') || fallback;
        if (priority === 'speed') selected = [...enabled].sort((a, b) => a.priority - b.priority)[0] || selected;
        if (priority === 'quality') selected = [...enabled].sort((a, b) => b.priority - a.priority)[0] || selected;
        return {
          providerId: selected.id,
          vendor: selected.vendor,
          model: selected.model,
          modelType: inferModelType(selected),
          apiBaseUrl: selected.apiBaseUrl,
          apiKey: selected.apiKey,
          version: selected.version,
          reason: `auto-${q}-${priority}`,
        };
      },
    }),
    {
      name: 'model-routing-store',
      version: 1,
      merge: (persisted: any, current: any) => {
        const state = { ...current, ...(persisted || {}) };
        if (Array.isArray(state.providers)) {
          state.providers = state.providers.map((p: ModelProvider) => ({
            ...p,
            modelType: inferModelType(p),
          }));
        }
        return state;
      },
      partialize: (s) => ({
        providers: s.providers,
        sessionManualProvider: s.sessionManualProvider,
      }),
    }
  )
);
