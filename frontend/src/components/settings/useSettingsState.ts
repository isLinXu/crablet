import React, { useCallback, useEffect, useState } from 'react';
import { LOCAL_STORAGE_KEYS } from '../../utils/constants';
import toast from 'react-hot-toast';
import type { ApiKeyInfo, McpOverview, RoutingEvaluationReport, RoutingSettings } from '@/types/domain';
import { settingsService } from '@/services/settingsService';
import { useModelStore, type ModelProvider } from '@/store/modelStore';
import {
  type VendorName,
  type SystemConfig,
  detectVendor,
  modelSuggestionsForVendor,
  vendorToEnvVendor,
  envVendorToRouteVendor,
  validateVendorModel,
  showHttpError,
  verifySystemChatConfig,
} from './settingsHelpers';

export interface UseSettingsState {
  // API settings
  apiBaseUrl: string;
  setApiBaseUrl: (v: string) => void;
  apiKey: string;
  setApiKey: (v: string) => void;
  testingConnection: boolean;
  syncingModels: boolean;
  autoFillEndpointOnCopy: boolean;

  // Profile
  profileName: string;
  setProfileName: (v: string) => void;
  profileEmail: string;
  setProfileEmail: (v: string) => void;
  profileOrg: string;
  setProfileOrg: (v: string) => void;

  // API Keys
  apiKeys: ApiKeyInfo[];
  newKeyName: string;
  setNewKeyName: (v: string) => void;

  // MCP
  mcpOverview: McpOverview | null;

  // Providers
  draftProviders: ModelProvider[];
  setDraftProviders: React.Dispatch<React.SetStateAction<ModelProvider[]>>;

  // Routing
  routingSettings: RoutingSettings;
  setRoutingSettings: React.Dispatch<React.SetStateAction<RoutingSettings>>;
  savingRoutingSettings: boolean;
  routingReport: RoutingEvaluationReport | null;
  routingReportWindow: number;
  setRoutingReportWindow: (v: number) => void;
  loadingRoutingReport: boolean;

  // System config
  systemConfig: SystemConfig;
  setSystemConfig: React.Dispatch<React.SetStateAction<SystemConfig>>;
  loadingSystemConfig: boolean;
  savingSystemConfig: boolean;

  // Derived
  currentVendor: VendorName | null;
  keyPatternIssue: string;

  // Actions
  handlePresetSelect: (value: string) => void;
  handleSaveApiSettings: () => Promise<void>;
  handleCopyEndpoint: () => Promise<void>;
  handleToggleAutoFillEndpoint: (checked: boolean) => void;
  handleTestConnection: () => Promise<void>;
  handleSyncModels: () => Promise<void>;
  handleSaveProfile: () => void;
  handleCreateKey: () => Promise<void>;
  handleRevokeKey: (id: string) => Promise<void>;
  handleSaveProviders: () => void;
  handleSaveRoutingSettings: () => Promise<void>;
  handleRefreshRoutingReport: () => Promise<void>;
  handleSaveSystemConfig: () => Promise<void>;
  handleSetAsSystemDefault: (p: ModelProvider) => Promise<void>;
  handleAddProvider: () => void;
  applySystemVendorPreset: (vendor: VendorName) => void;
  updateProviderVendor: (id: string, vendor: VendorName) => void;
  updateProviderModelType: (id: string, modelType: 'chat' | 'image') => void;
}

export function useSettingsState(): UseSettingsState {
  const { upsertProvider, removeProvider, providers } = useModelStore();

  const [apiBaseUrl, setApiBaseUrl] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [profileName, setProfileName] = useState('');
  const [profileEmail, setProfileEmail] = useState('');
  const [profileOrg, setProfileOrg] = useState('');
  const [apiKeys, setApiKeys] = useState<ApiKeyInfo[]>([]);
  const [newKeyName, setNewKeyName] = useState('');
  const [mcpOverview, setMcpOverview] = useState<McpOverview | null>(null);
  const [testingConnection, setTestingConnection] = useState(false);
  const [syncingModels, setSyncingModels] = useState(false);
  const [autoFillEndpointOnCopy, setAutoFillEndpointOnCopy] = useState(true);
  const [draftProviders, setDraftProviders] = useState<ModelProvider[]>([]);
  const [routingSettings, setRoutingSettings] = useState<RoutingSettings>({
    enable_adaptive_routing: false,
    system2_threshold: 0.3,
    system3_threshold: 0.7,
    bandit_exploration: 0.55,
    enable_hierarchical_reasoning: true,
    deliberate_threshold: 0.58,
    meta_reasoning_threshold: 0.82,
    mcts_simulations: 24,
    mcts_exploration_weight: 1.2,
    graph_rag_entity_mode: 'hybrid',
  });
  const [savingRoutingSettings, setSavingRoutingSettings] = useState(false);
  const [routingReport, setRoutingReport] = useState<RoutingEvaluationReport | null>(null);
  const [routingReportWindow, setRoutingReportWindow] = useState(200);
  const [loadingRoutingReport, setLoadingRoutingReport] = useState(false);
  const [systemConfig, setSystemConfig] = useState<SystemConfig>({});
  const [loadingSystemConfig, setLoadingSystemConfig] = useState(false);
  const [savingSystemConfig, setSavingSystemConfig] = useState(false);

  const currentVendor = detectVendor(apiBaseUrl.trim());

  const keyPatternIssue = (() => {
    const key = apiKey.trim();
    if (!currentVendor || !key) return '';
    if (currentVendor === 'OpenAI' && !key.startsWith('sk-')) return 'OpenAI Key 通常以 sk- 开头';
    if (currentVendor === 'Anthropic' && !key.startsWith('sk-ant-')) return 'Anthropic Key 通常以 sk-ant- 开头';
    if (currentVendor === 'Google' && !/^AIza/i.test(key)) return 'Google Key 通常以 AIza 开头';
    if (currentVendor === 'Aliyun' && !key.startsWith('sk-')) return '阿里百炼 Key 通常以 sk- 开头';
    if (currentVendor === 'Kimi' && !key.startsWith('sk-')) return 'Kimi Key 通常以 sk- 开头';
    if (currentVendor === 'Ollama') return '';
    return '';
  })();

  // Load initial data
  useEffect(() => {
    const savedUrl = localStorage.getItem(LOCAL_STORAGE_KEYS.API_BASE_URL) || import.meta.env.VITE_API_URL || '/api';
    const savedKey = localStorage.getItem(LOCAL_STORAGE_KEYS.API_KEY) || '';
    const savedProfileName = localStorage.getItem('crablet-profile-name') || '';
    const savedProfileEmail = localStorage.getItem('crablet-profile-email') || '';
    const savedProfileOrg = localStorage.getItem('crablet-profile-org') || '';
    const savedAutoFill = localStorage.getItem('crablet-auto-fill-endpoint-on-copy');
    setApiBaseUrl(savedUrl);
    setApiKey(savedKey);
    setProfileName(savedProfileName);
    setProfileEmail(savedProfileEmail);
    setProfileOrg(savedProfileOrg);
    setAutoFillEndpointOnCopy(savedAutoFill !== '0');
    settingsService.listApiKeys().then((res) => setApiKeys(res || [])).catch(() => {});
    settingsService.getMcpOverview().then((res) => setMcpOverview(res || null)).catch(() => {});
    settingsService.getRoutingSettings().then((res) => { if (res) setRoutingSettings(res); }).catch(() => {});
    settingsService.getRoutingReport(200).then((res) => { if (res) setRoutingReport(res); }).catch(() => {});
    setDraftProviders(providers);
    setLoadingSystemConfig(true);
    settingsService.getSystemConfig()
      .then((res) => setSystemConfig(res?.data || res || {}))
      .catch((e) => console.error('Failed to load system config', e))
      .finally(() => setLoadingSystemConfig(false));
  }, []);

  useEffect(() => { setDraftProviders(providers); }, [providers]);

  const handlePresetSelect = useCallback((value: string) => {
    setApiBaseUrl(value);
    const vendor = detectVendor(value);
    if (vendor) {
      toast(`已切换到 ${vendor} 预设地址，请填写对应 API Key 后测试连接/同步模型`);
    }
  }, []);

  const handleSaveApiSettings = useCallback(async () => {
    let normalizedUrl = apiBaseUrl.trim();
    if (!normalizedUrl) { toast.error('API URL 不能为空'); return; }

    const providerMode = detectVendor(normalizedUrl);

    if (/^api(?:\/.*)?$/i.test(normalizedUrl)) {
      normalizedUrl = `/${normalizedUrl.replace(/^\/+/, '')}`.replace(/\/+$/, '') || '/api';
    } else if (/^\//.test(normalizedUrl)) {
      normalizedUrl = normalizedUrl.replace(/\/+$/, '') || '/api';
    } else {
      if (!/^https?:\/\//i.test(normalizedUrl)) normalizedUrl = `http://${normalizedUrl}`;
      normalizedUrl = normalizedUrl.replace(/localhost/i, '127.0.0.1').replace(/\/+$/, '');
    }

    setApiBaseUrl(normalizedUrl);
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, normalizedUrl);
    const apiKeyValue = apiKey.trim();
    const isLocalGateway = normalizedUrl.startsWith('/api') || normalizedUrl.includes('127.0.0.1:18789') || normalizedUrl.includes('localhost:18789');

    if (apiKeyValue) {
      if (apiKeyValue.startsWith('sk-') && !apiKeyValue.startsWith('sk-crablet-')) {
        toast((t) => {
          return React.createElement('div', { className: 'flex flex-col gap-2' },
            React.createElement('span', null, 'Warning: It looks like you entered an OpenAI/LLM API Key.'),
            React.createElement('span', { className: 'text-xs' }, 'The "API Key" here is for Crablet Gateway auth. For local gateway auth-off mode, leave it empty.'),
            React.createElement('div', { className: 'flex gap-2 justify-end' },
              React.createElement('button', { 
                className: 'bg-white text-black px-2 py-1 rounded border text-xs', 
                onClick: () => toast.dismiss(t.id) 
              }, 'Dismiss')
            )
          );
        }, { duration: 6000, icon: '⚠️' });
      }
      if (isLocalGateway) {
        localStorage.setItem(LOCAL_STORAGE_KEYS.API_KEY, apiKeyValue);
        localStorage.setItem(LOCAL_STORAGE_KEYS.AUTH_TOKEN, apiKeyValue);
      } else {
        localStorage.removeItem(LOCAL_STORAGE_KEYS.API_KEY);
        localStorage.removeItem(LOCAL_STORAGE_KEYS.AUTH_TOKEN);
        toast('当前是第三方API地址，已忽略Gateway鉴权Token注入');
      }
    } else {
      localStorage.removeItem(LOCAL_STORAGE_KEYS.API_KEY);
      localStorage.removeItem(LOCAL_STORAGE_KEYS.AUTH_TOKEN);
    }

    if (providerMode) {
      const model = modelSuggestionsForVendor(providerMode, 'chat')[0] || 'qwen-plus';
      upsertProvider({
        id: `${providerMode.toLowerCase()}-${model.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`,
        vendor: providerMode, model,
        modelType: /image|画|绘图|图像|文生图|doubao-image|qwen-image/i.test(model) ? 'image' : 'chat',
        version: 'latest', apiBaseUrl: normalizedUrl, apiKey: apiKeyValue, enabled: true, priority: 0,
      });
      providers.filter((p) => p.vendor.toLowerCase() === providerMode.toLowerCase()).forEach((p) => {
        upsertProvider({ ...p, apiBaseUrl: normalizedUrl, apiKey: apiKeyValue, enabled: true });
      });
      setDraftProviders((prev) =>
        prev.map((p) => p.vendor.toLowerCase() === providerMode.toLowerCase()
          ? { ...p, apiBaseUrl: normalizedUrl, apiKey: apiKeyValue, enabled: true } : p
        )
      );
      try {
        const newConfig = { openai_api_key: apiKeyValue, openai_api_base: normalizedUrl, openai_model_name: model, llm_vendor: vendorToEnvVendor(providerMode) };
        await settingsService.updateSystemConfig(newConfig);
        const confirmed: any = await settingsService.getSystemConfig();
        const confirmedConfig = confirmed?.data || confirmed || {};
        setSystemConfig((prev) => ({ ...prev, ...confirmedConfig }));
        const expected = { openai_api_key: newConfig.openai_api_key || '', openai_api_base: newConfig.openai_api_base || '', openai_model_name: newConfig.openai_model_name || '', llm_vendor: newConfig.llm_vendor || '' };
        const actual = { openai_api_key: confirmedConfig.openai_api_key || '', openai_api_base: confirmedConfig.openai_api_base || '', openai_model_name: confirmedConfig.openai_model_name || '', llm_vendor: confirmedConfig.llm_vendor || '' };
        if (JSON.stringify(expected) !== JSON.stringify(actual)) toast.error('后端配置回读校验不一致，请检查 .env 文件权限或路径');
      } catch { toast.error('已保存前端设置，但同步后端 .env 失败'); }
      toast(`已切换到${providerMode}厂商模式，配置已同步到模型路由与后端.env`);
    }
    toast.success(`API 设置已保存并生效：${normalizedUrl}`);
  }, [apiBaseUrl, apiKey, providers, upsertProvider, setDraftProviders, setSystemConfig]);

  const handleCopyEndpoint = useCallback(async () => {
    if (!currentVendor) return;
    const { VENDOR_GUIDE } = await import('./settingsHelpers');
    const text = VENDOR_GUIDE[currentVendor].endpoint;
    const afterCopied = () => {
      if (autoFillEndpointOnCopy) {
        setApiBaseUrl(text);
        localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, text);
        toast.success('已复制并自动填入推荐端点');
      } else {
        toast.success('已复制推荐端点');
      }
    };
    try { if (navigator?.clipboard?.writeText) { await navigator.clipboard.writeText(text); afterCopied(); return; } } catch {}
    try {
      const el = document.createElement('textarea'); el.value = text; document.body.appendChild(el); el.select(); document.execCommand('copy'); document.body.removeChild(el); afterCopied();
    } catch { toast.error('复制失败，请手动复制'); }
  }, [currentVendor, autoFillEndpointOnCopy]);

  const handleToggleAutoFillEndpoint = useCallback((checked: boolean) => {
    setAutoFillEndpointOnCopy(checked);
    localStorage.setItem('crablet-auto-fill-endpoint-on-copy', checked ? '1' : '0');
  }, []);

  const fetchFirstAvailable = useCallback(async <T = unknown>(endpoints: string[], headers?: Record<string, string>): Promise<T | null> => {
    for (const endpoint of endpoints) { try { const res = await fetch(endpoint, { headers }); if (res.ok) return await res.json() as T; } catch {} }
    return null;
  }, []);

  const handleTestConnection = useCallback(async () => {
    setTestingConnection(true);
    try {
      const rawBase = apiBaseUrl.trim();
      const rawKey = apiKey.trim();
      const normalizedBase = /^https?:\/\//i.test(rawBase) || rawBase.startsWith('/') ? rawBase.replace(/\/+$/, '') : `http://${rawBase}`.replace(/\/+$/, '');
      localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, normalizedBase || '/api');
      if (rawKey) { localStorage.setItem(LOCAL_STORAGE_KEYS.API_KEY, rawKey); localStorage.setItem(LOCAL_STORAGE_KEYS.AUTH_TOKEN, rawKey); }
      else { localStorage.removeItem(LOCAL_STORAGE_KEYS.API_KEY); localStorage.removeItem(LOCAL_STORAGE_KEYS.AUTH_TOKEN); }
      const autoVendor = detectVendor(normalizedBase);
      if (!autoVendor) {
        const target = normalizedBase.startsWith('/') ? `${window.location.origin}${normalizedBase}/v1/swarm/stats` : `${normalizedBase}/v1/swarm/stats`;
        const res = await fetch(target, { method: 'GET', headers: rawKey ? { Authorization: `Bearer ${rawKey}` } : undefined });
        if (res.status === 200) toast.success('连接测试通过：API可用');
        else if (res.status === 401) toast.error('连接到网关但鉴权失败(401)。请清空或更换Gateway Token后再试。');
        else if (res.status === 404) toast.error('连接到服务但路径不匹配(404)。请确认API地址应以 /api 结尾。');
        else showHttpError(res.status, '连接测试失败');
        return;
      }
      if (!rawKey) { toast.error('请先填写该厂商的 API Key'); return; }
      if (keyPatternIssue) { toast.error(`连接测试失败：${keyPatternIssue}`); return; }

      const syncModels = async () => {
        const discoveredModels: string[] = [];
        if (autoVendor === 'OpenAI') {
          const res = await fetch(`${normalizedBase.replace(/\/v1$/i, '')}/v1/models`, { headers: { Authorization: `Bearer ${rawKey}` } });
          if (!res.ok) throw new Error(`OpenAI 模型列表获取失败：HTTP ${res.status}`);
          discoveredModels.push(...((await res.json()) as { data?: Array<{ id?: string }> }).data?.map((x) => x?.id).filter(Boolean) ?? []);
        } else if (autoVendor === 'Anthropic') {
          const res = await fetch(`${normalizedBase.replace(/\/v1$/i, '')}/v1/models`, { headers: { 'x-api-key': rawKey, 'anthropic-version': '2023-06-01' } });
          if (!res.ok) throw new Error(`Anthropic 模型列表获取失败：HTTP ${res.status}`);
          discoveredModels.push(...((await res.json()) as { data?: Array<{ id?: string }> }).data?.map((x) => x?.id).filter(Boolean) ?? []);
        } else if (autoVendor === 'Google') {
          const res = await fetch(`${normalizedBase.replace(/\?key=.*/i, '')}/models?key=${encodeURIComponent(rawKey)}`);
          if (!res.ok) throw new Error(`Google 模型列表获取失败：HTTP ${res.status}`);
          discoveredModels.push(...((await res.json()) as { models?: Array<{ name?: string }> }).models?.map((x) => String(x?.name || '').replace(/^models\//, '')).filter(Boolean) ?? []);
        } else if (autoVendor === 'Aliyun') {
          const base = normalizedBase.replace(/\/+$/, '');
          const json = await fetchFirstAvailable<{ data?: Array<{ id?: string }> }>([`${base.replace(/\/v1$/i, '')}/v1/models`, `${base.replace(/\/compatible-mode\/v1$/i, '')}/compatible-mode/v1/models`], { Authorization: `Bearer ${rawKey}` });
          discoveredModels.push(...(json?.data?.map((x) => x?.id).filter(Boolean)) ?? []);
          if (!discoveredModels.length) discoveredModels.push('qwen-plus', 'qwen-turbo', 'qwen-max', 'qwen2.5-72b-instruct');
        } else if (autoVendor === 'ByteDance') {
          const base = normalizedBase.replace(/\/+$/, '');
          const json = await fetchFirstAvailable<{ data?: Array<{ id?: string }> }>([`${base.replace(/\/api\/v3$/i, '')}/api/v3/models`, `${base.replace(/\/v1$/i, '')}/v1/models`], { Authorization: `Bearer ${rawKey}` });
          discoveredModels.push(...(json?.data?.map((x) => x?.id).filter(Boolean)) ?? []);
          if (!discoveredModels.length) discoveredModels.push('doubao-pro-32k', 'doubao-pro-4k', 'doubao-lite-32k');
        } else if (autoVendor === 'Tencent') {
          const base = normalizedBase.replace(/\/+$/, '');
          const json = await fetchFirstAvailable<{ data?: Array<{ id?: string; name?: string }> }>([`${base.replace(/\/v1$/i, '')}/v1/models`, `${base.replace(/\/hunyuan\/v1$/i, '')}/hunyuan/v1/models`], { Authorization: `Bearer ${rawKey}` });
          discoveredModels.push(...(json?.data?.map((x) => x?.id || x?.name).filter(Boolean)) ?? []);
          if (!discoveredModels.length) discoveredModels.push('hunyuan-pro', 'hunyuan-standard', 'hunyuan-lite');
        }
        const picked = [...new Set(discoveredModels)].slice(0, 20);
        if (picked.length === 0) { toast.error('连接成功，但未发现可用模型'); return 0; }
        const now = Date.now();
        const discoveredProviders: ModelProvider[] = picked.map((model, idx) => {
          const existing = draftProviders.find((p) => p.vendor.toLowerCase() === autoVendor.toLowerCase() && p.model.toLowerCase() === model.toLowerCase());
          return existing || {
            id: `${autoVendor.toLowerCase()}-${model.toLowerCase().replace(/[^a-z0-9]+/g, '-')}-${now}-${idx}`,
            vendor: autoVendor, model,
            modelType: /image|画|绘图|图像|文生图|doubao-image|qwen-image/i.test(model) ? 'image' : 'chat',
            version: 'latest', apiBaseUrl: normalizedBase, apiKey: rawKey, enabled: true, priority: draftProviders.length + idx + 1,
          };
        });
        setDraftProviders((prev) => {
          const merged = [...prev];
          discoveredProviders.forEach((np) => {
            const i = merged.findIndex((p) => p.vendor.toLowerCase() === np.vendor.toLowerCase() && p.model.toLowerCase() === np.model.toLowerCase());
            if (i >= 0) merged[i] = { ...merged[i], ...np, id: merged[i].id }; else merged.push(np);
          });
          return merged;
        });
        discoveredProviders.forEach((p) => upsertProvider(p));
        return picked.length;
      };
      const count = await syncModels();
      if (count > 0) toast.success(`连接成功，已自动同步 ${autoVendor} 模型 ${count} 个`);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      const m = msg.match(/HTTP\s*(\d+)/i);
      if (m) showHttpError(Number(m[1]), '模型同步失败');
      else toast.error('连接测试失败：跨域限制或网络不可达');
    } finally { setTestingConnection(false); }
  }, [apiBaseUrl, apiKey, keyPatternIssue, draftProviders, upsertProvider, fetchFirstAvailable]);

  const handleSyncModels = useCallback(async () => {
    setSyncingModels(true);
    try {
      const normalizedBase = /^https?:\/\//i.test(apiBaseUrl.trim()) || apiBaseUrl.trim().startsWith('/') ? apiBaseUrl.trim().replace(/\/+$/, '') : `http://${apiBaseUrl.trim()}`.replace(/\/+$/, '');
      const autoVendor = detectVendor(normalizedBase);
      if (!autoVendor) { toast.error('当前API地址不是已支持的模型厂商地址，请切换到OpenAI/Anthropic/Google/阿里百炼/腾讯/豆包'); return; }
      if (!apiKey.trim()) { toast.error('请先填写该厂商的 API Key'); return; }
      await handleTestConnection();
    } finally { setSyncingModels(false); }
  }, [apiBaseUrl, apiKey, handleTestConnection]);

  const handleSaveProfile = useCallback(() => {
    localStorage.setItem('crablet-profile-name', profileName.trim());
    localStorage.setItem('crablet-profile-email', profileEmail.trim());
    localStorage.setItem('crablet-profile-org', profileOrg.trim());
    toast.success('账户信息已保存');
  }, [profileName, profileEmail, profileOrg]);

  const handleCreateKey = useCallback(async () => {
    const name = newKeyName.trim();
    if (!name) return;
    try {
      const res: any = await settingsService.createApiKey(name);
      toast.success(`新密钥已创建：${res?.key || ''}`);
      setNewKeyName('');
      const refreshed: any = await settingsService.listApiKeys();
      setApiKeys(refreshed || []);
    } catch { toast.error('创建密钥失败'); }
  }, [newKeyName]);

  const handleRevokeKey = useCallback(async (id: string) => {
    try { await settingsService.revokeApiKey(id); setApiKeys((prev) => prev.filter((k) => k.id !== id)); toast.success('密钥已撤销'); }
    catch { toast.error('撤销密钥失败'); }
  }, []);

  const handleSaveProviders = useCallback(() => { draftProviders.forEach((p) => upsertProvider(p)); toast.success('多厂商模型配置已保存'); }, [draftProviders, upsertProvider]);

  const handleSaveRoutingSettings = useCallback(async () => {
    const s2 = Number(routingSettings.system2_threshold);
    const s3 = Number(routingSettings.system3_threshold);
    const exp = Number(routingSettings.bandit_exploration);
    const deliberate = Number(routingSettings.deliberate_threshold);
    const meta = Number(routingSettings.meta_reasoning_threshold);
    const simulations = Number(routingSettings.mcts_simulations);
    const mctsExploration = Number(routingSettings.mcts_exploration_weight);
    if (Number.isNaN(s2) || s2 < 0 || s2 > 1) { toast.error('system2_threshold 必须在 0~1'); return; }
    if (Number.isNaN(s3) || s3 < 0 || s3 > 1) { toast.error('system3_threshold 必须在 0~1'); return; }
    if (Number.isNaN(exp) || exp < 0.05 || exp > 2) { toast.error('bandit_exploration 必须在 0.05~2'); return; }
    if (Number.isNaN(deliberate) || deliberate < 0 || deliberate > 1) { toast.error('deliberate_threshold 必须在 0~1'); return; }
    if (Number.isNaN(meta) || meta < 0 || meta > 1) { toast.error('meta_reasoning_threshold 必须在 0~1'); return; }
    if (Number.isNaN(simulations) || simulations < 1 || simulations > 512) { toast.error('mcts_simulations 必须在 1~512'); return; }
    if (Number.isNaN(mctsExploration) || mctsExploration < 0.1 || mctsExploration > 3) { toast.error('mcts_exploration_weight 必须在 0.1~3'); return; }
    setSavingRoutingSettings(true);
    try {
      const saved = await settingsService.updateRoutingSettings({
        enable_adaptive_routing: routingSettings.enable_adaptive_routing, system2_threshold: s2, system3_threshold: s3,
        bandit_exploration: exp, enable_hierarchical_reasoning: routingSettings.enable_hierarchical_reasoning,
        deliberate_threshold: deliberate, meta_reasoning_threshold: meta,
        mcts_simulations: Math.round(simulations), mcts_exploration_weight: mctsExploration,
        graph_rag_entity_mode: routingSettings.graph_rag_entity_mode,
      });
      if (saved) setRoutingSettings(saved);
      toast.success('元认知路由配置已保存');
    } catch { toast.error('保存元认知路由配置失败'); } finally { setSavingRoutingSettings(false); }
  }, [routingSettings]);

  const handleRefreshRoutingReport = useCallback(async () => {
    const window = Math.max(10, Math.min(2000, Number(routingReportWindow) || 200));
    setRoutingReportWindow(window);
    setLoadingRoutingReport(true);
    try { const report = await settingsService.getRoutingReport(window); if (report) setRoutingReport(report); }
    catch { toast.error('读取路由评估报告失败'); } finally { setLoadingRoutingReport(false); }
  }, [routingReportWindow]);

  const handleSaveSystemConfig = useCallback(async () => {
    const base = String(systemConfig.openai_api_base || '').trim();
    const model = String(systemConfig.openai_model_name || '').trim();
    const key = String(systemConfig.openai_api_key || '').trim();
    const inferredVendor = vendorToEnvVendor(detectVendor(base));
    const vendor = String(systemConfig.llm_vendor || inferredVendor || 'custom').trim().toLowerCase();
    const modelIssue = validateVendorModel(vendor, model);
    if (modelIssue) { toast.error(modelIssue); return; }
    setSavingSystemConfig(true);
    try {
      await verifySystemChatConfig({ base, model, key, vendor });
      const payload = { ...systemConfig, openai_api_base: base, openai_model_name: model, openai_api_key: key, llm_vendor: vendor };
      await settingsService.updateSystemConfig(payload);
      const confirmed: any = await settingsService.getSystemConfig();
      const confirmedConfig = confirmed?.data || confirmed || {};
      setSystemConfig((prev) => ({ ...prev, ...confirmedConfig }));
      const expected = { openai_api_key: key, openai_api_base: base, openai_model_name: model, llm_vendor: vendor };
      const actual = { openai_api_key: confirmedConfig.openai_api_key || '', openai_api_base: confirmedConfig.openai_api_base || '', openai_model_name: confirmedConfig.openai_model_name || '', llm_vendor: confirmedConfig.llm_vendor || '' };
      if (JSON.stringify(expected) !== JSON.stringify(actual)) { toast.error('配置保存后回读不一致，请检查后端 .env 写入路径'); return; }
      toast.success('System config verified and synced to local .env.');
    } catch { toast.error('保存失败：模型配置校验未通过或后端写入失败'); } finally { setSavingSystemConfig(false); }
  }, [systemConfig]);

  const handleSetAsSystemDefault = useCallback(async (p: ModelProvider) => {
    if (!window.confirm(`确定要将 [${p.vendor}] ${p.model} 设为后端默认配置吗？\n这将覆盖 .env 文件中的 KEY/BASE/MODEL 配置。`)) return;
    if ((p.modelType || 'chat') !== 'chat') { toast.error('系统默认模型必须是 chat 类型，不能使用 image 模型'); return; }
    const vendor = vendorToEnvVendor(p.vendor);
    const modelIssue = validateVendorModel(vendor, p.model);
    if (modelIssue) { toast.error(modelIssue); return; }
    const newConfig = { openai_api_key: p.apiKey, openai_api_base: p.apiBaseUrl, openai_model_name: p.model, llm_vendor: vendor };
    setSavingSystemConfig(true);
    try {
      await verifySystemChatConfig({ base: String(p.apiBaseUrl || '').trim(), model: String(p.model || '').trim(), key: String(p.apiKey || '').trim(), vendor: vendor.toLowerCase() });
      await settingsService.updateSystemConfig(newConfig);
      const confirmed: any = await settingsService.getSystemConfig();
      const confirmedConfig = confirmed?.data || confirmed || {};
      setSystemConfig(prev => ({ ...prev, ...confirmedConfig }));
      toast.success('已更新系统默认配置并完成回读校验');
    } catch { toast.error('保存失败：模型校验或后端写入未通过'); } finally { setSavingSystemConfig(false); }
  }, []);

  const handleAddProvider = useCallback(() => {
    const ts = Date.now();
    setDraftProviders((prev) => [...prev, { id: `custom-${ts}`, vendor: 'Custom', model: 'model-name', modelType: 'chat', version: 'v1', apiBaseUrl: '', apiKey: '', enabled: true, priority: prev.length + 1 }]);
  }, []);

  const applySystemVendorPreset = useCallback((vendor: VendorName) => {
    const { VENDOR_DEFAULTS } = require('./settingsHelpers');
    const defaults = VENDOR_DEFAULTS[vendor];
    const suggestedModel = defaults.chatModels[0] || '';
    setSystemConfig((prev) => ({ ...prev, llm_vendor: vendorToEnvVendor(vendor), openai_api_base: defaults.endpoint || prev.openai_api_base || '', openai_model_name: suggestedModel || prev.openai_model_name || '' }));
    toast(`已应用 ${vendor} 推荐端点与模型`);
  }, []);

  const updateProviderVendor = useCallback((id: string, vendor: VendorName) => {
    setDraftProviders((prev) => prev.map((p) => {
      if (p.id !== id) return p;
      const { VENDOR_DEFAULTS } = require('./settingsHelpers');
      const defaults = VENDOR_DEFAULTS[vendor];
      const modelType = p.modelType || 'chat';
      const suggestions = modelSuggestionsForVendor(vendor, modelType);
      return { ...p, vendor, apiBaseUrl: defaults.endpoint || p.apiBaseUrl, model: suggestions[0] || p.model };
    }));
  }, []);

  const updateProviderModelType = useCallback((id: string, modelType: 'chat' | 'image') => {
    setDraftProviders((prev) => prev.map((p) => {
      if (p.id !== id) return p;
      const suggestions = modelSuggestionsForVendor(p.vendor, modelType);
      const model = suggestions.includes(p.model) ? p.model : (suggestions[0] || p.model);
      return { ...p, modelType, model };
    }));
  }, []);

  return {
    apiBaseUrl, setApiBaseUrl, apiKey, setApiKey, testingConnection, syncingModels, autoFillEndpointOnCopy,
    profileName, setProfileName, profileEmail, setProfileEmail, profileOrg, setProfileOrg,
    apiKeys, newKeyName, setNewKeyName, mcpOverview,
    draftProviders, setDraftProviders,
    routingSettings, setRoutingSettings, savingRoutingSettings, routingReport, routingReportWindow, setRoutingReportWindow, loadingRoutingReport,
    systemConfig, setSystemConfig, loadingSystemConfig, savingSystemConfig,
    currentVendor, keyPatternIssue,
    handlePresetSelect, handleSaveApiSettings, handleCopyEndpoint, handleToggleAutoFillEndpoint,
    handleTestConnection, handleSyncModels, handleSaveProfile, handleCreateKey, handleRevokeKey,
    handleSaveProviders, handleSaveRoutingSettings, handleRefreshRoutingReport,
    handleSaveSystemConfig, handleSetAsSystemDefault, handleAddProvider,
    applySystemVendorPreset, updateProviderVendor, updateProviderModelType,
  };
}
