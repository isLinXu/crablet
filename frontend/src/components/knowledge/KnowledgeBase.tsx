import React, { useEffect, useState, useRef } from 'react';
import { useApi } from '../../hooks/useApi';
import type { KnowledgeDocument } from '@/types/domain';
import { Card } from '../ui/Card';
import { Button } from '../ui/Button';
import { Loader2, Database, Upload, Trash2, FileText } from 'lucide-react';
import { EmptyState } from '../ui/EmptyState';
import { Input } from '../ui/Input';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { format } from 'date-fns';
import toast from 'react-hot-toast';
import { knowledgeService } from '@/services/knowledgeService';
import { validateFile, heuristicSecurityScan, computeFileHash, extractTagsByName } from '@/utils/filePipeline';
import { archiveIndexService } from '@/services/archiveIndexService';
import { useLocation } from 'react-router-dom';

export const KnowledgeBase: React.FC = () => {
  const location = useLocation();
  const { data: documents, loading, execute: fetchDocs, setData: setDocs } = useApi<KnowledgeDocument[]>(knowledgeService.listDocuments);
  const [uploading, setUploading] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [archivePath, setArchivePath] = useState('/default');
  const [manualTags, setManualTags] = useState('');
  const [autoArchiveEnabled, setAutoArchiveEnabled] = useState(localStorage.getItem('crablet-auto-archive-enabled') === '1');
  const [uploadProgress, setUploadProgress] = useState(0);
  const [highlightSource, setHighlightSource] = useState('');
  const [highlightSnippet, setHighlightSnippet] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    fetchDocs().catch(() => {});
  }, [fetchDocs]);

  useEffect(() => {
    const params = new URLSearchParams(location.search);
    const q = params.get('q') || '';
    const source = params.get('source') || '';
    const snippet = params.get('snippet') || '';
    if (q) setSearchQuery(q);
    if (source) setHighlightSource(source);
    if (snippet) setHighlightSnippet(snippet);
  }, [location.search]);

  useEffect(() => {
    if (!highlightSource) return;
    const id = `kb-${encodeURIComponent(highlightSource)}`;
    const el = document.getElementById(id);
    if (el) el.scrollIntoView({ behavior: 'smooth', block: 'center' });
  }, [highlightSource, documents]);

  const handleUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      setUploading(true);
      try {
        const file = e.target.files[0];
        const validated = validateFile(file);
        if (!validated.ok) throw new Error(validated.reason || '文件校验失败');
        const safe = await heuristicSecurityScan(file);
        if (!safe.safe) throw new Error(safe.reason || '文件安全扫描失败');
        const hash = await computeFileHash(file);
        const tags = [...extractTagsByName(file.name), ...manualTags.split(',').map((t) => t.trim()).filter(Boolean)];
        await knowledgeService.uploadFile(file, {
          tags,
          archivePath,
          autoRule: autoArchiveEnabled ? 'auto-rule-enabled' : 'manual',
          onProgress: setUploadProgress,
        });
        archiveIndexService.upsert(hash, file.name, validated.category, tags);
        toast.success('File uploaded successfully');
        fetchDocs(); // Refresh list
      } catch (err) {
        console.error(err); toast.error(err instanceof Error ? err.message : 'Failed to upload file');
      } finally {
        setUploading(false);
        setUploadProgress(0);
        // Reset input
        if (fileInputRef.current) fileInputRef.current.value = '';
      }
    }
  };

  const handleDelete = async () => {
    if (!deleteId) return;
    try {
      await knowledgeService.deleteDocument(deleteId);
      toast.success('Document deleted');
      setDocs(documents?.filter(d => d.id !== deleteId));
    } catch (err: any) {
      if (err?.message === 'KNOWLEDGE_DELETE_UNSUPPORTED') {
        toast.error('当前后端版本暂不支持删除文档，请稍后升级后端。');
      } else {
        toast.error('Failed to delete document');
      }
    }
    setDeleteId(null);
  };

  const safeDocuments = Array.isArray(documents) ? documents : [];

  const filteredDocs = safeDocuments.filter(d => 
    d.source.toLowerCase().includes(searchQuery.toLowerCase()) || 
    d.content_preview.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const renderPreview = (text: string) => {
    if (!highlightSnippet) return text;
    const source = text || '';
    const q = highlightSnippet.trim();
    if (!q) return source;
    const idx = source.toLowerCase().indexOf(q.toLowerCase());
    if (idx < 0) return source;
    const end = idx + q.length;
    return (
      <>
        {source.slice(0, idx)}
        <mark className="bg-yellow-200 dark:bg-yellow-700/60 px-0.5 rounded">{source.slice(idx, end)}</mark>
        {source.slice(end)}
      </>
    );
  };

  return (
    <div className="h-full p-6 overflow-y-auto bg-gray-50 dark:bg-gray-900">
      <div className="flex justify-between items-center mb-6">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100 flex items-center gap-2">
            <Database className="w-6 h-6" />
            Knowledge Base
        </h1>
        <div className="flex gap-2">
            <Input 
                placeholder="Search..." 
                className="w-64"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
            />
            <div className="relative">
                <input 
                    type="file" 
                    ref={fileInputRef}
                    className="hidden" 
                    onChange={handleUpload}
                    disabled={uploading}
                />
                <Button variant="primary" loading={uploading} onClick={() => fileInputRef.current?.click()}>
                    <Upload className="w-4 h-4 mr-2" />
                    Upload
                </Button>
            </div>
        </div>
        <div className="mt-3 grid grid-cols-1 md:grid-cols-4 gap-2">
            <Input placeholder="归档路径，如 /finance/2026" value={archivePath} onChange={(e) => setArchivePath(e.target.value)} />
            <Input placeholder="标签，逗号分隔" value={manualTags} onChange={(e) => setManualTags(e.target.value)} />
            <Button variant={autoArchiveEnabled ? 'primary' : 'secondary'} onClick={() => {
                const next = !autoArchiveEnabled;
                setAutoArchiveEnabled(next);
                localStorage.setItem('crablet-auto-archive-enabled', next ? '1' : '0');
            }}>
                {autoArchiveEnabled ? '自动归档：开' : '自动归档：关'}
            </Button>
            <div className="text-xs text-gray-500 flex items-center justify-end">{uploading ? `上传进度 ${uploadProgress}%` : '支持多格式文档/图像/音视频'}</div>
        </div>
      </div>

      {loading && !documents ? (
        <div className="flex justify-center p-10">
          <Loader2 className="w-8 h-8 animate-spin text-blue-500" />
        </div>
      ) : filteredDocs?.length === 0 ? (
        <EmptyState 
            title={searchQuery ? "No matching documents" : "Knowledge base is empty"} 
            description={searchQuery ? "Try a different search term" : "Upload documents to make them searchable."}
            icon={<Database className="w-12 h-12 text-gray-300" />}
        />
      ) : (
        <div className="grid grid-cols-1 gap-4">
          {highlightSnippet && (
            <div className="rounded-lg border border-yellow-300 dark:border-yellow-700 bg-yellow-50 dark:bg-yellow-900/20 p-3 text-xs text-yellow-800 dark:text-yellow-200">
              当前片段定位：{highlightSnippet}
            </div>
          )}
          {filteredDocs?.map((doc) => (
            <Card
              key={doc.id}
              id={`kb-${encodeURIComponent(doc.source)}`}
              className={`hover:shadow-md transition-shadow ${highlightSource && doc.source.includes(highlightSource) ? 'ring-2 ring-blue-500' : ''} ${highlightSnippet && doc.content_preview.toLowerCase().includes(highlightSnippet.toLowerCase()) ? 'ring-2 ring-yellow-500' : ''}`}
            >
              <div className="flex items-center p-4">
                  <div className="h-10 w-10 rounded-full bg-blue-100 dark:bg-blue-900 flex items-center justify-center mr-4">
                      <FileText className="w-5 h-5 text-blue-600 dark:text-blue-400" />
                  </div>
                  <div className="flex-1 min-w-0">
                      <h3 className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
                          {doc.source}
                      </h3>
                      <p className="text-xs text-gray-500 dark:text-gray-400">
                          {format(new Date(doc.timestamp), 'MMM d, yyyy HH:mm')} • {doc.type}
                      </p>
                      <p className="text-xs text-gray-400 mt-1 truncate max-w-2xl">
                          {renderPreview(doc.content_preview)}
                      </p>
                  </div>
                  <div className="flex items-center gap-2">
                      <Button variant="ghost" size="icon" onClick={() => setDeleteId(doc.id)}>
                          <Trash2 className="w-4 h-4 text-red-500" />
                      </Button>
                  </div>
              </div>
            </Card>
          ))}
        </div>
      )}

      <ConfirmDialog 
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
        title="Delete Document"
        description="Are you sure you want to remove this document from the knowledge base? This action cannot be undone."
        variant="danger"
      />
    </div>
  );
};
