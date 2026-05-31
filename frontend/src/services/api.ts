import client from '@/api/client';
import { normalizeApiRequestPath } from '@/utils/constants';

const normalizeUrl = (url: string) => {
  if (/^https?:\/\//i.test(url)) {
    return url;
  }

  return normalizeApiRequestPath(url);
};

export const api = {
  get: async <T>(url: string, params?: Record<string, unknown>): Promise<T> => {
    const response = await client.get(normalizeUrl(url), params ? { params } : undefined);
    return response.data as T;
  },
  post: async <T>(url: string, body?: unknown): Promise<T> => {
    const response = await client.post(normalizeUrl(url), body);
    return response.data as T;
  },
  put: async <T>(url: string, body?: unknown): Promise<T> => {
    const response = await client.put(normalizeUrl(url), body);
    return response.data as T;
  },
  delete: async <T>(url: string): Promise<T> => {
    const response = await client.delete(normalizeUrl(url));
    return response.data as T;
  },
};
