import { api } from '@/services/api';
import type { ApiKeyInfo, McpOverview, RoutingEvaluationReport, RoutingSettings } from '@/types/domain';

export const settingsService = {
  listApiKeys: () => api.get<ApiKeyInfo[]>('/v1/settings/keys'),
  createApiKey: (name: string) => api.post<{ key?: string; id?: string; name?: string }>('/v1/settings/keys', { name }),
  revokeApiKey: (id: string) => api.delete<void>(`/v1/settings/keys/${id}`),
  getRoutingSettings: () => api.get<RoutingSettings>('/v1/settings/routing'),
  updateRoutingSettings: (payload: RoutingSettings) => api.put<RoutingSettings>('/v1/settings/routing', payload),
  getRoutingReport: (window = 200) => api.get<RoutingEvaluationReport>(`/v1/settings/routing/report?window=${window}`),
  getMcpOverview: () => api.get<McpOverview>('/v1/mcp/overview'),
};
