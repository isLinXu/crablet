import type { AgentInfo, DashboardStats, SwarmStatsData, SwarmTasksResponse } from '@/types/domain';
import { dashboardService } from '@/services/dashboardService';

export type { AgentInfo, DashboardStats, Pagination, SkillManifest, SwarmStatsData, SwarmTasksResponse, TaskGraph, TaskNode, TaskStatus } from '@/types/domain';

export const getDashboardStats = (): Promise<DashboardStats> => dashboardService.getDashboardStats();
export const getSwarmGraphs = (page = 1, limit = 10, status = 'Active', query = ''): Promise<SwarmTasksResponse> =>
  dashboardService.getSwarmGraphs(page, limit, status, query);
export const batchSwarmAction = (action: 'pause' | 'resume' | 'delete', ids: string[]): Promise<void> =>
  dashboardService.batchSwarmAction(action, ids);
export const pauseSwarmTask = (graphId: string): Promise<void> => dashboardService.pauseSwarmTask(graphId);
export const resumeSwarmTask = (graphId: string): Promise<void> => dashboardService.resumeSwarmTask(graphId);
export const updateTaskPrompt = (graphId: string, nodeId: string, newPrompt: string, dependencies?: string[]): Promise<void> =>
  dashboardService.updateTaskPrompt(graphId, nodeId, newPrompt, dependencies);
export const getSwarmAgents = (): Promise<AgentInfo[]> => dashboardService.getSwarmAgents();
export const retryTaskNode = (graphId: string, nodeId: string): Promise<void> => dashboardService.retryTaskNode(graphId, nodeId);
export const addTaskToGraph = (graphId: string, role: string, prompt: string, dependencies: string[] = []): Promise<string> =>
  dashboardService.addTaskToGraph(graphId, role, prompt, dependencies);
export const getSwarmStats = (): Promise<SwarmStatsData> => dashboardService.getSwarmStats();
