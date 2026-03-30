import { beforeEach, describe, expect, it, vi } from 'vitest';

// Mock chatPhase3Service to avoid actual API calls
vi.mock('@/services/chatPhase3Service', () => ({
  chatPhase3Service: {
    listStars: vi.fn().mockResolvedValue({
      status: 'success',
      stars: [],
      count: 0,
    }),
    starMessage: vi.fn().mockResolvedValue({
      status: 'starred',
      id: 'star-1',
      session_id: 's1',
      message_id: 'm1',
      created_at: 1234567890,
    }),
    unstarMessage: vi.fn().mockResolvedValue(undefined),
  },
}));

import { useMessageStarsStore } from '../messageStarsStore';

describe('useMessageStarsStore', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
    useMessageStarsStore.setState({
      stars: [],
      starCount: 0,
      isLoading: false,
      error: null,
      starredMessageIds: new Set(),
    });
  });

  it('starts with empty state', () => {
    const state = useMessageStarsStore.getState();
    expect(state.stars).toEqual([]);
    expect(state.starCount).toBe(0);
    expect(state.isLoading).toBe(false);
    expect(state.error).toBeNull();
    expect(state.starredMessageIds.size).toBe(0);
  });

  it('clears stars', () => {
    useMessageStarsStore.setState({
      stars: [{ id: '1', session_id: 's1', message_id: 'm1', created_at: 1 }],
      starCount: 1,
      starredMessageIds: new Set(['m1']),
      error: 'some error',
    });

    useMessageStarsStore.getState().clearStars();
    const state = useMessageStarsStore.getState();
    expect(state.stars).toEqual([]);
    expect(state.starCount).toBe(0);
    expect(state.starredMessageIds.size).toBe(0);
    expect(state.error).toBeNull();
  });

  it('checks if a message is starred', () => {
    useMessageStarsStore.setState({
      starredMessageIds: new Set(['m1', 'm2']),
    });

    expect(useMessageStarsStore.getState().isStarred('m1')).toBe(true);
    expect(useMessageStarsStore.getState().isStarred('m3')).toBe(false);
  });

  it('sets error', () => {
    useMessageStarsStore.getState().setError('Network error');
    expect(useMessageStarsStore.getState().error).toBe('Network error');
    useMessageStarsStore.getState().setError(null);
    expect(useMessageStarsStore.getState().error).toBeNull();
  });

  it('loads stars from backend', async () => {
    const { chatPhase3Service } = await import('@/services/chatPhase3Service');
    vi.mocked(chatPhase3Service.listStars).mockResolvedValue({
      status: 'success',
      session_id: 's1',
      stars: [
        { id: 'star-1', session_id: 's1', message_id: 'm1', created_at: 100 } as any,
        { id: 'star-2', session_id: 's1', message_id: 'm2', created_at: 200 } as any,
      ],
      count: 2,
    });

    await useMessageStarsStore.getState().loadStars('s1');

    const state = useMessageStarsStore.getState();
    expect(state.isLoading).toBe(false);
    expect(state.stars).toHaveLength(2);
    expect(state.starCount).toBe(2);
    expect(state.starredMessageIds.has('m1')).toBe(true);
    expect(state.starredMessageIds.has('m2')).toBe(true);
  });

  it('handles loadStars failure', async () => {
    const { chatPhase3Service } = await import('@/services/chatPhase3Service');
    vi.mocked(chatPhase3Service.listStars).mockRejectedValue(new Error('fail'));

    await useMessageStarsStore.getState().loadStars('s1');

    const state = useMessageStarsStore.getState();
    expect(state.isLoading).toBe(false);
    expect(state.error).toBe('Failed to load stars');
  });

  it('stars a message', async () => {
    const { chatPhase3Service } = await import('@/services/chatPhase3Service');
    vi.mocked(chatPhase3Service.starMessage).mockResolvedValue({
      status: 'starred',
      id: 'star-new',
      session_id: 's1',
      message_id: 'm-new',
      created_at: 999,
    });

    const result = await useMessageStarsStore.getState().starMessage('s1', 'm-new');
    expect(result).toBe(true);

    const state = useMessageStarsStore.getState();
    expect(state.stars).toHaveLength(1);
    expect(state.starCount).toBe(1);
    expect(state.starredMessageIds.has('m-new')).toBe(true);
  });

  it('returns false when starMessage backend fails', async () => {
    const { chatPhase3Service } = await import('@/services/chatPhase3Service');
    vi.mocked(chatPhase3Service.starMessage).mockResolvedValue({ status: 'error' } as any);

    const result = await useMessageStarsStore.getState().starMessage('s1', 'm1');
    expect(result).toBe(false);
  });

  it('unstars a message', async () => {
    // Pre-populate
    useMessageStarsStore.setState({
      stars: [{ id: 'star-1', session_id: 's1', message_id: 'm1', created_at: 100 } as any],
      starCount: 1,
      starredMessageIds: new Set(['m1']),
    });

    const result = await useMessageStarsStore.getState().unstarMessage('s1', 'm1');
    expect(result).toBe(true);

    const state = useMessageStarsStore.getState();
    expect(state.stars).toHaveLength(0);
    expect(state.starCount).toBe(0);
    expect(state.starredMessageIds.has('m1')).toBe(false);
  });

  it('handles unstarMessage failure gracefully', async () => {
    const { chatPhase3Service } = await import('@/services/chatPhase3Service');
    vi.mocked(chatPhase3Service.unstarMessage).mockRejectedValue(new Error('fail'));

    const result = await useMessageStarsStore.getState().unstarMessage('s1', 'm1');
    expect(result).toBe(false);
    expect(useMessageStarsStore.getState().error).toBe('Failed to unstar message');
  });
});
