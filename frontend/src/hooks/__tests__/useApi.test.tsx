import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useApi } from '../useApi';

describe('useApi', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should initialize with loading=false and data=initialData', () => {
    const mockApi = vi.fn();
    const { result } = renderHook(() => useApi(mockApi, { initialData: 'init' }));

    expect(result.current.loading).toBe(false);
    expect(result.current.data).toBe('init');
    expect(result.current.error).toBe(null);
  });

  it('should handle successful api call', async () => {
    const mockData = { id: 1 };
    const mockApi = vi.fn().mockResolvedValue({ data: mockData });
    const onSuccess = vi.fn();

    const { result } = renderHook(() => useApi(mockApi, { onSuccess }));

    // Trigger execution
    await act(async () => {
        await result.current.execute('arg1');
    });

    expect(result.current.loading).toBe(false);
    expect(result.current.data).toEqual(mockData);
    expect(result.current.error).toBe(null);
    expect(onSuccess).toHaveBeenCalledWith(mockData);
    expect(mockApi).toHaveBeenCalledWith('arg1');
  });

  it('should handle api error', async () => {
    const mockError = new Error('Failed');
    const mockApi = vi.fn().mockRejectedValue(mockError);
    const onError = vi.fn();

    const { result } = renderHook(() => useApi(mockApi, { onError }));

    // Trigger execution
    await act(async () => {
        try {
            await result.current.execute();
        } catch {
            // Expected to throw
        }
    });

    expect(result.current.loading).toBe(false);
    expect(result.current.data).toBeUndefined();
    expect(result.current.error).toBe(mockError);
    expect(onError).toHaveBeenCalledWith(mockError);
  });
});
