import { beforeEach, describe, expect, it, vi } from 'vitest';
import { api } from '@/services/api';
import { dashboardService } from '@/services/dashboardService';
import { settingsService } from '@/services/settingsService';

vi.mock('@/services/api', () => ({
  api: { get: vi.fn(), post: vi.fn(), put: vi.fn(), delete: vi.fn() },
}));

const getMock = vi.mocked(api.get);

describe('feature page API contracts', () => {
  beforeEach(() => getMock.mockReset());

  it('maps the MCP backend envelope to the page domain model', async () => {
    getMock.mockResolvedValue({
      status: 'success', resources_count: 2, prompts_count: 1,
      resources: [{ uri: 'server://one', name: 'One' }, null],
      prompts: [{ name: 'server.prompt' }],
    });

    await expect(settingsService.getMcpOverview()).resolves.toEqual({
      status: 'success', mcp_tools: 0, resources: 2, prompts: 1,
      resource_items: [{ uri: 'server://one', name: 'One', description: undefined }],
      prompt_items: [{ name: 'server.prompt', description: undefined }],
    });
  });

  it('uses safe empty collections for malformed MCP payloads', async () => {
    getMock.mockResolvedValue({ status: 'success', resources: {}, prompts: null });
    const result = await settingsService.getMcpOverview();
    expect(result.resource_items).toEqual([]);
    expect(result.prompt_items).toEqual([]);
    expect(result.resources).toBe(0);
    expect(result.prompts).toBe(0);
  });

  it('uses safe dashboard defaults when list fields are malformed', async () => {
    getMock.mockResolvedValue({ status: 'healthy', skills: null });
    await expect(dashboardService.getDashboardStats()).resolves.toMatchObject({
      status: 'healthy', skills_count: 0, active_tasks: 0,
      system_load: 'Unknown', skills: [],
    });
  });
});
