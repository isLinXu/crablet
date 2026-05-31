import { api } from '@/services/api';
import type { SwarmStatsData, SwarmTasksResponse } from '@/types/domain';

const SWARM_API_PREFIX = '/v1/swarm';

export const swarmService = {
  getStats: async () => {
    const payload = await api.get<{ stats: SwarmStatsData }>(`${SWARM_API_PREFIX}/stats`);
    return payload.stats;
  },
  getTasks: async () => {
    const payload = await api.get<SwarmTasksResponse>(`${SWARM_API_PREFIX}/tasks`);
    return payload;
  },
  getState: () => api.get(`${SWARM_API_PREFIX}/state`),
  getAgents: () => api.get(`${SWARM_API_PREFIX}/agents`),
};
