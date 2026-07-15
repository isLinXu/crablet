import { beforeEach, describe, expect, it, vi } from 'vitest';

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock('@/services/api', () => ({
  api: {
    get,
    post: vi.fn(),
  },
}));

import { normalizeSkillsResponse, skillService } from '../skillService';

describe('skillService', () => {
  beforeEach(() => get.mockReset());

  it('normalizes the packaged gateway envelope', async () => {
    get.mockResolvedValue({ status: 'success', count: 2, skills: ['alpha', 'beta'] });

    await expect(skillService.listSkills()).resolves.toEqual([
      { name: 'alpha', description: '', version: '', enabled: true },
      { name: 'beta', description: '', version: '', enabled: true },
    ]);
  });

  it('keeps array responses compatible and fills optional fields', () => {
    expect(normalizeSkillsResponse([{ name: 'alpha', description: null, enabled: false } as never])).toEqual([
      { name: 'alpha', description: '', version: '', enabled: false },
    ]);
  });

  it.each([null, undefined, {}, { skills: null }, { skills: 'invalid' }])(
    'returns an empty list for malformed response %#',
    (response) => {
      expect(normalizeSkillsResponse(response)).toEqual([]);
    },
  );

  it('drops malformed entries without crashing the page', () => {
    expect(normalizeSkillsResponse({ skills: [null, {}, { name: '' }, 'valid'] })).toEqual([
      { name: 'valid', description: '', version: '', enabled: true },
    ]);
  });
});
