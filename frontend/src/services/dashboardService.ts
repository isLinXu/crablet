import { api } from '@/services/api';
import type {
  AgentInfo,
  DashboardStats,
  HitlReview,
  SwarmReplaySnapshot,
  SwarmStatsData,
  SwarmTasksResponse,
  SwarmTimelineEntry,
} from '@/types/domain';

const normalizeDashboardStats = (payload: Partial<DashboardStats> | null | undefined): DashboardStats => ({
  status: typeof payload?.status === 'string' ? payload.status : 'unknown',
  skills_count: typeof payload?.skills_count === 'number' ? payload.skills_count : 0,
  active_tasks: typeof payload?.active_tasks === 'number' ? payload.active_tasks : 0,
  system_load: typeof payload?.system_load === 'string' ? payload.system_load : 'Unknown',
  skills: Array.isArray(payload?.skills) ? payload.skills : [],
});

export const dashboardService = {
  getDashboardStats: async () => normalizeDashboardStats(
    await api.get<Partial<DashboardStats>>('/dashboard'),
  ),
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
  recoverTaskNode: (
    graphId: string,
    nodeId: string,
    payload: { agent_role?: string; prompt?: string; dependencies?: string[]; resume_graph?: boolean }
  ) => api.post<void>(`/swarm/tasks/${graphId}/nodes/${nodeId}/recover`, payload),
  getSwarmTimeline: async (
    graphId: string,
    options?: {
      nodeId?: string;
      limit?: number;
      eventType?: string;
      messageType?: string;
      status?: string;
      query?: string;
    }
  ) => {
    const payload = await api.get<{ timeline?: SwarmTimelineEntry[] }>(`/swarm/tasks/${graphId}/timeline`, {
      limit: options?.limit ?? 50,
      node_id: options?.nodeId,
      event_type: options?.eventType,
      message_type: options?.messageType,
      status: options?.status,
      q: options?.query,
    });
    return payload.timeline || [];
  },
  getSwarmReplay: async (
    graphId: string,
    options?: { at?: number; nodeId?: string }
  ) => {
    const payload = await api.get<{ snapshot?: SwarmReplaySnapshot }>(`/swarm/tasks/${graphId}/replay`, {
      at: options?.at,
      node_id: options?.nodeId,
    });
    return payload.snapshot;
  },
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
