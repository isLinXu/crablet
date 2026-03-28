import { api } from '@/services/api';
import type { AgentInfo, DashboardStats, HitlReview, SwarmStatsData, SwarmTasksResponse } from '@/types/domain';

export const dashboardService = {
  getDashboardStats: () => api.get<DashboardStats>('/dashboard'),
  getSwarmGraphs: async (page = 1, limit = 10, status = 'Active', query = '') => {
    const payload = await api.get<SwarmTasksResponse>('/swarm/tasks', { page, limit, status, q: query });
    return {
      graphs: payload?.graphs || [],
      pagination: payload?.pagination,
    };
  },
  batchSwarmAction: (action: 'pause' | 'resume' | 'delete', ids: string[]) =>
    api.post<void>('/swarm/tasks/batch', { action, ids }),
  pauseSwarmTask: (graphId: string) => api.post<void>(`/swarm/tasks/${graphId}/pause`),
  resumeSwarmTask: (graphId: string) => api.post<void>(`/swarm/tasks/${graphId}/resume`),
  updateTaskPrompt: (graphId: string, nodeId: string, prompt: string, dependencies?: string[]) =>
    api.put<void>(`/swarm/tasks/${graphId}/nodes/${nodeId}`, { prompt, dependencies }),
  retryTaskNode: (graphId: string, nodeId: string) => api.post<void>(`/swarm/tasks/${graphId}/nodes/${nodeId}/retry`),
  addTaskToGraph: async (graphId: string, role: string, prompt: string, dependencies: string[] = []) => {
    const payload = await api.post<{ task_id: string }>(`/swarm/tasks/${graphId}/nodes`, {
      agent_role: role,
      prompt,
      dependencies,
    });
    return payload.task_id;
  },
  getSwarmAgents: async () => {
    const payload = await api.get<{ agents?: AgentInfo[] }>('/swarm/agents');
    return payload.agents || [];
  },
  getSwarmStats: async () => {
    const payload = await api.get<{ stats: SwarmStatsData }>('/swarm/stats');
    return payload.stats;
  },
  listSwarmReviews: async () => {
    const payload = await api.get<{ reviews?: HitlReview[] }>('/swarm/reviews');
    return payload.reviews || [];
  },
  decideSwarmReview: async (
    taskId: string,
    payload: { decision: 'approved' | 'rejected' | 'edited' | 'selected' | 'feedback'; value?: string; selected_index?: number }
  ) => {
    return api.post<void>(`/swarm/reviews/${encodeURIComponent(taskId)}/decision`, payload);
  },
};
