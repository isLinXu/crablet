export interface PendingAttachment {
  id: string;
  file: File;
  progress: number;
  status: 'pending' | 'uploading' | 'uploaded' | 'failed' | 'processing';
  hash?: string;
  isOcr?: boolean;
  ocrProgress?: number;
}

export interface RetrievalHit {
  content: string;
  score: number;
  metadata?: Record<string, unknown>;
}
