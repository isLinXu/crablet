import { api } from '@/services/api';
import client from '@/api/client';
import type { KnowledgeDocument } from '@/types/domain';

export const knowledgeService = {
  listDocuments: async () => {
    const payload = await api.get<any>('/v1/knowledge/documents');
    return Array.isArray(payload) ? payload as KnowledgeDocument[] : (Array.isArray(payload?.documents) ? payload.documents : []);
  },
  search: async (q: string) => {
    const payload = await api.get<any>('/v1/knowledge/search', { q });
    return Array.isArray(payload?.results) ? payload.results : [];
  },
  uploadFile: async (
    file: File,
    options?: {
      onProgress?: (percent: number) => void;
      tags?: string[];
      archivePath?: string;
      autoRule?: string;
    }
  ) => {
    const formData = new FormData();
    formData.append('file', file);
    if (options?.tags?.length) formData.append('tags', JSON.stringify(options.tags));
    if (options?.archivePath) formData.append('archive_path', options.archivePath);
    if (options?.autoRule) formData.append('auto_rule', options.autoRule);
    const response = await client.post('/v1/knowledge/upload', formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
      onUploadProgress: (event) => {
        if (!options?.onProgress || !event.total) return;
        const percent = Math.round((event.loaded / event.total) * 100);
        options.onProgress(percent);
      },
    });
    return response.data as { id: string; source: string };
  },
  deleteDocument: async (docId: string) => {
    try {
      return await api.delete<void>(`/v1/knowledge/documents/${docId}`);
    } catch (error: any) {
      if (error?.response?.status === 404) {
        throw new Error('KNOWLEDGE_DELETE_UNSUPPORTED');
      }
      throw error;
    }
  },
};
