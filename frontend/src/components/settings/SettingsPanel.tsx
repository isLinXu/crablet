import React, { useEffect, useState } from 'react';
import { useThemeStore } from '../../store/themeStore';
import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card';
import { Settings, Moon, Sun, Monitor, KeyRound, Link, User, Trash2, Server, Save } from 'lucide-react';
import clsx from 'clsx';
import { Input } from '../ui/Input';
import { Button } from '../ui/Button';
import { LOCAL_STORAGE_KEYS } from '../../utils/constants';
import toast from 'react-hot-toast';
import type { ApiKeyInfo, McpOverview, RoutingEvaluationReport, RoutingSettings } from '@/types/domain';
import { settingsService } from '@/services/settingsService';
import { useModelStore, type ModelProvider } from '@/store/modelStore';
import { api } from '@/services/api';

type VendorName = 'OpenAI' | 'Anthropic' | 'Google' | 'Aliyun' | 'Tencent' | 'ByteDance' | 'Kimi' | 'ZhiPu' | 'Ollama' | 'Custom';

const VENDOR_OPTIONS: VendorName[] = ['OpenAI', 'Anthropic', 'Google', 'Aliyun', 'Tencent', 'ByteDance', 'Kimi', 'ZhiPu', 'Ollama', 'Custom'];

const VENDOR_DEFAULTS: Record<VendorName, { endpoint: string; chatModels: string[]; imageModels: string[] }> = {
  OpenAI: { endpoint: 'https://api.openai.com/v1', chatModels: ['gpt-4o-mini', 'gpt-4o', 'o3-mini'], imageModels: ['gpt-image-1'] },
  Anthropic: { endpoint: 'https://api.anthropic.com/v1', chatModels: ['claude-3-5-sonnet', 'claude-3-5-haiku'], imageModels: [] },
  Google: { endpoint: 'https://generativelanguage.googleapis.com/v1beta', chatModels: ['gemini-1.5-pro', 'gemini-1.5-flash'], imageModels: [] },
  Aliyun: { endpoint: 'https://dashscope.aliyuncs.com/compatible-mode/v1', chatModels: ['qwen-plus', 'qwen-max', 'qwen-turbo'], imageModels: ['qwen-image-2.0', 'wanx2.1-t2i-turbo'] },
  Tencent: { endpoint: 'https://api.hunyuan.cloud.tencent.com/v1', chatModels: ['hunyuan-pro', 'hunyuan-lite'], imageModels: ['hunyuan-image'] },
  ByteDance: { endpoint: 'https://ark.cn-beijing.volces.com/api/v3', chatModels: ['doubao-pro-32k', 'doubao-lite-4k'], imageModels: ['doubao-image'] },
  Kimi: { endpoint: 'https://api.moonshot.cn/v1', chatModels: ['moonshot-v1-8k', 'moonshot-v1-32k'], imageModels: [] },
  ZhiPu: { endpoint: 'https://open.bigmodel.cn/api/paas/v4', chatModels: ['glm-4-flash', 'glm-4-plus'], imageModels: ['cogview-3-flash'] },
  Ollama: { endpoint: 'http://127.0.0.1:11434/v1', chatModels: ['qwen2.5:14b', 'llama3.1:8b'], imageModels: ['sdxl:latest'] },
  Custom: { endpoint: '', chatModels: ['qwen-plus'], imageModels: [] },
};

export const SettingsPanel: React.FC = () => {
  const { theme, setTheme } = useThemeStore();
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
  const providers = useModelStore((s) => s.providers);
  const upsertProvider = useModelStore((s) => s.upsertProvider);
  const removeProvider = useModelStore((s) => s.removeProvider);
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
  const [systemConfig, setSystemConfig] = useState<{
    openai_api_key?: string;
    openai_api_base?: string;
    openai_model_name?: string;
    ollama_model?: string;
    llm_vendor?: string;
  }>({});
  const [loadingSystemConfig, setLoadingSystemConfig] = useState(false);
  const [savingSystemConfig, setSavingSystemConfig] = useState(false);
  const apiPresets = [
    { label: 'Local Proxy', value: '/api' },
    { label: 'Local Backend', value: 'http://127.0.0.1:18789/api' },
    { label: 'OpenAI', value: 'https://api.openai.com/v1' },
    { label: 'Anthropic', value: 'https://api.anthropic.com/v1' },
    { label: 'Google', value: 'https://generativelanguage.googleapis.com/v1beta' },
    { label: '阿里百炼', value: 'https://dashscope.aliyuncs.com/compatible-mode/v1' },
    { label: '腾讯混元', value: 'https://api.hunyuan.cloud.tencent.com/v1' },
    { label: '字节豆包', value: 'https://ark.cn-beijing.volces.com/api/v3' },
  ];

  const detectVendor = (base: string): VendorName | null => {
    const lower = base.toLowerCase();
    if (lower.includes('openai.com')) return 'OpenAI';
    if (lower.includes('anthropic.com')) return 'Anthropic';
    if (lower.includes('generativelanguage.googleapis.com') || lower.includes('googleapis.com')) return 'Google';
    if (lower.includes('dashscope.aliyuncs.com') || lower.includes('aliyuncs.com')) return 'Aliyun';
    if (lower.includes('hunyuan.cloud.tencent.com') || lower.includes('hunyuan.tencentcloudapi.com') || lower.includes('tencentcloudapi.com')) return 'Tencent';
    if (lower.includes('volces.com') || lower.includes('volcengineapi.com') || lower.includes('ark.cn-beijing')) return 'ByteDance';
    if (lower.includes('moonshot.cn')) return 'Kimi';
    if (lower.includes('bigmodel.cn')) return 'ZhiPu';
    if (lower.includes('127.0.0.1:11434') || lower.includes('localhost:11434')) return 'Ollama';
    return null;
  };

  const normalizeVendorName = (vendor: string): VendorName => {
    const value = String(vendor || '').trim().toLowerCase();
    if (value === 'openai') return 'OpenAI';
    if (value === 'anthropic') return 'Anthropic';
    if (value === 'google') return 'Google';
    if (value === 'aliyun') return 'Aliyun';
    if (value === 'tencent') return 'Tencent';
    if (value === 'bytedance') return 'ByteDance';
    if (value === 'kimi') return 'Kimi';
    if (value === 'zhipu') return 'ZhiPu';
    if (value === 'ollama') return 'Ollama';
    return 'Custom';
  };

  const modelSuggestionsForVendor = (vendor: string, modelType: 'chat' | 'image' = 'chat') => {
    const v = normalizeVendorName(vendor);
    const defaults = VENDOR_DEFAULTS[v];
    return modelType === 'image' ? defaults.imageModels : defaults.chatModels;
  };

  const vendorToEnvVendor = (vendor: string | null): string => {
    switch (vendor) {
      case 'Aliyun':
        return 'aliyun';
      case 'OpenAI':
        return 'openai';
      case 'Kimi':
        return 'kimi';
      case 'ZhiPu':
        return 'zhipu';
      case 'Ollama':
        return 'ollama';
      case 'Google':
        return 'custom';
      case 'Anthropic':
        return 'custom';
      case 'Tencent':
        return 'custom';
      case 'ByteDance':
        return 'custom';
      default:
        return 'custom';
    }
  };

  const envVendorToRouteVendor = (vendor: string): string => {
    switch (vendor.toLowerCase()) {
      case 'aliyun':
        return 'Aliyun';
      case 'openai':
        return 'OpenAI';
      case 'ollama':
        return 'Ollama';
      case 'kimi':
        return 'Kimi';
      case 'zhipu':
        return 'ZhiPu';
      default:
        return 'Custom';
    }
  };

  const validateVendorModel = (vendor: string, model: string): string => {
    const v = vendor.toLowerCase();
    if (!model.trim()) return 'Model Name 不能为空';
    if (v === 'aliyun' && !/^(qwen|wanx)/i.test(model)) return 'Aliyun 模型建议使用 qwen* 或 wanx*';
    if (v === 'openai' && !/^(gpt|o1|o3)/i.test(model)) return 'OpenAI 模型建议使用 gpt*/o1*/o3*';
    if (v === 'kimi' && !/^(moonshot|kimi)/i.test(model)) return 'Kimi 模型建议使用 moonshot* 或 kimi*';
    if (v === 'zhipu' && !/^(glm|cog)/i.test(model)) return 'ZhiPu 模型建议使用 glm* 或 cog*';
    if (v === 'anthropic' && !/^claude/i.test(model)) return 'Anthropic 模型建议使用 claude*';
    if (v === 'google' && !/^gemini/i.test(model)) return 'Google 模型建议使用 gemini*';
    if (v === 'tencent' && !/^hunyuan/i.test(model)) return 'Tencent 模型建议使用 hunyuan*';
    if (v === 'bytedance' && !/^doubao/i.test(model)) return 'ByteDance 模型建议使用 doubao*';
    if (v === 'ollama' && !/:/.test(model)) return 'Ollama 模型建议使用 model:tag 格式';
    if (/(qwen-image|wanx|doubao-image|sdxl|stable-diffusion|image)/i.test(model)) return '当前配置用于聊天模型，请不要填写图像模型';
    return '';
  };

  const verifySystemChatConfig = async (cfg: { base: string; model: string; key: string; vendor: string }) => {
    if (!cfg.base) throw new Error('API Base URL 不能为空');
    if (!cfg.model) throw new Error('Model Name 不能为空');
    if (!cfg.key && cfg.vendor !== 'ollama') throw new Error('API Key 不能为空（本地 Ollama 可为空）');
    const resp: any = await api.post('/v1/chat', {
      message: 'ping',
      session_id: `cfg-verify-${Date.now()}`,
      route: {
        vendor: envVendorToRouteVendor(cfg.vendor),
        model: cfg.model,
        api_base_url: cfg.base,
        api_key: cfg.key,
        model_type: 'chat',
        reason: 'settings-verify',
      },
    });
    if (!resp || resp.error) {
      throw new Error(resp?.error || '模型连通性验证失败');
    }
  };

  const handlePresetSelect = (value: string) => {
    setApiBaseUrl(value);
    const vendor = detectVendor(value);
    if (vendor) {
      toast(`已切换到 ${vendor} 预设地址，请填写对应 API Key 后测试连接/同步模型`);
    }
  };

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
    settingsService.listApiKeys().then((res: any) => setApiKeys(res || [])).catch(() => {});
    settingsService.getMcpOverview().then((res: any) => setMcpOverview(res || null)).catch(() => {});
    settingsService.getRoutingSettings().then((res: any) => {
      if (res) setRoutingSettings(res);
    }).catch(() => {});
    settingsService.getRoutingReport(200).then((res: any) => {
      if (res) setRoutingReport(res);
    }).catch(() => {});
    setDraftProviders(providers);
    setLoadingSystemConfig(true);
    settingsService.getSystemConfig()
      .then((res: any) => setSystemConfig(res?.data || res || {}))
      .catch((e) => console.error('Failed to load system config', e))
      .finally(() => setLoadingSystemConfig(false));
  }, []);

  useEffect(() => {
    setDraftProviders(providers);
  }, [providers]);

  const handleSaveApiSettings = async () => {
    let normalizedUrl = apiBaseUrl.trim();
    if (!normalizedUrl) {
      toast.error('API URL 不能为空');
      return;
    }
    
    const providerMode = detectVendor(normalizedUrl);

    // Keep relative proxy path as-is
    if (/^api(?:\/.*)?$/i.test(normalizedUrl)) {
        normalizedUrl = `/${normalizedUrl.replace(/^\/+/, '')}`.replace(/\/+$/, '') || '/api';
    } else if (/^\//.test(normalizedUrl)) {
        normalizedUrl = normalizedUrl.replace(/\/+$/, '') || '/api';
    } else {
      // Ensure URL starts with http:// or https://
      if (!/^https?:\/\//i.test(normalizedUrl)) {
          // Default to http if not specified
          normalizedUrl = `http://${normalizedUrl}`;
      }

      // Automatically convert localhost to 127.0.0.1 to avoid IPv6 issues on macOS
      normalizedUrl = normalizedUrl.replace(/localhost/i, '127.0.0.1');
      
      // Remove trailing slash
      normalizedUrl = normalizedUrl.replace(/\/+$/, '');
    }

    setApiBaseUrl(normalizedUrl); // Update state to reflect change
    localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, normalizedUrl);
    const apiKeyValue = apiKey.trim();
    const isLocalGateway = normalizedUrl.startsWith('/api') || normalizedUrl.includes('127.0.0.1:18789') || normalizedUrl.includes('localhost:18789');
    if (apiKeyValue) {
      if (apiKeyValue.startsWith('sk-') && !apiKeyValue.startsWith('sk-crablet-')) {
        toast((t) => (
          <div className="flex flex-col gap-2">
            <span>Warning: It looks like you entered an OpenAI/LLM API Key.</span>
            <span className="text-xs">The "API Key" here is for Crablet Gateway auth. For local gateway auth-off mode, leave it empty.</span>
            <div className="flex gap-2 justify-end">
              <button className="bg-white text-black px-2 py-1 rounded border text-xs" onClick={() => toast.dismiss(t.id)}>Dismiss</button>
            </div>
          </div>
        ), { duration: 6000, icon: '⚠️' });
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
        vendor: providerMode,
        model,
        modelType: /image|画|绘图|图像|文生图|doubao-image|qwen-image/i.test(model) ? 'image' : 'chat',
        version: 'latest',
        apiBaseUrl: normalizedUrl,
        apiKey: apiKeyValue,
        enabled: true,
        priority: 0,
      });
      const sameVendor = providers.filter((p) => p.vendor.toLowerCase() === providerMode.toLowerCase());
      sameVendor.forEach((p) => {
        upsertProvider({
          ...p,
          apiBaseUrl: normalizedUrl,
          apiKey: apiKeyValue,
          enabled: true,
        });
      });
      setDraftProviders((prev) =>
        prev.map((p) =>
          p.vendor.toLowerCase() === providerMode.toLowerCase()
            ? { ...p, apiBaseUrl: normalizedUrl, apiKey: apiKeyValue, enabled: true }
            : p
        )
      );
      try {
        const newConfig = {
          openai_api_key: apiKeyValue,
          openai_api_base: normalizedUrl,
          openai_model_name: model,
          llm_vendor: vendorToEnvVendor(providerMode),
        };
        await settingsService.updateSystemConfig(newConfig);
        const confirmed: any = await settingsService.getSystemConfig();
        const confirmedConfig = confirmed?.data || confirmed || {};
        setSystemConfig((prev) => ({ ...prev, ...confirmedConfig }));
        const expected = {
          openai_api_key: newConfig.openai_api_key || '',
          openai_api_base: newConfig.openai_api_base || '',
          openai_model_name: newConfig.openai_model_name || '',
          llm_vendor: newConfig.llm_vendor || '',
        };
        const actual = {
          openai_api_key: confirmedConfig.openai_api_key || '',
          openai_api_base: confirmedConfig.openai_api_base || '',
          openai_model_name: confirmedConfig.openai_model_name || '',
          llm_vendor: confirmedConfig.llm_vendor || '',
        };
        if (JSON.stringify(expected) !== JSON.stringify(actual)) {
          toast.error('后端配置回读校验不一致，请检查 .env 文件权限或路径');
        }
      } catch {
        toast.error('已保存前端设置，但同步后端 .env 失败');
      }
      toast(`已切换到${providerMode}厂商模式，配置已同步到模型路由与后端.env`);
    }
    toast.success(`API 设置已保存并生效：${normalizedUrl}`);
  };

  const handleCopyEndpoint = async () => {
    if (!currentVendor) return;
    const text = vendorGuide[currentVendor].endpoint;
    const afterCopied = () => {
      if (autoFillEndpointOnCopy) {
        setApiBaseUrl(text);
        localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, text);
        toast.success('已复制并自动填入推荐端点');
      } else {
        toast.success('已复制推荐端点');
      }
    };
    try {
      if (navigator?.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
        afterCopied();
        return;
      }
    } catch {}
    try {
      const el = document.createElement('textarea');
      el.value = text;
      document.body.appendChild(el);
      el.select();
      document.execCommand('copy');
      document.body.removeChild(el);
      afterCopied();
    } catch {
      toast.error('复制失败，请手动复制');
    }
  };

  const handleToggleAutoFillEndpoint = (checked: boolean) => {
    setAutoFillEndpointOnCopy(checked);
    localStorage.setItem('crablet-auto-fill-endpoint-on-copy', checked ? '1' : '0');
  };

  const classifyHttpError = (status: number) => {
    if (status === 400) return 'Key格式错误';
    if (status === 401 || status === 403) return '权限不足';
    if (status === 404) return 'Endpoint错误';
    if (status === 429) return '请求限流';
    if (status >= 500) return '服务端异常';
    return '连接异常';
  };

  const showHttpError = (status: number, prefix: string) => {
    const category = classifyHttpError(status);
    toast.error(`${prefix}：${category}（HTTP ${status}）`);
  };

  const handleTestConnection = async () => {
    setTestingConnection(true);
    try {
      const rawBase = apiBaseUrl.trim();
      const rawKey = apiKey.trim();
      const normalizedBase = /^https?:\/\//i.test(rawBase) || rawBase.startsWith('/') ? rawBase.replace(/\/+$/, '') : `http://${rawBase}`.replace(/\/+$/, '');
      localStorage.setItem(LOCAL_STORAGE_KEYS.API_BASE_URL, normalizedBase || '/api');
      if (rawKey) {
        localStorage.setItem(LOCAL_STORAGE_KEYS.API_KEY, rawKey);
        localStorage.setItem(LOCAL_STORAGE_KEYS.AUTH_TOKEN, rawKey);
      } else {
        localStorage.removeItem(LOCAL_STORAGE_KEYS.API_KEY);
        localStorage.removeItem(LOCAL_STORAGE_KEYS.AUTH_TOKEN);
      }
      const autoVendor = detectVendor(normalizedBase);
      if (!autoVendor) {
        const target = normalizedBase.startsWith('/')
          ? `${window.location.origin}${normalizedBase}/v1/swarm/stats`
          : `${normalizedBase}/v1/swarm/stats`;
        const res = await fetch(target, {
          method: 'GET',
          headers: rawKey ? { Authorization: `Bearer ${rawKey}` } : undefined,
        });
        if (res.status === 200) {
          toast.success('连接测试通过：API可用');
        } else if (res.status === 401) {
          toast.error('连接到网关但鉴权失败(401)。请清空或更换Gateway Token后再试。');
        } else if (res.status === 404) {
          toast.error('连接到服务但路径不匹配(404)。请确认API地址应以 /api 结尾。');
        } else {
          showHttpError(res.status, '连接测试失败');
        }
        return;
      }
      if (!rawKey) {
        toast.error('请先填写该厂商的 API Key');
        return;
      }
      if (keyPatternIssue) {
        toast.error(`连接测试失败：${keyPatternIssue}`);
        return;
      }
      const fetchFirstAvailable = async (endpoints: string[], headers?: Record<string, string>) => {
        for (const endpoint of endpoints) {
          try {
            const res = await fetch(endpoint, { headers });
            if (res.ok) return await res.json();
          } catch {}
        }
        return null;
      };
      const syncModels = async () => {
        const discoveredModels: string[] = [];
        if (autoVendor === 'OpenAI') {
          const endpoint = `${normalizedBase.replace(/\/v1$/i, '')}/v1/models`;
          const res = await fetch(endpoint, { headers: { Authorization: `Bearer ${rawKey}` } });
          if (!res.ok) throw new Error(`OpenAI 模型列表获取失败：HTTP ${res.status}`);
          const json: any = await res.json();
          discoveredModels.push(...((json?.data || []).map((x: any) => x?.id).filter(Boolean)));
        } else if (autoVendor === 'Anthropic') {
          const endpoint = `${normalizedBase.replace(/\/v1$/i, '')}/v1/models`;
          const res = await fetch(endpoint, { headers: { 'x-api-key': rawKey, 'anthropic-version': '2023-06-01' } });
          if (!res.ok) throw new Error(`Anthropic 模型列表获取失败：HTTP ${res.status}`);
          const json: any = await res.json();
          discoveredModels.push(...((json?.data || []).map((x: any) => x?.id).filter(Boolean)));
        } else if (autoVendor === 'Google') {
          const endpoint = `${normalizedBase.replace(/\?key=.*/i, '')}/models?key=${encodeURIComponent(rawKey)}`;
          const res = await fetch(endpoint);
          if (!res.ok) throw new Error(`Google 模型列表获取失败：HTTP ${res.status}`);
          const json: any = await res.json();
          discoveredModels.push(...((json?.models || []).map((x: any) => String(x?.name || '').replace(/^models\//, '')).filter(Boolean)));
        } else if (autoVendor === 'Aliyun') {
          const base = normalizedBase.replace(/\/+$/, '');
          const json: any = await fetchFirstAvailable(
            [`${base.replace(/\/v1$/i, '')}/v1/models`, `${base.replace(/\/compatible-mode\/v1$/i, '')}/compatible-mode/v1/models`],
            { Authorization: `Bearer ${rawKey}` }
          );
          discoveredModels.push(...((json?.data || []).map((x: any) => x?.id).filter(Boolean)));
          if (!discoveredModels.length) {
            discoveredModels.push('qwen-plus', 'qwen-turbo', 'qwen-max', 'qwen2.5-72b-instruct');
          }
        } else if (autoVendor === 'ByteDance') {
          const base = normalizedBase.replace(/\/+$/, '');
          const json: any = await fetchFirstAvailable(
            [`${base.replace(/\/api\/v3$/i, '')}/api/v3/models`, `${base.replace(/\/v1$/i, '')}/v1/models`],
            { Authorization: `Bearer ${rawKey}` }
          );
          discoveredModels.push(...((json?.data || []).map((x: any) => x?.id).filter(Boolean)));
          if (!discoveredModels.length) {
            discoveredModels.push('doubao-pro-32k', 'doubao-pro-4k', 'doubao-lite-32k');
          }
        } else if (autoVendor === 'Tencent') {
          const base = normalizedBase.replace(/\/+$/, '');
          const json: any = await fetchFirstAvailable(
            [`${base.replace(/\/v1$/i, '')}/v1/models`, `${base.replace(/\/hunyuan\/v1$/i, '')}/hunyuan/v1/models`],
            { Authorization: `Bearer ${rawKey}` }
          );
          discoveredModels.push(...((json?.data || []).map((x: any) => x?.id || x?.name).filter(Boolean)));
          if (!discoveredModels.length) {
            discoveredModels.push('hunyuan-pro', 'hunyuan-standard', 'hunyuan-lite');
          }
        }
        const picked = [...new Set(discoveredModels)].slice(0, 20);
        if (picked.length === 0) {
          toast.error('连接成功，但未发现可用模型');
          return 0;
        }
        const now = Date.now();
        const discoveredProviders: ModelProvider[] = picked.map((model, idx) => {
          const existing = draftProviders.find((p) => p.vendor.toLowerCase() === autoVendor.toLowerCase() && p.model.toLowerCase() === model.toLowerCase());
          return existing || {
            id: `${autoVendor.toLowerCase()}-${model.toLowerCase().replace(/[^a-z0-9]+/g, '-')}-${now}-${idx}`,
            vendor: autoVendor,
            model,
            modelType: /image|画|绘图|图像|文生图|doubao-image|qwen-image/i.test(model) ? 'image' : 'chat',
            version: 'latest',
            apiBaseUrl: normalizedBase,
            apiKey: rawKey,
            enabled: true,
            priority: draftProviders.length + idx + 1,
          };
        });
        setDraftProviders((prev) => {
          const merged = [...prev];
          discoveredProviders.forEach((np) => {
            const i = merged.findIndex((p) => p.vendor.toLowerCase() === np.vendor.toLowerCase() && p.model.toLowerCase() === np.model.toLowerCase());
            if (i >= 0) merged[i] = { ...merged[i], ...np, id: merged[i].id };
            else merged.push(np);
          });
          return merged;
        });
        discoveredProviders.forEach((p) => upsertProvider(p));
        return picked.length;
      };
      const count = await syncModels();
      if (count > 0) toast.success(`连接成功，已自动同步 ${autoVendor} 模型 ${count} 个`);
    } catch (err: any) {
      const msg = String(err?.message || '');
      const m = msg.match(/HTTP\s*(\d+)/i);
      if (m) {
        showHttpError(Number(m[1]), '模型同步失败');
      } else {
        toast.error('连接测试失败：跨域限制或网络不可达');
      }
    } finally {
      setTestingConnection(false);
    }
  };

  const handleSyncModels = async () => {
    setSyncingModels(true);
    try {
      const rawBase = apiBaseUrl.trim();
      const rawKey = apiKey.trim();
      const normalizedBase = /^https?:\/\//i.test(rawBase) || rawBase.startsWith('/') ? rawBase.replace(/\/+$/, '') : `http://${rawBase}`.replace(/\/+$/, '');
      const autoVendor = detectVendor(normalizedBase);
      if (!autoVendor) {
        toast.error('当前API地址不是已支持的模型厂商地址，请切换到OpenAI/Anthropic/Google/阿里百炼/腾讯/豆包');
        return;
      }
      if (!rawKey) {
        toast.error('请先填写该厂商的 API Key');
        return;
      }
      await handleTestConnection();
    } finally {
      setSyncingModels(false);
    }
  };

  const handleSaveProfile = () => {
    localStorage.setItem('crablet-profile-name', profileName.trim());
    localStorage.setItem('crablet-profile-email', profileEmail.trim());
    localStorage.setItem('crablet-profile-org', profileOrg.trim());
    toast.success('账户信息已保存');
  };

  const handleCreateKey = async () => {
    const name = newKeyName.trim();
    if (!name) return;
    try {
      const res: any = await settingsService.createApiKey(name);
      toast.success(`新密钥已创建：${res?.key || ''}`);
      setNewKeyName('');
      const refreshed: any = await settingsService.listApiKeys();
      setApiKeys(refreshed || []);
    } catch {
      toast.error('创建密钥失败');
    }
  };

  const handleRevokeKey = async (id: string) => {
    try {
      await settingsService.revokeApiKey(id);
      setApiKeys((prev) => prev.filter((k) => k.id !== id));
      toast.success('密钥已撤销');
    } catch {
      toast.error('撤销密钥失败');
    }
  };

  const handleSaveProviders = () => {
    draftProviders.forEach((p) => upsertProvider(p));
    toast.success('多厂商模型配置已保存');
  };

  const handleSaveRoutingSettings = async () => {
    const s2 = Number(routingSettings.system2_threshold);
    const s3 = Number(routingSettings.system3_threshold);
    const exp = Number(routingSettings.bandit_exploration);
    const deliberate = Number(routingSettings.deliberate_threshold);
    const meta = Number(routingSettings.meta_reasoning_threshold);
    const simulations = Number(routingSettings.mcts_simulations);
    const mctsExploration = Number(routingSettings.mcts_exploration_weight);
    if (Number.isNaN(s2) || s2 < 0 || s2 > 1) {
      toast.error('system2_threshold 必须在 0~1');
      return;
    }
    if (Number.isNaN(s3) || s3 < 0 || s3 > 1) {
      toast.error('system3_threshold 必须在 0~1');
      return;
    }
    if (Number.isNaN(exp) || exp < 0.05 || exp > 2) {
      toast.error('bandit_exploration 必须在 0.05~2');
      return;
    }
    if (Number.isNaN(deliberate) || deliberate < 0 || deliberate > 1) {
      toast.error('deliberate_threshold 必须在 0~1');
      return;
    }
    if (Number.isNaN(meta) || meta < 0 || meta > 1) {
      toast.error('meta_reasoning_threshold 必须在 0~1');
      return;
    }
    if (Number.isNaN(simulations) || simulations < 1 || simulations > 512) {
      toast.error('mcts_simulations 必须在 1~512');
      return;
    }
    if (Number.isNaN(mctsExploration) || mctsExploration < 0.1 || mctsExploration > 3) {
      toast.error('mcts_exploration_weight 必须在 0.1~3');
      return;
    }
    setSavingRoutingSettings(true);
    try {
      const saved: any = await settingsService.updateRoutingSettings({
        enable_adaptive_routing: routingSettings.enable_adaptive_routing,
        system2_threshold: s2,
        system3_threshold: s3,
        bandit_exploration: exp,
        enable_hierarchical_reasoning: routingSettings.enable_hierarchical_reasoning,
        deliberate_threshold: deliberate,
        meta_reasoning_threshold: meta,
        mcts_simulations: Math.round(simulations),
        mcts_exploration_weight: mctsExploration,
        graph_rag_entity_mode: routingSettings.graph_rag_entity_mode,
      });
      if (saved) setRoutingSettings(saved);
      toast.success('元认知路由配置已保存');
    } catch {
      toast.error('保存元认知路由配置失败');
    } finally {
      setSavingRoutingSettings(false);
    }
  };

  const handleRefreshRoutingReport = async () => {
    const window = Math.max(10, Math.min(2000, Number(routingReportWindow) || 200));
    setRoutingReportWindow(window);
    setLoadingRoutingReport(true);
    try {
      const report: any = await settingsService.getRoutingReport(window);
      if (report) setRoutingReport(report);
    } catch {
      toast.error('读取路由评估报告失败');
    } finally {
      setLoadingRoutingReport(false);
    }
  };

  const handleSaveSystemConfig = async () => {
    const base = String(systemConfig.openai_api_base || '').trim();
    const model = String(systemConfig.openai_model_name || '').trim();
    const key = String(systemConfig.openai_api_key || '').trim();
    const inferredVendor = vendorToEnvVendor(detectVendor(base));
    const vendor = String(systemConfig.llm_vendor || inferredVendor || 'custom').trim().toLowerCase();
    const modelIssue = validateVendorModel(vendor, model);
    if (modelIssue) {
      toast.error(modelIssue);
      return;
    }
    setSavingSystemConfig(true);
    try {
      await verifySystemChatConfig({ base, model, key, vendor });
      const payload = { ...systemConfig, openai_api_base: base, openai_model_name: model, openai_api_key: key, llm_vendor: vendor };
      await settingsService.updateSystemConfig(payload);
      const confirmed: any = await settingsService.getSystemConfig();
      const confirmedConfig = confirmed?.data || confirmed || {};
      setSystemConfig((prev) => ({ ...prev, ...confirmedConfig }));
      const expected = {
        openai_api_key: key,
        openai_api_base: base,
        openai_model_name: model,
        llm_vendor: vendor,
      };
      const actual = {
        openai_api_key: confirmedConfig.openai_api_key || '',
        openai_api_base: confirmedConfig.openai_api_base || '',
        openai_model_name: confirmedConfig.openai_model_name || '',
        llm_vendor: confirmedConfig.llm_vendor || '',
      };
      if (JSON.stringify(expected) !== JSON.stringify(actual)) {
        toast.error('配置保存后回读不一致，请检查后端 .env 写入路径');
        return;
      }
      toast.success('System config verified and synced to local .env.');
    } catch {
      toast.error('保存失败：模型配置校验未通过或后端写入失败');
    } finally {
      setSavingSystemConfig(false);
    }
  };

  const handleSetAsSystemDefault = async (p: ModelProvider) => {
    if (!window.confirm(`确定要将 [${p.vendor}] ${p.model} 设为后端默认配置吗？\n这将覆盖 .env 文件中的 KEY/BASE/MODEL 配置。`)) return;
    if ((p.modelType || 'chat') !== 'chat') {
      toast.error('系统默认模型必须是 chat 类型，不能使用 image 模型');
      return;
    }
    const vendor = vendorToEnvVendor(p.vendor);
    const modelIssue = validateVendorModel(vendor, p.model);
    if (modelIssue) {
      toast.error(modelIssue);
      return;
    }
    
    const newConfig = {
      openai_api_key: p.apiKey,
      openai_api_base: p.apiBaseUrl,
      openai_model_name: p.model,
      llm_vendor: vendor,
    };
    
    setSavingSystemConfig(true);
    try {
      await verifySystemChatConfig({
        base: String(p.apiBaseUrl || '').trim(),
        model: String(p.model || '').trim(),
        key: String(p.apiKey || '').trim(),
        vendor: vendor.toLowerCase(),
      });
      await settingsService.updateSystemConfig(newConfig);
      const confirmed: any = await settingsService.getSystemConfig();
      const confirmedConfig = confirmed?.data || confirmed || {};
      setSystemConfig(prev => ({ ...prev, ...confirmedConfig }));
      toast.success('已更新系统默认配置并完成回读校验');
    } catch {
      toast.error('保存失败：模型校验或后端写入未通过');
    } finally {
      setSavingSystemConfig(false);
    }
  };

  const handleAddProvider = () => {
    const ts = Date.now();
    setDraftProviders((prev) => [
      ...prev,
      {
        id: `custom-${ts}`,
        vendor: 'Custom',
        model: 'model-name',
        modelType: 'chat',
        version: 'v1',
        apiBaseUrl: '',
        apiKey: '',
        enabled: true,
        priority: prev.length + 1,
      },
    ]);
  };

  const applySystemVendorPreset = (vendor: VendorName) => {
    const defaults = VENDOR_DEFAULTS[vendor];
    const suggestedModel = defaults.chatModels[0] || '';
    setSystemConfig((prev) => ({
      ...prev,
      llm_vendor: vendorToEnvVendor(vendor),
      openai_api_base: defaults.endpoint || prev.openai_api_base || '',
      openai_model_name: suggestedModel || prev.openai_model_name || '',
    }));
    toast(`已应用 ${vendor} 推荐端点与模型`);
  };

  const updateProviderVendor = (id: string, vendor: VendorName) => {
    setDraftProviders((prev) =>
      prev.map((p) => {
        if (p.id !== id) return p;
        const defaults = VENDOR_DEFAULTS[vendor];
        const modelType = p.modelType || 'chat';
        const suggestions = modelSuggestionsForVendor(vendor, modelType);
        return {
          ...p,
          vendor,
          apiBaseUrl: defaults.endpoint || p.apiBaseUrl,
          model: suggestions[0] || p.model,
        };
      })
    );
  };

  const updateProviderModelType = (id: string, modelType: 'chat' | 'image') => {
    setDraftProviders((prev) =>
      prev.map((p) => {
        if (p.id !== id) return p;
        const suggestions = modelSuggestionsForVendor(p.vendor, modelType);
        const model = suggestions.includes(p.model) ? p.model : (suggestions[0] || p.model);
        return { ...p, modelType, model };
      })
    );
  };

  const currentVendor = detectVendor(apiBaseUrl.trim());
  const vendorGuide: Record<VendorName, { endpoint: string; keyHint: string }> = {
    OpenAI: { endpoint: VENDOR_DEFAULTS.OpenAI.endpoint, keyHint: 'sk-***' },
    Anthropic: { endpoint: VENDOR_DEFAULTS.Anthropic.endpoint, keyHint: 'sk-ant-***' },
    Google: { endpoint: VENDOR_DEFAULTS.Google.endpoint, keyHint: 'AIza***' },
    Aliyun: { endpoint: VENDOR_DEFAULTS.Aliyun.endpoint, keyHint: 'sk-***' },
    Tencent: { endpoint: VENDOR_DEFAULTS.Tencent.endpoint, keyHint: '按腾讯云控制台分配的Key' },
    ByteDance: { endpoint: VENDOR_DEFAULTS.ByteDance.endpoint, keyHint: '按火山引擎控制台分配的Key' },
    Kimi: { endpoint: VENDOR_DEFAULTS.Kimi.endpoint, keyHint: 'sk-***' },
    ZhiPu: { endpoint: VENDOR_DEFAULTS.ZhiPu.endpoint, keyHint: '按智谱控制台分配的Key' },
    Ollama: { endpoint: VENDOR_DEFAULTS.Ollama.endpoint, keyHint: '本地模型通常无需Key' },
    Custom: { endpoint: '', keyHint: '按服务商文档填写' },
  };
  const keyPlaceholderByVendor: Record<VendorName, string> = {
    OpenAI: '例如 sk-***',
    Anthropic: '例如 sk-ant-***',
    Google: '例如 AIza***',
    Aliyun: '例如 sk-***',
    Tencent: '输入腾讯云分配的 Key',
    ByteDance: '输入火山引擎分配的 Key',
    Kimi: '例如 sk-***',
    ZhiPu: '输入智谱分配的 Key',
    Ollama: '本地可留空',
    Custom: '输入服务商分配的 Key',
  };
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
  const troubleshootingGuide: Record<VendorName, { keyFormat: string; endpoint: string; permission: string; cors: string }> = {
    OpenAI: {
      keyFormat: '确认Key以 sk- 开头，且未包含多余空格',
      endpoint: '端点建议使用 https://api.openai.com/v1',
      permission: '确认账号与项目对目标模型有可用权限',
      cors: '若在浏览器受限，优先通过本地网关 /api 转发',
    },
    Anthropic: {
      keyFormat: '确认Key以 sk-ant- 开头',
      endpoint: '端点建议使用 https://api.anthropic.com/v1',
      permission: '确认已开通对应模型调用权限',
      cors: '前端直连失败时建议改为网关转发',
    },
    Google: {
      keyFormat: '确认Key以 AIza 开头',
      endpoint: '端点建议使用 https://generativelanguage.googleapis.com/v1beta',
      permission: '确认API Key已启用 Generative Language API',
      cors: '浏览器跨域受限时建议使用后端代理',
    },
    Aliyun: {
      keyFormat: '确认百炼Key有效，通常以 sk- 开头',
      endpoint: '建议使用 https://dashscope.aliyuncs.com/compatible-mode/v1',
      permission: '确认阿里云账号已开通百炼并有模型访问权限',
      cors: '浏览器跨域失败时建议网关转发',
    },
    Tencent: {
      keyFormat: '确认腾讯云控制台分配的Key/签名配置正确',
      endpoint: '建议使用 https://api.hunyuan.cloud.tencent.com/v1',
      permission: '确认混元服务已开通并有模型权限',
      cors: '若前端受限，建议通过后端统一签名与转发',
    },
    ByteDance: {
      keyFormat: '确认火山引擎分配的Ark Key有效',
      endpoint: '建议使用 https://ark.cn-beijing.volces.com/api/v3',
      permission: '确认对应模型已开通并在可用区域',
      cors: '浏览器跨域失败时优先使用后端代理',
    },
    Kimi: {
      keyFormat: '确认Moonshot Key有效',
      endpoint: '建议使用 https://api.moonshot.cn/v1',
      permission: '确认账号已开通目标模型调用权限',
      cors: '浏览器跨域失败时优先使用后端代理',
    },
    ZhiPu: {
      keyFormat: '确认智谱Key有效',
      endpoint: '建议使用 https://open.bigmodel.cn/api/paas/v4',
      permission: '确认目标模型已开通',
      cors: '浏览器跨域失败时优先使用后端代理',
    },
    Ollama: {
      keyFormat: '本地Ollama通常无需Key',
      endpoint: '建议使用 http://127.0.0.1:11434/v1',
      permission: '确认本地模型已 pull 并运行',
      cors: '本地访问通常无跨域问题',
    },
    Custom: {
      keyFormat: '按服务商文档检查Key格式',
      endpoint: '确认 endpoint 与协议版本匹配',
      permission: '确认目标模型权限已开通',
      cors: '跨域失败时优先使用后端代理',
    },
  };

  return (
    <div className="h-full px-4 py-6 pb-10 sm:px-6 md:px-8 overflow-y-auto bg-gray-50 dark:bg-gray-900">
      <div className="max-w-4xl mx-auto space-y-6">
        <div className="flex justify-between items-center">
          <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100 flex items-center gap-2">
              <Settings className="w-6 h-6" />
              Settings
          </h1>
        </div>

        <Card className="overflow-hidden bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-700 text-slate-900 dark:text-slate-100 shadow-lg">
            <CardHeader className="border-b border-slate-200 dark:border-slate-700 bg-slate-100/60 dark:bg-slate-800/60">
                <CardTitle className="text-slate-900 dark:text-slate-100 flex items-center gap-2">
                    <User className="w-5 h-5" />
                    账户与MCP
                </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6 pt-6">
                <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                    <Input value={profileName} onChange={(e) => setProfileName(e.target.value)} placeholder="姓名" />
                    <Input value={profileEmail} onChange={(e) => setProfileEmail(e.target.value)} placeholder="邮箱" />
                    <Input value={profileOrg} onChange={(e) => setProfileOrg(e.target.value)} placeholder="组织" />
                </div>
                <div className="flex justify-end">
                    <Button onClick={handleSaveProfile} variant="secondary">保存账户信息</Button>
                </div>
                <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                    <div className="rounded border border-slate-200 dark:border-slate-700 p-3">
                        <div className="text-xs text-slate-500">MCP Tools</div>
                        <div className="text-xl font-semibold">{mcpOverview?.mcp_tools ?? '-'}</div>
                    </div>
                    <div className="rounded border border-slate-200 dark:border-slate-700 p-3">
                        <div className="text-xs text-slate-500">MCP Resources</div>
                        <div className="text-xl font-semibold">{mcpOverview?.resources ?? '-'}</div>
                    </div>
                    <div className="rounded border border-slate-200 dark:border-slate-700 p-3">
                        <div className="text-xs text-slate-500">MCP Prompts</div>
                        <div className="text-xl font-semibold">{mcpOverview?.prompts ?? '-'}</div>
                    </div>
                </div>
                <div>
                    <div className="text-sm font-medium mb-2 flex items-center gap-2">
                        <Server className="w-4 h-4" />
                        API Keys 管理
                    </div>
                    <div className="flex gap-2 mb-3">
                        <Input value={newKeyName} onChange={(e) => setNewKeyName(e.target.value)} placeholder="新密钥名称" />
                        <Button onClick={handleCreateKey}>创建</Button>
                    </div>
                    <div className="space-y-2">
                        {apiKeys.map((key) => (
                            <div key={key.id} className="flex items-center justify-between rounded border border-slate-200 dark:border-slate-700 p-2">
                                <div className="min-w-0">
                                    <div className="text-sm font-medium truncate">{key.name}</div>
                                    <div className="text-xs text-slate-500 truncate">{key.id}</div>
                                </div>
                                <Button variant="ghost" size="icon" onClick={() => handleRevokeKey(key.id)}>
                                    <Trash2 className="w-4 h-4 text-red-500" />
                                </Button>
                            </div>
                        ))}
                    </div>
                </div>
            </CardContent>
        </Card>

        <Card className="overflow-hidden bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-700 text-slate-900 dark:text-slate-100 shadow-lg">
            <CardHeader className="border-b border-slate-200 dark:border-slate-700 bg-slate-100/60 dark:bg-slate-800/60">
                <CardTitle className="text-slate-900 dark:text-slate-100 flex items-center gap-2">
                    <Server className="w-5 h-5" />
                    元认知路由（Bandit）
                </CardTitle>
            </CardHeader>
            <CardContent className="pt-6 space-y-4">
                <label className="inline-flex items-center gap-2 text-sm">
                    <input
                        type="checkbox"
                        checked={routingSettings.enable_adaptive_routing}
                        onChange={(e) => setRoutingSettings((prev) => ({ ...prev, enable_adaptive_routing: e.target.checked }))}
                    />
                    <span>启用自适应路由（Contextual Bandit）</span>
                </label>
                <label className="inline-flex items-center gap-2 text-sm">
                    <input
                        type="checkbox"
                        checked={routingSettings.enable_hierarchical_reasoning}
                        onChange={(e) => setRoutingSettings((prev) => ({ ...prev, enable_hierarchical_reasoning: e.target.checked }))}
                    />
                    <span>启用分层推理控制（直觉→分析→元认知）</span>
                </label>
                <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                    <div className="space-y-1">
                        <div className="text-xs text-slate-500">system2_threshold (0~1)</div>
                        <Input value={String(routingSettings.system2_threshold)} onChange={(e) => setRoutingSettings((prev) => ({ ...prev, system2_threshold: Number(e.target.value || 0) }))} />
                    </div>
                    <div className="space-y-1">
                        <div className="text-xs text-slate-500">system3_threshold (0~1)</div>
                        <Input value={String(routingSettings.system3_threshold)} onChange={(e) => setRoutingSettings((prev) => ({ ...prev, system3_threshold: Number(e.target.value || 0) }))} />
                    </div>
                    <div className="space-y-1">
                        <div className="text-xs text-slate-500">bandit_exploration (0.05~2)</div>
                        <Input value={String(routingSettings.bandit_exploration)} onChange={(e) => setRoutingSettings((prev) => ({ ...prev, bandit_exploration: Number(e.target.value || 0) }))} />
                    </div>
                </div>
                <div className="grid grid-cols-1 md:grid-cols-4 gap-3">
                    <div className="space-y-1">
                        <div className="text-xs text-slate-500">deliberate_threshold (0~1)</div>
                        <Input value={String(routingSettings.deliberate_threshold)} onChange={(e) => setRoutingSettings((prev) => ({ ...prev, deliberate_threshold: Number(e.target.value || 0) }))} />
                    </div>
                    <div className="space-y-1">
                        <div className="text-xs text-slate-500">meta_reasoning_threshold (0~1)</div>
                        <Input value={String(routingSettings.meta_reasoning_threshold)} onChange={(e) => setRoutingSettings((prev) => ({ ...prev, meta_reasoning_threshold: Number(e.target.value || 0) }))} />
                    </div>
                    <div className="space-y-1">
                        <div className="text-xs text-slate-500">mcts_simulations (1~512)</div>
                        <Input value={String(routingSettings.mcts_simulations)} onChange={(e) => setRoutingSettings((prev) => ({ ...prev, mcts_simulations: Number(e.target.value || 0) }))} />
                    </div>
                    <div className="space-y-1">
                        <div className="text-xs text-slate-500">mcts_exploration_weight (0.1~3)</div>
                        <Input value={String(routingSettings.mcts_exploration_weight)} onChange={(e) => setRoutingSettings((prev) => ({ ...prev, mcts_exploration_weight: Number(e.target.value || 0) }))} />
                    </div>
                </div>
                <div className="space-y-1">
                    <div className="text-xs text-slate-500">graph_rag_entity_mode</div>
                    <select
                        value={routingSettings.graph_rag_entity_mode}
                        onChange={(e) =>
                            setRoutingSettings((prev) => ({
                                ...prev,
                                graph_rag_entity_mode: e.target.value as 'rule' | 'phrase' | 'hybrid',
                            }))
                        }
                        className="h-10 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 px-3 text-sm"
                    >
                        <option value="rule">rule</option>
                        <option value="phrase">phrase</option>
                        <option value="hybrid">hybrid</option>
                    </select>
                    <div className="text-[11px] text-slate-500">Rule偏稳定，Phrase偏召回，Hybrid综合平衡。</div>
                </div>
                <div className="text-xs text-slate-500">
                    开启后将基于历史质量与延迟反馈学习最优System1/2/3路由，阈值用作基础边界控制。
                </div>
                <div className="rounded border border-slate-200 dark:border-slate-700 p-3 space-y-3">
                    <div className="flex items-center justify-between gap-2">
                        <div className="text-sm font-medium">离线评估报告</div>
                        <div className="flex items-center gap-2">
                            <Input
                                value={String(routingReportWindow)}
                                onChange={(e) => setRoutingReportWindow(Number(e.target.value || 200))}
                                className="w-24"
                            />
                            <Button variant="secondary" loading={loadingRoutingReport} onClick={handleRefreshRoutingReport}>
                                刷新
                            </Button>
                        </div>
                    </div>
                    <div className="grid grid-cols-2 md:grid-cols-4 gap-2 text-xs">
                        <div className="rounded border border-slate-200 dark:border-slate-700 p-2">
                            <div className="text-slate-500">反馈样本</div>
                            <div className="font-semibold">{routingReport?.total_feedback ?? 0}</div>
                        </div>
                        <div className="rounded border border-slate-200 dark:border-slate-700 p-2">
                            <div className="text-slate-500">平均Reward</div>
                            <div className="font-semibold">{(routingReport?.avg_reward ?? 0).toFixed(3)}</div>
                        </div>
                        <div className="rounded border border-slate-200 dark:border-slate-700 p-2">
                            <div className="text-slate-500">平均质量</div>
                            <div className="font-semibold">{(routingReport?.avg_quality_score ?? 0).toFixed(3)}</div>
                        </div>
                        <div className="rounded border border-slate-200 dark:border-slate-700 p-2">
                            <div className="text-slate-500">平均延迟(ms)</div>
                            <div className="font-semibold">{(routingReport?.avg_latency_ms ?? 0).toFixed(1)}</div>
                        </div>
                    </div>
                    <div className="grid grid-cols-1 md:grid-cols-3 gap-2 text-xs">
                        {(routingReport?.by_choice ?? []).map((c) => (
                            <div key={c.choice} className="rounded border border-slate-200 dark:border-slate-700 p-2">
                                <div className="font-medium">{c.choice}</div>
                                <div className="text-slate-500">count: {c.count}</div>
                                <div className="text-slate-500">avg_reward: {c.avg_reward.toFixed(3)}</div>
                                <div className="text-slate-500">avg_latency: {c.avg_latency_ms.toFixed(1)} ms</div>
                            </div>
                        ))}
                    </div>
                    <div className="grid grid-cols-2 md:grid-cols-4 gap-2 text-xs">
                        <div className="rounded border border-slate-200 dark:border-slate-700 p-2">
                            <div className="text-slate-500">分层请求数</div>
                            <div className="font-semibold">{routingReport?.hierarchical_stats?.total_requests ?? 0}</div>
                        </div>
                        <div className="rounded border border-slate-200 dark:border-slate-700 p-2">
                            <div className="text-slate-500">分析层触发</div>
                            <div className="font-semibold">{routingReport?.hierarchical_stats?.deliberate_activations ?? 0}</div>
                        </div>
                        <div className="rounded border border-slate-200 dark:border-slate-700 p-2">
                            <div className="text-slate-500">元认知触发</div>
                            <div className="font-semibold">{routingReport?.hierarchical_stats?.meta_activations ?? 0}</div>
                        </div>
                        <div className="rounded border border-slate-200 dark:border-slate-700 p-2">
                            <div className="text-slate-500">策略切换次数</div>
                            <div className="font-semibold">{routingReport?.hierarchical_stats?.strategy_switches ?? 0}</div>
                        </div>
                    </div>
                </div>
                <div className="flex justify-end">
                    <Button onClick={handleSaveRoutingSettings} loading={savingRoutingSettings}>保存路由配置</Button>
                </div>
            </CardContent>
        </Card>

        <Card className="overflow-hidden bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-700 text-slate-900 dark:text-slate-100 shadow-lg">
            <CardHeader className="border-b border-slate-200 dark:border-slate-700 bg-slate-100/60 dark:bg-slate-800/60">
                <CardTitle className="text-slate-900 dark:text-slate-100 flex items-center gap-2">
                    <Server className="w-5 h-5" />
                    Backend LLM Configuration (.env)
                </CardTitle>
            </CardHeader>
            <CardContent className="pt-6 space-y-4">
                {loadingSystemConfig && <div className="text-sm text-slate-500 animate-pulse">Loading backend config...</div>}
                <div className="text-xs text-slate-500 mb-4">
                    这些配置直接对应后端的 .env 文件。修改后需要重启后端服务才能生效。
                </div>
                <div className="grid gap-2">
                    <label className="text-sm font-medium">LLM Vendor</label>
                    <div className="flex gap-2">
                      <select
                          value={normalizeVendorName(systemConfig.llm_vendor ? envVendorToRouteVendor(systemConfig.llm_vendor) : (detectVendor(systemConfig.openai_api_base || '') || 'Custom'))}
                          onChange={(e) => applySystemVendorPreset(e.target.value as VendorName)}
                          className="h-10 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 px-3 text-sm"
                      >
                          {VENDOR_OPTIONS.map((v) => (
                            <option key={v} value={v}>{v}</option>
                          ))}
                      </select>
                      <Button
                        variant="secondary"
                        onClick={() => applySystemVendorPreset(normalizeVendorName(systemConfig.llm_vendor ? envVendorToRouteVendor(systemConfig.llm_vendor) : (detectVendor(systemConfig.openai_api_base || '') || 'Custom')))}
                      >
                        应用推荐
                      </Button>
                    </div>
                </div>
                <div className="grid gap-2">
                    <label className="text-sm font-medium">OpenAI / DashScope API Key</label>
                    <Input 
                        type="password"
                        value={systemConfig.openai_api_key || ''} 
                        onChange={(e) => setSystemConfig({...systemConfig, openai_api_key: e.target.value})}
                        placeholder="sk-..." 
                    />
                </div>
                <div className="grid gap-2">
                    <label className="text-sm font-medium">API Base URL</label>
                    <Input 
                        value={systemConfig.openai_api_base || ''} 
                        onChange={(e) => setSystemConfig({...systemConfig, openai_api_base: e.target.value})}
                        placeholder="https://dashscope.aliyuncs.com/compatible-mode/v1" 
                    />
                </div>
                <div className="grid grid-cols-2 gap-4">
                    <div className="grid gap-2">
                        <label className="text-sm font-medium">Model Name (Cloud)</label>
                        <Input 
                            list="system-model-recommendations"
                            value={systemConfig.openai_model_name || ''} 
                            onChange={(e) => setSystemConfig({...systemConfig, openai_model_name: e.target.value})}
                            placeholder="qwen-plus" 
                        />
                        <datalist id="system-model-recommendations">
                          {modelSuggestionsForVendor(
                            systemConfig.llm_vendor ? envVendorToRouteVendor(systemConfig.llm_vendor) : (detectVendor(systemConfig.openai_api_base || '') || 'Custom'),
                            'chat'
                          ).map((m) => (
                            <option key={m} value={m} />
                          ))}
                        </datalist>
                        <div className="text-[11px] text-slate-500">
                          推荐模型：{modelSuggestionsForVendor(systemConfig.llm_vendor ? envVendorToRouteVendor(systemConfig.llm_vendor) : (detectVendor(systemConfig.openai_api_base || '') || 'Custom'), 'chat').join(' / ') || '无'}
                        </div>
                    </div>
                    <div className="grid gap-2">
                        <label className="text-sm font-medium">Ollama Model (Local)</label>
                        <Input 
                            value={systemConfig.ollama_model || ''} 
                            onChange={(e) => setSystemConfig({...systemConfig, ollama_model: e.target.value})}
                            placeholder="qwen3:4b" 
                        />
                    </div>
                </div>
                <div className="flex justify-end pt-2">
                    <Button onClick={handleSaveSystemConfig} loading={savingSystemConfig}>
                        Save to .env
                    </Button>
                </div>
            </CardContent>
        </Card>

        <Card className="overflow-hidden bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-700 text-slate-900 dark:text-slate-100 shadow-lg">
            <CardHeader className="border-b border-slate-200 dark:border-slate-700 bg-slate-100/60 dark:bg-slate-800/60">
                <CardTitle className="text-slate-900 dark:text-slate-100 flex items-center gap-2">
                    <KeyRound className="w-5 h-5" />
                    API Configuration
                </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6 pt-6">
                <div className="space-y-4">
                    <div className="grid gap-2">
                        <label className="text-sm font-medium text-slate-700 dark:text-slate-200 flex items-center gap-2">
                            <Link className="w-4 h-4 text-slate-500 dark:text-slate-400" />
                            API Base URL
                        </label>
                        <Input
                            value={apiBaseUrl}
                            onChange={(e) => setApiBaseUrl(e.target.value)}
                            placeholder="e.g., http://localhost:18789/api"
                            className="bg-white dark:bg-slate-800 border-slate-300 dark:border-slate-600 text-slate-900 dark:text-slate-100 placeholder:text-slate-400 dark:placeholder:text-slate-500 focus:ring-blue-500 focus:border-blue-500"
                        />
                        <div className="flex flex-wrap gap-2">
                            {apiPresets.map((preset) => (
                                <Button
                                    key={preset.value}
                                    type="button"
                                    size="sm"
                                    variant={apiBaseUrl.trim() === preset.value ? 'primary' : 'secondary'}
                                    onClick={() => handlePresetSelect(preset.value)}
                                >
                                    {preset.label}
                                </Button>
                            ))}
                        </div>
                        <p className="text-xs text-slate-500 dark:text-slate-400">
                            The base URL for the backend API. Default is <code className="bg-slate-100 dark:bg-slate-800 px-1 py-0.5 rounded">/api</code>.
                        </p>
                    </div>

                    <div className="grid gap-2">
                        <label className="text-sm font-medium text-slate-700 dark:text-slate-200 flex items-center gap-2">
                            <KeyRound className="w-4 h-4 text-slate-500 dark:text-slate-400" />
                            API Key / Bearer Token
                        </label>
                        <Input
                            type="password"
                            value={apiKey}
                            onChange={(e) => setApiKey(e.target.value)}
                            placeholder={currentVendor ? keyPlaceholderByVendor[currentVendor] : "Enter your API Key or Bearer Token"}
                            className="bg-white dark:bg-slate-800 border-slate-300 dark:border-slate-600 text-slate-900 dark:text-slate-100 placeholder:text-slate-400 dark:placeholder:text-slate-500 focus:ring-blue-500 focus:border-blue-500"
                        />
                        <p className="text-xs text-slate-500 dark:text-slate-400">
                            Your authentication token. Leave empty if authentication is disabled.
                        </p>
                        {currentVendor && (
                          <div className="rounded border border-amber-300/70 bg-amber-50 dark:bg-amber-900/20 dark:border-amber-700 px-3 py-2 text-xs text-amber-800 dark:text-amber-200">
                            <div>厂商模式：{currentVendor}</div>
                            <div className="flex items-center justify-between gap-2">
                              <span className="truncate">建议端点：{vendorGuide[currentVendor].endpoint}</span>
                              <Button size="sm" variant="secondary" onClick={handleCopyEndpoint}>复制端点</Button>
                            </div>
                            <label className="inline-flex items-center gap-2 mt-1">
                              <input
                                type="checkbox"
                                checked={autoFillEndpointOnCopy}
                                onChange={(e) => handleToggleAutoFillEndpoint(e.target.checked)}
                              />
                              <span>复制后自动填入API Base URL</span>
                            </label>
                            <div>Key示例：{vendorGuide[currentVendor].keyHint}</div>
                          </div>
                        )}
                        {!!keyPatternIssue && (
                          <div className="rounded border border-rose-300/70 bg-rose-50 dark:bg-rose-900/20 dark:border-rose-700 px-3 py-2 text-xs text-rose-700 dark:text-rose-200">
                            {keyPatternIssue}
                          </div>
                        )}
                        {currentVendor && (
                          <div className="rounded border border-sky-300/70 bg-sky-50 dark:bg-sky-900/20 dark:border-sky-700 px-3 py-2 text-xs text-sky-800 dark:text-sky-200 space-y-1">
                            <div className="font-medium">常见排障建议</div>
                            <div>Key格式错误：{troubleshootingGuide[currentVendor].keyFormat}</div>
                            <div>Endpoint错误：{troubleshootingGuide[currentVendor].endpoint}</div>
                            <div>权限不足：{troubleshootingGuide[currentVendor].permission}</div>
                            <div>跨域限制：{troubleshootingGuide[currentVendor].cors}</div>
                          </div>
                        )}
                    </div>

                    <div className="flex justify-end gap-2 pt-2">
                        <Button variant="secondary" onClick={handleSyncModels} loading={syncingModels}>
                            同步模型
                        </Button>
                        <Button variant="secondary" onClick={handleTestConnection} loading={testingConnection}>
                            测试连接
                        </Button>
                        <Button onClick={handleSaveApiSettings} className="bg-blue-600 hover:bg-blue-700 text-white">
                            Save & Apply
                        </Button>
                    </div>
                </div>
            </CardContent>
        </Card>

        <Card className="overflow-hidden bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-700 text-slate-900 dark:text-slate-100 shadow-lg">
            <CardHeader className="border-b border-slate-200 dark:border-slate-700 bg-slate-100/60 dark:bg-slate-800/60">
                <CardTitle className="text-slate-900 dark:text-slate-100 flex items-center gap-2">
                    <Monitor className="w-5 h-5" />
                    Appearance
                </CardTitle>
            </CardHeader>
            <CardContent className="pt-6">
                <div className="flex items-center justify-between">
                    <span className="text-sm font-medium text-slate-700 dark:text-slate-200">Theme Preference</span>
                    <div className="flex bg-slate-100 dark:bg-slate-800 rounded-lg p-1 border border-slate-200 dark:border-slate-700">
                        <button
                            onClick={() => setTheme('light')}
                            className={clsx(
                                "px-3 py-1.5 rounded-md transition-all flex items-center gap-2 text-sm font-medium",
                                theme === 'light' 
                                    ? "bg-slate-600 text-white shadow-sm" 
                                    : "text-slate-600 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200 hover:bg-slate-200 dark:hover:bg-slate-700/50"
                            )}
                        >
                            <Sun className="w-4 h-4" />
                            Light
                        </button>
                        <button
                            onClick={() => setTheme('system')}
                            className={clsx(
                                "px-3 py-1.5 rounded-md transition-all flex items-center gap-2 text-sm font-medium",
                                theme === 'system' 
                                    ? "bg-slate-600 text-white shadow-sm" 
                                    : "text-slate-600 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200 hover:bg-slate-200 dark:hover:bg-slate-700/50"
                            )}
                        >
                            <Monitor className="w-4 h-4" />
                            System
                        </button>
                        <button
                            onClick={() => setTheme('dark')}
                            className={clsx(
                                "px-3 py-1.5 rounded-md transition-all flex items-center gap-2 text-sm font-medium",
                                theme === 'dark' 
                                    ? "bg-slate-600 text-white shadow-sm" 
                                    : "text-slate-600 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200 hover:bg-slate-200 dark:hover:bg-slate-700/50"
                            )}
                        >
                            <Moon className="w-4 h-4" />
                            Dark
                        </button>
                    </div>
                </div>
            </CardContent>
        </Card>

        <Card className="overflow-hidden bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-700 text-slate-900 dark:text-slate-100 shadow-lg">
            <CardHeader className="border-b border-slate-200 dark:border-slate-700 bg-slate-100/60 dark:bg-slate-800/60">
                <CardTitle className="text-slate-900 dark:text-slate-100 flex items-center gap-2">
                    <Server className="w-5 h-5" />
                    多厂商模型管理与智能路由
                </CardTitle>
            </CardHeader>
            <CardContent className="pt-6 space-y-4">
                <div className="text-xs text-slate-500">
                    支持按会话路由、问题类型、优先级自动选择模型。每个会话可在聊天页手动覆盖。
                </div>
                <div className="space-y-3">
                    {draftProviders.map((p, idx) => (
                        <div key={p.id} className="rounded border border-slate-200 dark:border-slate-700 p-3 grid grid-cols-1 md:grid-cols-8 gap-2">
                            <select
                                value={normalizeVendorName(p.vendor)}
                                onChange={(e) => updateProviderVendor(p.id, e.target.value as VendorName)}
                                className="bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-600 rounded px-2 py-2 text-sm"
                            >
                                {VENDOR_OPTIONS.map((v) => (
                                  <option key={v} value={v}>{v}</option>
                                ))}
                            </select>
                            <Input
                                list={`provider-model-${idx}`}
                                value={p.model}
                                onChange={(e) => setDraftProviders((prev) => prev.map((x) => x.id === p.id ? { ...x, model: e.target.value } : x))}
                                placeholder="模型名"
                            />
                            <datalist id={`provider-model-${idx}`}>
                                {modelSuggestionsForVendor(p.vendor, p.modelType || 'chat').map((m) => (
                                  <option key={m} value={m} />
                                ))}
                            </datalist>
                            <select
                                value={p.modelType || 'chat'}
                                onChange={(e) => updateProviderModelType(p.id, e.target.value as 'chat' | 'image')}
                                className="bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-600 rounded px-2 py-2 text-sm"
                            >
                                <option value="chat">chat</option>
                                <option value="image">image</option>
                            </select>
                            <Input value={p.version} onChange={(e) => setDraftProviders((prev) => prev.map((x) => x.id === p.id ? { ...x, version: e.target.value } : x))} placeholder="版本" />
                            <Input value={p.apiBaseUrl} onChange={(e) => setDraftProviders((prev) => prev.map((x) => x.id === p.id ? { ...x, apiBaseUrl: e.target.value } : x))} placeholder="接口地址" />
                            <Input type="password" value={p.apiKey} onChange={(e) => setDraftProviders((prev) => prev.map((x) => x.id === p.id ? { ...x, apiKey: e.target.value } : x))} placeholder="API Key" />
                            <Input value={String(p.priority)} onChange={(e) => setDraftProviders((prev) => prev.map((x) => x.id === p.id ? { ...x, priority: Number(e.target.value || 0) } : x))} placeholder="优先级" />
                            <div className="flex items-center justify-between gap-2">
                                <Button size="sm" variant={p.enabled ? 'primary' : 'secondary'} onClick={() => setDraftProviders((prev) => prev.map((x) => x.id === p.id ? { ...x, enabled: !x.enabled } : x))}>
                                    {p.enabled ? '启用' : '停用'}
                                </Button>
                                <Button size="sm" variant="ghost" title="设为后端默认 (.env)" onClick={() => handleSetAsSystemDefault(p)}>
                                    <Save className="w-4 h-4 text-blue-500" />
                                </Button>
                                <Button size="sm" variant="ghost" onClick={() => {
                                    setDraftProviders((prev) => prev.filter((x) => x.id !== p.id));
                                    removeProvider(p.id);
                                }}>
                                    删除
                                </Button>
                            </div>
                            <div className="md:col-span-7 text-[11px] text-slate-500">ID: {p.id} · 排序位: {idx + 1}</div>
                        </div>
                    ))}
                </div>
                <div className="flex justify-end gap-2">
                    <Button variant="secondary" onClick={handleAddProvider}>新增厂商</Button>
                    <Button onClick={handleSaveProviders}>保存模型配置</Button>
                </div>
            </CardContent>
        </Card>

        <Card className="overflow-hidden bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-700 text-slate-900 dark:text-slate-100 shadow-lg">
            <CardHeader className="border-b border-slate-200 dark:border-slate-700 bg-slate-100/60 dark:bg-slate-800/60">
                <CardTitle className="text-slate-900 dark:text-slate-100 flex items-center gap-2">
                    <div className="w-5 h-5 flex items-center justify-center font-bold text-xs bg-slate-200 dark:bg-slate-700 rounded-full">i</div>
                    About
                </CardTitle>
            </CardHeader>
            <CardContent className="pt-6">
                <div className="space-y-4 text-sm text-slate-600 dark:text-slate-300">
                    <div className="grid grid-cols-2 gap-4">
                        <div className="space-y-1">
                            <span className="text-slate-500 text-xs uppercase tracking-wider">Version</span>
                            <p className="font-mono text-slate-700 dark:text-slate-200">0.2.0-beta</p>
                        </div>
                        <div className="space-y-1">
                            <span className="text-slate-500 text-xs uppercase tracking-wider">Environment</span>
                            <p className="font-mono text-slate-700 dark:text-slate-200">{import.meta.env.MODE}</p>
                        </div>
                    </div>
                    <div className="pt-4 border-t border-slate-200 dark:border-slate-800">
                        <p className="font-medium text-slate-800 dark:text-slate-200">Crablet Agent System</p>
                        <p className="text-xs mt-1 text-slate-500 dark:text-slate-500">Powered by Rust, React, and LLMs.</p>
                    </div>
                </div>
            </CardContent>
        </Card>
      </div>
    </div>
  );
};
