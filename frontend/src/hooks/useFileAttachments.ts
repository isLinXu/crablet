/**
 * useFileAttachments — 管理文件附件的完整生命周期：
 *   选取 → 安全扫描 → 哈希计算 → 上传归档 → 状态追踪
 */
import { useState, useEffect } from 'react';
import { validateFile, heuristicSecurityScan, computeFileHash, extractTagsByName } from '@/utils/filePipeline';
import { knowledgeService } from '@/services/knowledgeService';
import { archiveIndexService } from '@/services/archiveIndexService';
import toast from 'react-hot-toast';
import type { PendingAttachment } from '@/components/chat/types';

export function useFileAttachments() {
  const [attachments, setAttachments] = useState<PendingAttachment[]>([]);

  /** 处理 <input type="file"> 的 FileList，逐文件校验后加入队列 */
  const handlePickFiles = async (files: FileList | null) => {
    if (!files?.length) return;
    const next: PendingAttachment[] = [];
    for (const file of Array.from(files)) {
      const check = validateFile(file);
      if (!check.ok) {
        toast.error(`${file.name}: ${check.reason}`);
        continue;
      }
      const security = await heuristicSecurityScan(file);
      if (!security.safe) {
        toast.error(`${file.name}: ${security.reason}`);
        continue;
      }
      const hash = await computeFileHash(file);
      next.push({
        id: `${Date.now()}-${Math.random()}`,
        file,
        progress: 0,
        status: 'pending',
        hash,
      });
    }
    setAttachments((prev) => [...prev, ...next]);
  };

  /** 将单个附件归档到知识库 */
  const archiveAttachment = async (item: PendingAttachment, autoRule = 'manual') => {
    setAttachments((prev) =>
      prev.map((x) => (x.id === item.id ? { ...x, status: 'uploading', progress: 1 } : x))
    );
    try {
      const tags = extractTagsByName(item.file.name);
      await knowledgeService.uploadFile(item.file, {
        tags,
        archivePath: '/default',
        autoRule,
        onProgress: (p) =>
          setAttachments((prev) =>
            prev.map((x) => (x.id === item.id ? { ...x, progress: p } : x))
          ),
      });
      archiveIndexService.upsert(item.hash || '', item.file.name, 'file', tags);
      setAttachments((prev) =>
        prev.map((x) => (x.id === item.id ? { ...x, status: 'uploaded', progress: 100 } : x))
      );
      toast.success(`${item.file.name} 已归档到知识库`);
    } catch {
      setAttachments((prev) =>
        prev.map((x) => (x.id === item.id ? { ...x, status: 'failed' } : x))
      );
      toast.error(`${item.file.name} 归档失败`);
    }
  };

  /** 自动归档符合规则的文档类型（当 crablet-auto-archive-enabled=1 时触发） */
  useEffect(() => {
    const enabled = localStorage.getItem('crablet-auto-archive-enabled') === '1';
    if (!enabled) return;
    attachments
      .filter((a) => a.status === 'pending')
      .forEach((a) => {
        if (/\.(pdf|doc|docx|txt|md|csv|xls|xlsx)$/i.test(a.file.name)) {
          archiveAttachment(a, 'auto-doc-rule');
        }
      });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [attachments]);

  const clearAttachments = () => setAttachments([]);

  return {
    attachments,
    setAttachments,
    handlePickFiles,
    archiveAttachment,
    clearAttachments,
  };
}
