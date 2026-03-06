import { useState, useCallback } from 'react';

interface UseApiOptions<T> {
  onSuccess?: (data: T) => void;
  onError?: (error: Error) => void;
  initialData?: T;
}

export function useApi<T = any, A extends any[] = any[]>(
  apiFunc: (...args: A) => Promise<{ data: T }>,
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
        setData(response.data);
        onSuccess?.(response.data);
        return response.data;
      } catch (err: any) {
        setError(err);
        onError?.(err);
        throw err;
      } finally {
        setLoading(false);
      }
    },
    [apiFunc, onSuccess, onError]
  );

  return { data, loading, error, execute, setData };
}
