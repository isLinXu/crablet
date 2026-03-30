import React from 'react';
import { useThemeStore } from '../../store/themeStore';
import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card';
import { Settings, Moon, Sun, Monitor, KeyRound, Link, User, Trash2, Server, Save } from 'lucide-react';
import clsx from 'clsx';
import { Input } from '../ui/Input';
import { Button } from '../ui/Button';
import { useSettingsState } from './useSettingsState';
import {
  VENDOR_OPTIONS,
  VENDOR_GUIDE,
  KEY_PLACEHOLDER,
  TROUBLESHOOTING_GUIDE,
  modelSuggestionsForVendor,
  normalizeVendorName,
  envVendorToRouteVendor,
  detectVendor,
} from './settingsHelpers';

const API_PRESETS = [
  { label: 'Local Proxy', value: '/api' },
  { label: 'Local Backend', value: 'http://127.0.0.1:18789/api' },
  { label: 'OpenAI', value: 'https://api.openai.com/v1' },
  { label: 'Anthropic', value: 'https://api.anthropic.com/v1' },
  { label: 'Google', value: 'https://generativelanguage.googleapis.com/v1beta' },
  { label: '阿里百炼', value: 'https://dashscope.aliyuncs.com/compatible-mode/v1' },
  { label: '腾讯混元', value: 'https://api.hunyuan.cloud.tencent.com/v1' },
  { label: '字节豆包', value: 'https://ark.cn-beijing.volces.com/api/v3' },
];

export const SettingsPanel: React.FC = () => {
  const { theme, setTheme } = useThemeStore();
  const s = useSettingsState();

  const systemVendorDisplay = normalizeVendorName(
    s.systemConfig.llm_vendor
      ? envVendorToRouteVendor(s.systemConfig.llm_vendor)
      : (detectVendor(s.systemConfig.openai_api_base || '') || 'Custom')
  );

  return (
    <div className="h-full px-4 py-6 pb-10 sm:px-6 md:px-8 overflow-y-auto bg-gray-50 dark:bg-gray-900">
      <div className="max-w-4xl mx-auto space-y-6">
        <div className="flex justify-between items-center">
          <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100 flex items-center gap-2">
            <Settings className="w-6 h-6" />
            Settings
          </h1>
        </div>

        {/* 账户与MCP */}
        <Card className="overflow-hidden bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-700 text-slate-900 dark:text-slate-100 shadow-lg">
          <CardHeader className="border-b border-slate-200 dark:border-slate-700 bg-slate-100/60 dark:bg-slate-800/60">
            <CardTitle className="text-slate-900 dark:text-slate-100 flex items-center gap-2">
              <User className="w-5 h-5" />
              账户与MCP
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-6 pt-6">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
              <Input value={s.profileName} onChange={(e) => s.setProfileName(e.target.value)} placeholder="姓名" />
              <Input value={s.profileEmail} onChange={(e) => s.setProfileEmail(e.target.value)} placeholder="邮箱" />
              <Input value={s.profileOrg} onChange={(e) => s.setProfileOrg(e.target.value)} placeholder="组织" />
            </div>
            <div className="flex justify-end">
              <Button onClick={s.handleSaveProfile} variant="secondary">保存账户信息</Button>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
              <div className="rounded border border-slate-200 dark:border-slate-700 p-3">
                <div className="text-xs text-slate-500">MCP Tools</div>
                <div className="text-xl font-semibold">{s.mcpOverview?.mcp_tools ?? '-'}</div>
              </div>
              <div className="rounded border border-slate-200 dark:border-slate-700 p-3">
                <div className="text-xs text-slate-500">MCP Resources</div>
                <div className="text-xl font-semibold">{s.mcpOverview?.resources ?? '-'}</div>
              </div>
              <div className="rounded border border-slate-200 dark:border-slate-700 p-3">
                <div className="text-xs text-slate-500">MCP Prompts</div>
                <div className="text-xl font-semibold">{s.mcpOverview?.prompts ?? '-'}</div>
              </div>
            </div>
            <div>
              <div className="text-sm font-medium mb-2 flex items-center gap-2">
                <Server className="w-4 h-4" />
                API Keys 管理
              </div>
              <div className="flex gap-2 mb-3">
                <Input value={s.newKeyName} onChange={(e) => s.setNewKeyName(e.target.value)} placeholder="新密钥名称" />
                <Button onClick={s.handleCreateKey}>创建</Button>
              </div>
              <div className="space-y-2">
                {s.apiKeys.map((key) => (
                  <div key={key.id} className="flex items-center justify-between rounded border border-slate-200 dark:border-slate-700 p-2">
                    <div className="min-w-0">
                      <div className="text-sm font-medium truncate">{key.name}</div>
                      <div className="text-xs text-slate-500 truncate">{key.id}</div>
                    </div>
                    <Button variant="ghost" size="icon" onClick={() => s.handleRevokeKey(key.id)}>
                      <Trash2 className="w-4 h-4 text-red-500" />
                    </Button>
                  </div>
                ))}
              </div>
            </div>
          </CardContent>
        </Card>

        {/* 元认知路由 */}
        <Card className="overflow-hidden bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-700 text-slate-900 dark:text-slate-100 shadow-lg">
          <CardHeader className="border-b border-slate-200 dark:border-slate-700 bg-slate-100/60 dark:bg-slate-800/60">
            <CardTitle className="text-slate-900 dark:text-slate-100 flex items-center gap-2">
              <Server className="w-5 h-5" />
              元认知路由（Bandit）
            </CardTitle>
          </CardHeader>
          <CardContent className="pt-6 space-y-4">
            <label className="inline-flex items-center gap-2 text-sm">
              <input type="checkbox" checked={s.routingSettings.enable_adaptive_routing} onChange={(e) => s.setRoutingSettings((prev) => ({ ...prev, enable_adaptive_routing: e.target.checked }))} />
              <span>启用自适应路由（Contextual Bandit）</span>
            </label>
            <label className="inline-flex items-center gap-2 text-sm">
              <input type="checkbox" checked={s.routingSettings.enable_hierarchical_reasoning} onChange={(e) => s.setRoutingSettings((prev) => ({ ...prev, enable_hierarchical_reasoning: e.target.checked }))} />
              <span>启用分层推理控制（直觉→分析→元认知）</span>
            </label>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
              <div className="space-y-1">
                <div className="text-xs text-slate-500">system2_threshold (0~1)</div>
                <Input value={String(s.routingSettings.system2_threshold)} onChange={(e) => s.setRoutingSettings((prev) => ({ ...prev, system2_threshold: Number(e.target.value || 0) }))} />
              </div>
              <div className="space-y-1">
                <div className="text-xs text-slate-500">system3_threshold (0~1)</div>
                <Input value={String(s.routingSettings.system3_threshold)} onChange={(e) => s.setRoutingSettings((prev) => ({ ...prev, system3_threshold: Number(e.target.value || 0) }))} />
              </div>
              <div className="space-y-1">
                <div className="text-xs text-slate-500">bandit_exploration (0.05~2)</div>
                <Input value={String(s.routingSettings.bandit_exploration)} onChange={(e) => s.setRoutingSettings((prev) => ({ ...prev, bandit_exploration: Number(e.target.value || 0) }))} />
              </div>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-4 gap-3">
              {[
                { label: 'deliberate_threshold (0~1)', key: 'deliberate_threshold' },
                { label: 'meta_reasoning_threshold (0~1)', key: 'meta_reasoning_threshold' },
                { label: 'mcts_simulations (1~512)', key: 'mcts_simulations' },
                { label: 'mcts_exploration_weight (0.1~3)', key: 'mcts_exploration_weight' },
              ].map(({ label, key }) => (
                <div key={key} className="space-y-1">
                  <div className="text-xs text-slate-500">{label}</div>
                  <Input value={String((s.routingSettings as any)[key])} onChange={(e) => s.setRoutingSettings((prev) => ({ ...prev, [key]: Number(e.target.value || 0) }))} />
                </div>
              ))}
            </div>
            <div className="space-y-1">
              <div className="text-xs text-slate-500">graph_rag_entity_mode</div>
              <select value={s.routingSettings.graph_rag_entity_mode} onChange={(e) => s.setRoutingSettings((prev) => ({ ...prev, graph_rag_entity_mode: e.target.value as 'rule' | 'phrase' | 'hybrid' }))} className="h-10 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 px-3 text-sm">
                <option value="rule">rule</option><option value="phrase">phrase</option><option value="hybrid">hybrid</option>
              </select>
              <div className="text-[11px] text-slate-500">Rule偏稳定，Phrase偏召回，Hybrid综合平衡。</div>
            </div>
            <div className="text-xs text-slate-500">开启后将基于历史质量与延迟反馈学习最优System1/2/3路由，阈值用作基础边界控制。</div>
            {/* 路由评估报告 */}
            <div className="rounded border border-slate-200 dark:border-slate-700 p-3 space-y-3">
              <div className="flex items-center justify-between gap-2">
                <div className="text-sm font-medium">离线评估报告</div>
                <div className="flex items-center gap-2">
                  <Input value={String(s.routingReportWindow)} onChange={(e) => s.setRoutingReportWindow(Number(e.target.value || 200))} className="w-24" />
                  <Button variant="secondary" loading={s.loadingRoutingReport} onClick={s.handleRefreshRoutingReport}>刷新</Button>
                </div>
              </div>
              <div className="grid grid-cols-2 md:grid-cols-4 gap-2 text-xs">
                {[
                  { label: '反馈样本', value: s.routingReport?.total_feedback ?? 0 },
                  { label: '平均Reward', value: (s.routingReport?.avg_reward ?? 0).toFixed(3) },
                  { label: '平均质量', value: (s.routingReport?.avg_quality_score ?? 0).toFixed(3) },
                  { label: '平均延迟(ms)', value: (s.routingReport?.avg_latency_ms ?? 0).toFixed(1) },
                ].map(({ label, value }) => (
                  <div key={label} className="rounded border border-slate-200 dark:border-slate-700 p-2">
                    <div className="text-slate-500">{label}</div><div className="font-semibold">{value}</div>
                  </div>
                ))}
              </div>
              {(s.routingReport?.by_choice ?? []).length > 0 && (
                <div className="grid grid-cols-1 md:grid-cols-3 gap-2 text-xs">
                  {s.routingReport!.by_choice.map((c) => (
                    <div key={c.choice} className="rounded border border-slate-200 dark:border-slate-700 p-2">
                      <div className="font-medium">{c.choice}</div>
                      <div className="text-slate-500">count: {c.count}</div>
                      <div className="text-slate-500">avg_reward: {c.avg_reward?.toFixed(3) ?? 'N/A'}</div>
                      <div className="text-slate-500">avg_latency: {c.avg_latency_ms?.toFixed(1) ?? 'N/A'} ms</div>
                    </div>
                  ))}
                </div>
              )}
              <div className="grid grid-cols-2 md:grid-cols-4 gap-2 text-xs">
                {[
                  { label: '分层请求数', value: s.routingReport?.hierarchical_stats?.total_requests ?? 0 },
                  { label: '分析层触发', value: s.routingReport?.hierarchical_stats?.deliberate_activations ?? 0 },
                  { label: '元认知触发', value: s.routingReport?.hierarchical_stats?.meta_activations ?? 0 },
                  { label: '策略切换次数', value: s.routingReport?.hierarchical_stats?.strategy_switches ?? 0 },
                ].map(({ label, value }) => (
                  <div key={label} className="rounded border border-slate-200 dark:border-slate-700 p-2">
                    <div className="text-slate-500">{label}</div><div className="font-semibold">{value}</div>
                  </div>
                ))}
              </div>
            </div>
            <div className="flex justify-end">
              <Button onClick={s.handleSaveRoutingSettings} loading={s.savingRoutingSettings}>保存路由配置</Button>
            </div>
          </CardContent>
        </Card>

        {/* Backend LLM Configuration */}
        <Card className="overflow-hidden bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-700 text-slate-900 dark:text-slate-100 shadow-lg">
          <CardHeader className="border-b border-slate-200 dark:border-slate-700 bg-slate-100/60 dark:bg-slate-800/60">
            <CardTitle className="text-slate-900 dark:text-slate-100 flex items-center gap-2">
              <Server className="w-5 h-5" />
              Backend LLM Configuration (.env)
            </CardTitle>
          </CardHeader>
          <CardContent className="pt-6 space-y-4">
            {s.loadingSystemConfig && <div className="text-sm text-slate-500 animate-pulse">Loading backend config...</div>}
            <div className="text-xs text-slate-500 mb-4">这些配置直接对应后端的 .env 文件。修改后需要重启后端服务才能生效。</div>
            <div className="grid gap-2">
              <label className="text-sm font-medium">LLM Vendor</label>
              <div className="flex gap-2">
                <select value={systemVendorDisplay} onChange={(e) => s.applySystemVendorPreset(e.target.value as any)} className="h-10 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 px-3 text-sm">
                  {VENDOR_OPTIONS.map((v) => <option key={v} value={v}>{v}</option>)}
                </select>
                <Button variant="secondary" onClick={() => s.applySystemVendorPreset(systemVendorDisplay)}>应用推荐</Button>
              </div>
            </div>
            <div className="grid gap-2">
              <label className="text-sm font-medium">OpenAI / DashScope API Key</label>
              <Input type="password" value={s.systemConfig.openai_api_key || ''} onChange={(e) => s.setSystemConfig({ ...s.systemConfig, openai_api_key: e.target.value })} placeholder="sk-..." />
            </div>
            <div className="grid gap-2">
              <label className="text-sm font-medium">API Base URL</label>
              <Input value={s.systemConfig.openai_api_base || ''} onChange={(e) => s.setSystemConfig({ ...s.systemConfig, openai_api_base: e.target.value })} placeholder="https://dashscope.aliyuncs.com/compatible-mode/v1" />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="grid gap-2">
                <label className="text-sm font-medium">Model Name (Cloud)</label>
                <Input list="system-model-recommendations" value={s.systemConfig.openai_model_name || ''} onChange={(e) => s.setSystemConfig({ ...s.systemConfig, openai_model_name: e.target.value })} placeholder="qwen-plus" />
                <datalist id="system-model-recommendations">
                  {modelSuggestionsForVendor(systemVendorDisplay, 'chat').map((m) => <option key={m} value={m} />)}
                </datalist>
                <div className="text-[11px] text-slate-500">推荐模型：{modelSuggestionsForVendor(systemVendorDisplay, 'chat').join(' / ') || '无'}</div>
              </div>
              <div className="grid gap-2">
                <label className="text-sm font-medium">Ollama Model (Local)</label>
                <Input value={s.systemConfig.ollama_model || ''} onChange={(e) => s.setSystemConfig({ ...s.systemConfig, ollama_model: e.target.value })} placeholder="qwen3:4b" />
              </div>
            </div>
            <div className="flex justify-end pt-2">
              <Button onClick={s.handleSaveSystemConfig} loading={s.savingSystemConfig}>Save to .env</Button>
            </div>
          </CardContent>
        </Card>

        {/* API Configuration */}
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
                <Input value={s.apiBaseUrl} onChange={(e) => s.setApiBaseUrl(e.target.value)} placeholder="e.g., http://localhost:18789/api" className="bg-white dark:bg-slate-800 border-slate-300 dark:border-slate-600 text-slate-900 dark:text-slate-100 placeholder:text-slate-400 dark:placeholder:text-slate-500 focus:ring-blue-500 focus:border-blue-500" />
                <div className="flex flex-wrap gap-2">
                  {API_PRESETS.map((preset) => (
                    <Button key={preset.value} type="button" size="sm" variant={s.apiBaseUrl.trim() === preset.value ? 'primary' : 'secondary'} onClick={() => s.handlePresetSelect(preset.value)}>
                      {preset.label}
                    </Button>
                  ))}
                </div>
                <p className="text-xs text-slate-500 dark:text-slate-400">The base URL for the backend API. Default is <code className="bg-slate-100 dark:bg-slate-800 px-1 py-0.5 rounded">/api</code>.</p>
              </div>

              <div className="grid gap-2">
                <label className="text-sm font-medium text-slate-700 dark:text-slate-200 flex items-center gap-2">
                  <KeyRound className="w-4 h-4 text-slate-500 dark:text-slate-400" />
                  API Key / Bearer Token
                </label>
                <Input type="password" value={s.apiKey} onChange={(e) => s.setApiKey(e.target.value)} placeholder={s.currentVendor ? KEY_PLACEHOLDER[s.currentVendor] : "Enter your API Key or Bearer Token"} className="bg-white dark:bg-slate-800 border-slate-300 dark:border-slate-600 text-slate-900 dark:text-slate-100 placeholder:text-slate-400 dark:placeholder:text-slate-500 focus:ring-blue-500 focus:border-blue-500" />
                <p className="text-xs text-slate-500 dark:text-slate-400">Your authentication token. Leave empty if authentication is disabled.</p>
                {s.currentVendor && (
                  <div className="rounded border border-amber-300/70 bg-amber-50 dark:bg-amber-900/20 dark:border-amber-700 px-3 py-2 text-xs text-amber-800 dark:text-amber-200">
                    <div>厂商模式：{s.currentVendor}</div>
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate">建议端点：{VENDOR_GUIDE[s.currentVendor].endpoint}</span>
                      <Button size="sm" variant="secondary" onClick={s.handleCopyEndpoint}>复制端点</Button>
                    </div>
                    <label className="inline-flex items-center gap-2 mt-1">
                      <input type="checkbox" checked={s.autoFillEndpointOnCopy} onChange={(e) => s.handleToggleAutoFillEndpoint(e.target.checked)} />
                      <span>复制后自动填入API Base URL</span>
                    </label>
                    <div>Key示例：{VENDOR_GUIDE[s.currentVendor].keyHint}</div>
                  </div>
                )}
                {!!s.keyPatternIssue && (
                  <div className="rounded border border-rose-300/70 bg-rose-50 dark:bg-rose-900/20 dark:border-rose-700 px-3 py-2 text-xs text-rose-700 dark:text-rose-200">{s.keyPatternIssue}</div>
                )}
                {s.currentVendor && (
                  <div className="rounded border border-sky-300/70 bg-sky-50 dark:bg-sky-900/20 dark:border-sky-700 px-3 py-2 text-xs text-sky-800 dark:text-sky-200 space-y-1">
                    <div className="font-medium">常见排障建议</div>
                    <div>Key格式错误：{TROUBLESHOOTING_GUIDE[s.currentVendor].keyFormat}</div>
                    <div>Endpoint错误：{TROUBLESHOOTING_GUIDE[s.currentVendor].endpoint}</div>
                    <div>权限不足：{TROUBLESHOOTING_GUIDE[s.currentVendor].permission}</div>
                    <div>跨域限制：{TROUBLESHOOTING_GUIDE[s.currentVendor].cors}</div>
                  </div>
                )}
              </div>
              <div className="flex justify-end gap-2 pt-2">
                <Button variant="secondary" onClick={s.handleSyncModels} loading={s.syncingModels}>同步模型</Button>
                <Button variant="secondary" onClick={s.handleTestConnection} loading={s.testingConnection}>测试连接</Button>
                <Button onClick={s.handleSaveApiSettings} className="bg-blue-600 hover:bg-blue-700 text-white">Save & Apply</Button>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Appearance */}
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
                {([
                  { value: 'light' as const, icon: Sun, label: 'Light' },
                  { value: 'system' as const, icon: Monitor, label: 'System' },
                  { value: 'dark' as const, icon: Moon, label: 'Dark' },
                ]).map(({ value, icon: Icon, label }) => (
                  <button key={value} onClick={() => setTheme(value)} className={clsx("px-3 py-1.5 rounded-md transition-all flex items-center gap-2 text-sm font-medium", theme === value ? "bg-slate-600 text-white shadow-sm" : "text-slate-600 dark:text-slate-400 hover:text-slate-800 dark:hover:text-slate-200 hover:bg-slate-200 dark:hover:bg-slate-700/50")}>
                    <Icon className="w-4 h-4" /> {label}
                  </button>
                ))}
              </div>
            </div>
          </CardContent>
        </Card>

        {/* 多厂商模型管理 */}
        <Card className="overflow-hidden bg-white dark:bg-slate-900 border-slate-200 dark:border-slate-700 text-slate-900 dark:text-slate-100 shadow-lg">
          <CardHeader className="border-b border-slate-200 dark:border-slate-700 bg-slate-100/60 dark:bg-slate-800/60">
            <CardTitle className="text-slate-900 dark:text-slate-100 flex items-center gap-2">
              <Server className="w-5 h-5" />
              多厂商模型管理与智能路由
            </CardTitle>
          </CardHeader>
          <CardContent className="pt-6 space-y-4">
            <div className="text-xs text-slate-500">支持按会话路由、问题类型、优先级自动选择模型。每个会话可在聊天页手动覆盖。</div>
            <div className="space-y-3">
              {s.draftProviders.map((p, idx) => (
                <div key={p.id} className="rounded border border-slate-200 dark:border-slate-700 p-3 grid grid-cols-1 md:grid-cols-8 gap-2">
                  <select value={normalizeVendorName(p.vendor)} onChange={(e) => s.updateProviderVendor(p.id, e.target.value as any)} className="bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-600 rounded px-2 py-2 text-sm">
                    {VENDOR_OPTIONS.map((v) => <option key={v} value={v}>{v}</option>)}
                  </select>
                  <Input list={`provider-model-${idx}`} value={p.model} onChange={(e) => s.setDraftProviders((prev) => prev.map((x) => x.id === p.id ? { ...x, model: e.target.value } : x))} placeholder="模型名" />
                  <datalist id={`provider-model-${idx}`}>{modelSuggestionsForVendor(p.vendor, p.modelType || 'chat').map((m) => <option key={m} value={m} />)}</datalist>
                  <select value={p.modelType || 'chat'} onChange={(e) => s.updateProviderModelType(p.id, e.target.value as 'chat' | 'image')} className="bg-white dark:bg-slate-800 border border-slate-300 dark:border-slate-600 rounded px-2 py-2 text-sm">
                    <option value="chat">chat</option><option value="image">image</option>
                  </select>
                  <Input value={p.version} onChange={(e) => s.setDraftProviders((prev) => prev.map((x) => x.id === p.id ? { ...x, version: e.target.value } : x))} placeholder="版本" />
                  <Input value={p.apiBaseUrl} onChange={(e) => s.setDraftProviders((prev) => prev.map((x) => x.id === p.id ? { ...x, apiBaseUrl: e.target.value } : x))} placeholder="接口地址" />
                  <Input type="password" value={p.apiKey} onChange={(e) => s.setDraftProviders((prev) => prev.map((x) => x.id === p.id ? { ...x, apiKey: e.target.value } : x))} placeholder="API Key" />
                  <Input value={String(p.priority)} onChange={(e) => s.setDraftProviders((prev) => prev.map((x) => x.id === p.id ? { ...x, priority: Number(e.target.value || 0) } : x))} placeholder="优先级" />
                  <div className="flex items-center justify-between gap-2">
                    <Button size="sm" variant={p.enabled ? 'primary' : 'secondary'} onClick={() => s.setDraftProviders((prev) => prev.map((x) => x.id === p.id ? { ...x, enabled: !x.enabled } : x))}>{p.enabled ? '启用' : '停用'}</Button>
                    <Button size="sm" variant="ghost" title="设为后端默认 (.env)" onClick={() => s.handleSetAsSystemDefault(p)}><Save className="w-4 h-4 text-blue-500" /></Button>
                    <Button size="sm" variant="ghost" onClick={() => { s.setDraftProviders((prev) => prev.filter((x) => x.id !== p.id)); }}>{/* removeProvider called via useModelStore */}</Button>
                  </div>
                  <div className="md:col-span-7 text-[11px] text-slate-500">ID: {p.id} · 排序位: {idx + 1}</div>
                </div>
              ))}
            </div>
            <div className="flex justify-end gap-2">
              <Button variant="secondary" onClick={s.handleAddProvider}>新增厂商</Button>
              <Button onClick={s.handleSaveProviders}>保存模型配置</Button>
            </div>
          </CardContent>
        </Card>

        {/* About */}
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
