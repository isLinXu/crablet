export type { Message, ContentPart, ChatSession, Skill, KnowledgeDocument } from '@/types/domain';

export interface ApiResponse<T> {
  data: T;
  status: string;
  message?: string;
}
