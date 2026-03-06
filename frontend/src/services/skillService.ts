import { api } from '@/services/api';
import type { Skill, BatchTestResult, RegistrySkillItem, SkillsShTopItem } from '@/types/domain';

export const skillService = {
  listSkills: () => api.get<Skill[]>('/v1/skills'),
  toggleSkill: (skillName: string, enabled: boolean) => api.post('/v1/skills/' + skillName + '/toggle', { enabled }),
  searchRegistry: (q: string) => api.get<{ status: string; source?: string; items: RegistrySkillItem[] }>('/v1/skills/registry/search', { q }),
  install: (payload: { name?: string; url?: string; source?: string; skill_id?: string }) => api.post<{ status: string }>('/v1/skills/install', payload),
  batchTest: (skills: string[]) => api.post<{ status: string; results: BatchTestResult[] }>('/v1/skills/test/batch', { skills }),
  getTopSkills: (limit = 100) => api.get<{ status: string; source?: string; items: SkillsShTopItem[] }>('/v1/skills/top', { limit }),
};
