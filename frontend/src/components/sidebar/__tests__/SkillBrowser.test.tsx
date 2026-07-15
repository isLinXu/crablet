import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  listSkills: vi.fn(),
  getTopSkills: vi.fn(),
}));

vi.mock('@/services/skillService', () => ({
  skillService: {
    listSkills: mocks.listSkills,
    getTopSkills: mocks.getTopSkills,
  },
}));

vi.mock('../skills/SemanticSearch', () => ({ SemanticSearch: () => null }));
vi.mock('../skills/SkillRunner', () => ({ SkillRunner: () => null }));
vi.mock('../skills/SkillLogs', () => ({ SkillLogs: () => null }));
vi.mock('../skills/SkillCreator', () => ({ CreateSkillButton: () => null }));

import { SkillBrowser } from '../SkillBrowser';

describe('SkillBrowser', () => {
  beforeEach(() => {
    mocks.listSkills.mockReset();
    mocks.getTopSkills.mockReset();
    mocks.getTopSkills.mockResolvedValue({ items: [] });
  });

  it('renders normalized installed skills', async () => {
    mocks.listSkills.mockResolvedValue([
      { name: 'alpha', description: '', version: '', enabled: true },
    ]);

    render(<SkillBrowser />);

    expect(await screen.findByText('alpha')).toBeInTheDocument();
  });

  it('contains a failed request in the page and offers retry', async () => {
    mocks.listSkills.mockRejectedValue(new Error('gateway unavailable'));

    render(<SkillBrowser />);

    expect(await screen.findByText('技能加载失败')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '重试' })).toBeInTheDocument();
    await waitFor(() => expect(mocks.listSkills).toHaveBeenCalledTimes(1));
  });
});
