/**
 * useRetrievalSearch — 输入防抖 + 知识库向量检索
 *   当 input 长度 ≥ 6 时，延迟 280ms 发起检索，结果存入 retrievalHits
 */
import { useState, useEffect } from 'react';
import { knowledgeService } from '@/services/knowledgeService';
import type { RetrievalHit } from '@/components/chat/types';

export function useRetrievalSearch(input: string) {
  const [retrievalHits, setRetrievalHits] = useState<RetrievalHit[]>([]);
  const [retrieving, setRetrieving] = useState(false);
  const [selectedRetrieval, setSelectedRetrieval] = useState<number[]>([]);

  useEffect(() => {
    const q = input.trim();
    if (q.length < 6) {
      setRetrievalHits([]);
      setSelectedRetrieval([]);
      return;
    }
    const timer = setTimeout(async () => {
      try {
        setRetrieving(true);
        const results = await knowledgeService.search(q);
        const hits = (Array.isArray(results) ? results : []).slice(0, 5) as RetrievalHit[];
        setRetrievalHits(hits);
        setSelectedRetrieval(hits.map((_, i) => i));
      } catch {
        setRetrievalHits([]);
        setSelectedRetrieval([]);
      } finally {
        setRetrieving(false);
      }
    }, 280);
    return () => clearTimeout(timer);
  }, [input]);

  return {
    retrievalHits,
    retrieving,
    selectedRetrieval,
    setSelectedRetrieval,
    clearRetrievalHits: () => {
      setRetrievalHits([]);
      setSelectedRetrieval([]);
    },
  };
}
