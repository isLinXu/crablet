import type { ApiKeyInfo, BatchTestResult, McpOverview, RegistrySkillItem, RoutingEvaluationReport, RoutingSettings, Skill, SkillsShTopItem } from '@/types/domain';
import { settingsService } from '@/services/settingsService';
import { skillService } from '@/services/skillService';
export type { ApiKeyInfo, BatchTestResult, McpOverview, RegistrySkillItem, RoutingEvaluationReport, RoutingSettings, SkillsShTopItem } from '@/types/domain';

export const settingsApi = {
  getSkills: async () => ({ data: await skillService.listSkills() as Skill[] }),
  toggleSkill: async (skillName: string, enabled: boolean) => ({ data: await skillService.toggleSkill(skillName, enabled) }),
  searchRegistrySkills: async (q: string) => ({ data: await skillService.searchRegistry(q) as { status: string; source?: string; items: RegistrySkillItem[] } }),
  getTopSkills: async (limit = 100) => ({ data: await skillService.getTopSkills(limit) as { status: string; source?: string; items: SkillsShTopItem[] } }),
  installSkill: async (payload: { name?: string; url?: string; source?: string; skill_id?: string }) => ({ data: await skillService.install(payload) }),
  batchTestSkills: async (skills: string[]) => ({ data: await skillService.batchTest(skills) as { status: string; results: BatchTestResult[] } }),
  listApiKeys: async () => ({ data: await settingsService.listApiKeys() as ApiKeyInfo[] }),
  createApiKey: async (name: string) => ({ data: await settingsService.createApiKey(name) }),
  revokeApiKey: async (id: string) => ({ data: await settingsService.revokeApiKey(id) }),
  getRoutingSettings: async () => ({ data: await settingsService.getRoutingSettings() as RoutingSettings }),
  updateRoutingSettings: async (payload: RoutingSettings) => ({ data: await settingsService.updateRoutingSettings(payload) as RoutingSettings }),
  getRoutingReport: async (window = 200) => ({ data: await settingsService.getRoutingReport(window) as RoutingEvaluationReport }),
  getMcpOverview: async () => ({ data: await settingsService.getMcpOverview() as McpOverview }),
};
