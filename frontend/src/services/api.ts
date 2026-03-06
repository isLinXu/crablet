import client from '@/api/client';

export const api = {
  get: async <T>(url: string, params?: Record<string, unknown>): Promise<T> => {
    const response = await client.get(url, params ? { params } : undefined);
    return response.data as T;
  },
  post: async <T>(url: string, body?: unknown): Promise<T> => {
    const response = await client.post(url, body);
    return response.data as T;
  },
  put: async <T>(url: string, body?: unknown): Promise<T> => {
    const response = await client.put(url, body);
    return response.data as T;
  },
  delete: async <T>(url: string): Promise<T> => {
    const response = await client.delete(url);
    return response.data as T;
  },
};
