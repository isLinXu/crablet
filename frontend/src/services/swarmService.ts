import { api } from '@/services/api';
import type { SwarmStatsData, SwarmTasksResponse } from '@/types/domain';

export const swarmService = {
  getStats: async () => {
    const payload = await api.get<{ stats: SwarmStatsData }>('/v1/swarm/stats');
    return payload.stats;
  },
  getTasks: async () => {
    const payload = await api.get<SwarmTasksResponse>('/v1/swarm/tasks');
    return payload;
  },
  getState: () => api.get('/v1/swarm/state'),
  getAgents: () => api.get('/v1/swarm/agents'),
};
