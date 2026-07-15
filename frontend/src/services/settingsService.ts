import { api } from '@/services/api';
import type { ApiKeyInfo, McpOverview, RoutingEvaluationReport, RoutingSettings } from '@/types/domain';

export interface SystemConfig {
  openai_api_key?: string;
  openai_api_key_masked?: string;
  openai_api_base?: string;
  openai_model_name?: string;
  ollama_model?: string;
  llm_vendor?: string;
  [key: string]: unknown;
}

interface McpOverviewResponse {
  status?: string;
  resources_count?: number;
  prompts_count?: number;
  resources?: unknown;
  prompts?: unknown;
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null;

const normalizeMcpOverview = (payload: McpOverviewResponse): McpOverview => {
  const resourceItems = Array.isArray(payload.resources)
    ? payload.resources.filter(isRecord).map((item) => ({
        uri: String(item.uri ?? ''),
        name: item.name == null ? undefined : String(item.name),
        description: item.description == null ? undefined : String(item.description),
      })).filter((item) => item.uri)
    : [];
  const promptItems = Array.isArray(payload.prompts)
    ? payload.prompts.filter(isRecord).map((item) => ({
        name: String(item.name ?? ''),
        description: item.description == null ? undefined : String(item.description),
      })).filter((item) => item.name)
    : [];

  return {
    status: typeof payload.status === 'string' ? payload.status : 'unknown',
    mcp_tools: 0,
    resources: typeof payload.resources_count === 'number' ? payload.resources_count : resourceItems.length,
    prompts: typeof payload.prompts_count === 'number' ? payload.prompts_count : promptItems.length,
    resource_items: resourceItems,
    prompt_items: promptItems,
  };
};

export const settingsService = {
  listApiKeys: () => api.get<ApiKeyInfo[]>('/v1/settings/keys'),
  createApiKey: (name: string) => api.post<{ key?: string; id?: string; name?: string }>('/v1/settings/keys', { name }),
  revokeApiKey: (id: string) => api.delete<void>(`/v1/settings/keys/${id}`),
  getRoutingSettings: () => api.get<RoutingSettings>('/v1/settings/routing'),
  updateRoutingSettings: (payload: RoutingSettings) => api.put<RoutingSettings>('/v1/settings/routing', payload),
  getRoutingReport: (window = 200) => api.get<RoutingEvaluationReport>(`/v1/settings/routing/report?window=${window}`),
  getSystemConfig: () => api.get<SystemConfig>('/v1/settings/system/config'),
  getModelHealth: () => api.get<{ status: string; model_configured: boolean; model_name: string; ollama_reachable?: boolean }>('/health/model'),
  updateSystemConfig: (payload: Partial<SystemConfig>) => api.post<SystemConfig>('/v1/settings/system/config', payload),
  getMcpOverview: async () => normalizeMcpOverview(
    await api.get<McpOverviewResponse>('/v1/mcp/overview'),
  ),
};
