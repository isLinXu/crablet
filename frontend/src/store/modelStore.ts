import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { settingsService } from '@/services/settingsService';

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
  syncFromBackend: () => Promise<void>;
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

const pickFallback = (providers: ModelProvider[], modelType: 'chat' | 'image' = 'chat') => {
  const enabled = providers.filter((p) => p.enabled);
  const typed = enabled.filter((p) => inferModelType(p) === modelType);
  return [...(typed.length > 0 ? typed : enabled)].sort((a, b) => a.priority - b.priority)[0];
};

const preferProviderWithKey = (
  candidate: ModelProvider,
  providers: ModelProvider[],
  preferredType: 'chat' | 'image' = 'chat'
) => {
  if (candidate.apiKey?.trim()) return candidate;
  const enabled = providers.filter((p) => p.enabled);
  const typed = enabled.filter((p) => inferModelType(p) === preferredType);
  const withKey = [...providers]
    .filter((p) => typed.includes(p) && !!p.apiKey?.trim())
    .sort((a, b) => a.priority - b.priority)[0];
  if (withKey) return withKey;
  return candidate;
};

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
        const chatProviders = enabled.filter((p: ModelProvider) => inferModelType(p) === 'chat');
        const imageProviders = enabled.filter((p: ModelProvider) => inferModelType(p) === 'image');
        const fallback = pickFallback(enabled, 'chat') || defaultProviders[0];
        if (!sessionId) {
          const effective = preferProviderWithKey(fallback, enabled, 'chat');
          return {
            providerId: effective.id,
            vendor: effective.vendor,
            model: effective.model,
            modelType: inferModelType(effective),
            apiBaseUrl: effective.apiBaseUrl,
            apiKey: effective.apiKey,
            version: effective.version,
            reason: 'no-session-fallback',
          };
        }
        const manual = sessionManualProvider[sessionId];
        if (manual) {
          const selected = providers.find((p: ModelProvider) => p.id === manual && p.enabled) || fallback;
          const effective = preferProviderWithKey(selected, enabled, inferModelType(selected));
          return {
            providerId: effective.id,
            vendor: effective.vendor,
            model: effective.model,
            modelType: inferModelType(effective),
            apiBaseUrl: effective.apiBaseUrl,
            apiKey: effective.apiKey,
            version: effective.version,
            reason: 'manual-selection',
          };
        }
        const q = classifyPrompt(prompt);
        const byVendor = (vendor: string) => enabled.find((p: ModelProvider) => p.vendor.toLowerCase() === vendor.toLowerCase());
        let selected = fallback;
        if (q === 'image_gen' && imageProviders.length > 0) {
          selected = [...imageProviders].sort((a, b) => a.priority - b.priority)[0] || fallback;
        }
        if (q === 'multimodal') selected = byVendor('Google') || pickFallback(enabled, 'chat') || fallback;
        if (q === 'coding') selected = byVendor('OpenAI') || pickFallback(enabled, 'chat') || fallback;
        if (q === 'analysis') selected = byVendor('Anthropic') || pickFallback(enabled, 'chat') || fallback;
        if (q !== 'image_gen' && inferModelType(selected) !== 'chat') {
          selected = pickFallback(enabled, 'chat') || selected;
        }
        if (priority === 'speed') {
          const speedPool = q === 'image_gen' ? imageProviders : (chatProviders.length > 0 ? chatProviders : enabled);
          selected = [...speedPool].sort((a, b) => a.priority - b.priority)[0] || selected;
        }
        if (priority === 'quality') {
          const qualityPool = q === 'image_gen' ? imageProviders : (chatProviders.length > 0 ? chatProviders : enabled);
          selected = [...qualityPool].sort((a, b) => b.priority - a.priority)[0] || selected;
        }
        const effective = preferProviderWithKey(selected, enabled, q === 'image_gen' ? 'image' : 'chat');
        return {
          providerId: effective.id,
          vendor: effective.vendor,
          model: effective.model,
          modelType: inferModelType(effective),
          apiBaseUrl: effective.apiBaseUrl,
          apiKey: effective.apiKey,
          version: effective.version,
          reason: `auto-${q}-${priority}`,
        };
      },
      syncFromBackend: async () => {
        try {
          const config: any = await settingsService.getSystemConfig();
          const modelName = config?.openai_model_name;
          if (modelName) {
            const { providers, upsertProvider } = get();
            const exists = providers.some((p: ModelProvider) => p.model === modelName);
            if (!exists) {
               upsertProvider({
                 id: `backend-${modelName}`,
                 vendor: 'Backend',
                 model: modelName,
                 modelType: 'chat',
                 version: 'latest',
                 apiBaseUrl: 'http://127.0.0.1:18789/api',
                 apiKey: '',
                 enabled: true,
                 priority: 0,
               });
            }
          }
        } catch (e) {
          console.warn('Failed to sync model from backend', e);
        }
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
