import type { KnowledgeDocument, ApiResponse } from './types';
import { knowledgeService } from '@/services/knowledgeService';

export const knowledgeApi = {
  getDocuments: async () => {
    const data = await knowledgeService.listDocuments();
    return { data: data as KnowledgeDocument[] };
  },
  uploadFile: async (file: File): Promise<ApiResponse<{ id: string; source: string }>> => {
    const data = await knowledgeService.uploadFile(file);
    return { data, status: 'ok' };
  },
  deleteDocument: async (docId: string): Promise<ApiResponse<void>> => {
    await knowledgeService.deleteDocument(docId);
    return { data: undefined, status: 'ok' };
  },
  search: async (query: string) => {
    const data = await knowledgeService.search(query);
    return { data };
  },
};
