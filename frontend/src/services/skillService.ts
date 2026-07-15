import { api } from '@/services/api';
import type { Skill, BatchTestResult, RegistrySkillItem, SkillsShTopItem, SemanticSearchResult, SkillExecutionLog, SkillRunResult } from '@/types/domain';

type SkillsResponse = Skill[] | { skills?: unknown } | null | undefined;

const normalizeSkill = (value: unknown): Skill | null => {
  if (typeof value === 'string') {
    return { name: value, description: '', version: '', enabled: true };
  }
  if (!value || typeof value !== 'object') return null;

  const item = value as Partial<Skill>;
  if (typeof item.name !== 'string' || item.name.trim() === '') return null;
  return {
    name: item.name,
    description: typeof item.description === 'string' ? item.description : '',
    version: typeof item.version === 'string' ? item.version : '',
    enabled: typeof item.enabled === 'boolean' ? item.enabled : true,
  };
};

export const normalizeSkillsResponse = (response: SkillsResponse): Skill[] => {
  const values = Array.isArray(response)
    ? response
    : response && typeof response === 'object' && Array.isArray(response.skills)
      ? response.skills
      : [];
  return values.map(normalizeSkill).filter((skill): skill is Skill => skill !== null);
};

export const skillService = {
  listSkills: async () => normalizeSkillsResponse(await api.get<SkillsResponse>('/v1/skills')),
  toggleSkill: (skillName: string, enabled: boolean) => api.post('/v1/skills/' + skillName + '/toggle', { enabled }),
  searchRegistry: (q: string) => api.get<{ status: string; source?: string; items: RegistrySkillItem[] }>('/v1/skills/registry/search', { q }),
  install: (payload: { name?: string; url?: string; source?: string; skill_id?: string }) => api.post<{ status: string }>('/v1/skills/install', payload),
  batchTest: (skills: string[]) => api.post<{ status: string; results: BatchTestResult[] }>('/v1/skills/test/batch', { skills }),
  getTopSkills: (limit = 100) => api.get<{ status: string; source?: string; items: SkillsShTopItem[] }>('/v1/skills/top', { limit }),
  
  // 新增 API
  semanticSearch: (query: string, limit = 10, minSimilarity = 0.5) => 
    api.post<{ status: string; query: string; results: SemanticSearchResult[]; note?: string }>('/v1/skills/semantic-search', { query, limit, min_similarity: minSimilarity }),
  
  runSkill: (skillName: string, args?: Record<string, unknown>, timeoutSecs = 30) => 
    api.post<{ status: string; result?: SkillRunResult; error?: string }>(`/v1/skills/${skillName}/run`, { args, timeout_secs: timeoutSecs }),
  
  getSkillLogs: (skillName: string, limit = 50) => 
    api.get<{ status: string; skill_name: string; logs: SkillExecutionLog[]; total: number }>(`/v1/skills/${skillName}/logs`, { limit }),
  
  getAllLogs: (limit = 100) => 
    api.get<{ status: string; logs: SkillExecutionLog[]; total: number }>('/v1/skills/logs', { limit }),
};
