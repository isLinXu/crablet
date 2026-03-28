/**
 * Version History Panel - 版本历史面板
 * 提供版本保存、恢复、对比功能
 */

import React, { useState } from 'react';
import type { Node, Edge } from '@xyflow/react';
import {
  History,
  RotateCcw,
  Download,
  Upload,
  Trash2,
  GitCompare,
  Save,
  Clock,
  X,
} from 'lucide-react';
import { Button } from '../../ui/Button';
import { useVersionHistory, type WorkflowVersion, type VersionDiff } from '../hooks/useVersionHistory';

interface VersionHistoryPanelProps {
  nodes: Node[];
  edges: Edge[];
  onRestore: (nodes: Node[], edges: Edge[]) => void;
  onClose: () => void;
}

export const VersionHistoryPanel: React.FC<VersionHistoryPanelProps> = ({
  nodes,
  edges,
  onRestore,
  onClose,
}) => {
  const [saveName, setSaveName] = useState('');
  const [saveDescription, setSaveDescription] = useState('');
  const [compareMode, setCompareMode] = useState(false);
  const [compareV1, setCompareV1] = useState<string | null>(null);
  const [compareV2, setCompareV2] = useState<string | null>(null);

  const {
    versions,
    currentVersionId,
    isAutoSaving,
    saveVersion,
    loadVersion,
    deleteVersion,
    exportVersion,
    importVersion,
    getVersionDiff,
  } = useVersionHistory(nodes, edges);

  const [diff, setDiff] = useState<VersionDiff | null>(null);

  const handleSave = () => {
    if (!saveName.trim()) return;
    saveVersion(saveName, saveDescription);
    setSaveName('');
    setSaveDescription('');
  };

  const handleRestore = (id: string) => {
    const data = loadVersion(id);
    if (data) {
      onRestore(data.nodes, data.edges);
    }
  };

  const handleExport = (id: string) => {
    const json = exportVersion(id);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `workflow-${id}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleImport = () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (file) {
        const reader = new FileReader();
        reader.onload = (ev) => {
          if (ev.target?.result) {
            importVersion(ev.target.result as string);
          }
        };
        reader.readAsText(file);
      }
    };
    input.click();
  };

  const handleCompare = () => {
    if (compareV1 && compareV2) {
      const result = getVersionDiff(compareV1, compareV2);
      setDiff(result);
      setCompareMode(false);
    }
  };

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - timestamp;
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return '刚刚';
    if (diffMins < 60) return `${diffMins} 分钟前`;
    if (diffHours < 24) return `${diffHours} 小时前`;
    if (diffDays < 7) return `${diffDays} 天前`;
    return date.toLocaleDateString();
  };

  return (
    <div className="w-80 bg-white dark:bg-gray-800 border-l border-gray-200 dark:border-gray-700 flex flex-col h-full">
      {/* Header */}
      <div className="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <History className="w-5 h-5 text-gray-700 dark:text-gray-300" />
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
            版本历史
          </h2>
          {isAutoSaving && (
            <span className="text-xs text-gray-500">(自动保存中...)</span>
          )}
        </div>
        <button
          onClick={onClose}
          className="p-1 hover:bg-gray-100 dark:hover:bg-gray-700 rounded"
        >
          <X className="w-5 h-5 text-gray-500" />
        </button>
      </div>

      {/* Save Version */}
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
          保存新版本
        </h3>
        <div className="space-y-2">
          <input
            type="text"
            value={saveName}
            onChange={(e) => setSaveName(e.target.value)}
            placeholder="版本名称"
            className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          />
          <input
            type="text"
            value={saveDescription}
            onChange={(e) => setSaveDescription(e.target.value)}
            placeholder="描述 (可选)"
            className="w-full px-3 py-2 text-sm border border-gray-200 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
          />
          <Button onClick={handleSave} className="w-full" size="sm">
            <Save className="w-4 h-4 mr-2" />
            保存版本
          </Button>
        </div>
      </div>

      {/* Import/Export */}
      <div className="p-4 border-b border-gray-200 dark:border-gray-700 flex gap-2">
        <Button onClick={handleImport} variant="secondary" size="sm" className="flex-1">
          <Upload className="w-4 h-4 mr-1" />
          导入
        </Button>
        <Button
          onClick={() => handleExport(currentVersionId || '')}
          variant="secondary"
          size="sm"
          className="flex-1"
          disabled={!currentVersionId}
        >
          <Download className="w-4 h-4 mr-1" />
          导出
        </Button>
        <Button
          onClick={() => setCompareMode(true)}
          variant="secondary"
          size="sm"
          disabled={versions.length < 2}
        >
          <GitCompare className="w-4 h-4" />
        </Button>
      </div>

      {/* Compare Mode */}
      {compareMode && (
        <div className="p-4 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-700/50">
          <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            选择版本对比
          </h3>
          <div className="flex gap-2 mb-2">
            <select
              value={compareV1 || ''}
              onChange={(e) => setCompareV1(e.target.value)}
              className="flex-1 px-2 py-1 text-sm border rounded"
            >
              <option value="">选择版本1</option>
              {versions.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.name}
                </option>
              ))}
            </select>
            <select
              value={compareV2 || ''}
              onChange={(e) => setCompareV2(e.target.value)}
              className="flex-1 px-2 py-1 text-sm border rounded"
            >
              <option value="">选择版本2</option>
              {versions.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.name}
                </option>
              ))}
            </select>
          </div>
          <div className="flex gap-2">
            <Button onClick={handleCompare} size="sm" className="flex-1">
              对比
            </Button>
            <Button onClick={() => setCompareMode(false)} variant="secondary" size="sm">
              取消
            </Button>
          </div>
        </div>
      )}

      {/* Diff Result */}
      {diff && (
        <div className="p-4 border-b border-gray-200 dark:border-gray-700 bg-blue-50 dark:bg-blue-900/20">
          <div className="flex items-center justify-between mb-2">
            <h3 className="text-sm font-medium text-blue-700 dark:text-blue-300">
              版本差异
            </h3>
            <button onClick={() => setDiff(null)}>
              <X className="w-4 h-4" />
            </button>
          </div>
          <div className="text-xs space-y-1">
            <p className="text-green-600 dark:text-green-400">
              + 新增节点: {diff.addedNodes.length}
            </p>
            <p className="text-red-600 dark:text-red-400">
              - 删除节点: {diff.removedNodes.length}
            </p>
            <p className="text-yellow-600 dark:text-yellow-400">
              ~ 修改节点: {diff.modifiedNodes.length}
            </p>
            <p className="text-blue-600 dark:text-blue-400">
              → 新增连接: {diff.addedEdges.length}
            </p>
          </div>
        </div>
      )}

      {/* Version List */}
      <div className="flex-1 overflow-y-auto p-2">
        {versions.length === 0 ? (
          <div className="text-center text-gray-500 py-8">
            <Clock className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p>暂无版本记录</p>
            <p className="text-xs">保存工作流以创建第一个版本</p>
          </div>
        ) : (
          <div className="space-y-2">
            {versions.map((version) => (
              <div
                key={version.id}
                className={`p-3 rounded-lg border transition-all ${
                  currentVersionId === version.id
                    ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
                    : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
                }`}
              >
                <div className="flex items-start justify-between">
                  <div className="flex-1 min-w-0">
                    <h4 className="text-sm font-medium text-gray-900 dark:text-white truncate">
                      {version.name}
                    </h4>
                    <p className="text-xs text-gray-500 mt-1">
                      {formatTime(version.timestamp)}
                    </p>
                    {version.description && (
                      <p className="text-xs text-gray-600 dark:text-gray-400 mt-1 line-clamp-2">
                        {version.description}
                      </p>
                    )}
                  </div>
                  <div className="flex items-center gap-1 ml-2">
                    <button
                      onClick={() => handleRestore(version.id)}
                      className="p-1.5 text-gray-500 hover:text-blue-600 hover:bg-blue-50 dark:hover:bg-blue-900/30 rounded"
                      title="恢复此版本"
                    >
                      <RotateCcw className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => handleExport(version.id)}
                      className="p-1.5 text-gray-500 hover:text-green-600 hover:bg-green-50 dark:hover:bg-green-900/30 rounded"
                      title="导出"
                    >
                      <Download className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => deleteVersion(version.id)}
                      className="p-1.5 text-gray-500 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-900/30 rounded"
                      title="删除"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>
                <div className="mt-2 flex items-center gap-3 text-xs text-gray-500">
                  <span>{version.nodes.length} 节点</span>
                  <span>{version.edges.length} 连接</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

export default VersionHistoryPanel;
