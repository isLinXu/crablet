import { useState, useCallback } from 'react';

interface UseApiOptions<T> {
  onSuccess?: (data: T) => void;
  onError?: (error: Error) => void;
  initialData?: T;
}

const toError = (value: unknown): Error =>
  value instanceof Error ? value : new Error(typeof value === 'string' ? value : 'Unknown API error');

export function useApi<T = unknown, A extends unknown[] = unknown[]>(
  apiFunc: (...args: A) => Promise<T | { data: T }>,
  options: UseApiOptions<T> = {}
) {
  const { onSuccess, onError } = options;
  const [data, setData] = useState<T | undefined>(options.initialData);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const execute = useCallback(
    async (...args: A) => {
      setLoading(true);
      setError(null);
      try {
        const response = await apiFunc(...args);
        const data = (typeof response === 'object' && response !== null && 'data' in response)
          ? (response as { data: T }).data
          : response as T;
        setData(data);
        onSuccess?.(data);
        return data;
      } catch (err: unknown) {
        const error = toError(err);
        setError(error);
        onError?.(error);
        throw error;
      } finally {
        setLoading(false);
      }
    },
    [apiFunc, onSuccess, onError]
  );

  return { data, loading, error, execute, setData };
}
