/**
 * Canvas Toolbar - 增强的工具栏
 * 提供撤销/重做、复制/粘贴、批量操作等按钮
 */

import React, { useCallback } from 'react';
import {
  Undo2,
  Redo2,
  Copy,
  Scissors,
  Clipboard,
  Trash2,
  Layers,
  ZoomIn,
  ZoomOut,
  Maximize2,
  Save,
  FolderOpen,
  Download,
  Upload,
  Square,
} from 'lucide-react';
import { Button } from '../../ui/Button';

export interface CanvasToolbarProps {
  // 历史控制
  canUndo: boolean;
  canRedo: boolean;
  onUndo: () => void;
  onRedo: () => void;
  
  // 编辑控制
  canPaste: boolean;
  onCopy: () => void;
  onCut: () => void;
  onPaste: () => void;
  onDelete: () => void;
  onSelectAll: () => void;
  
  // 视图控制
  zoom: number;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onFitView: () => void;
  onToggleMinimap: () => void;
  
  // 文件操作
  onSave: () => void;
  onLoad: () => void;
  onExport: () => void;
  onImport: () => void;
  
  // 其他
  className?: string;
}

export const CanvasToolbar: React.FC<CanvasToolbarProps> = ({
  canUndo,
  canRedo,
  onUndo,
  onRedo,
  canPaste,
  onCopy,
  onCut,
  onPaste,
  onDelete,
  onSelectAll,
  zoom,
  onZoomIn,
  onZoomOut,
  onFitView,
  onToggleMinimap,
  onSave,
  onLoad,
  onExport,
  onImport,
  className = '',
}) => {
  return (
    <div className={`canvas-toolbar flex items-center gap-1 p-2 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 ${className}`}>
      {/* 历史控制组 */}
      <div className="flex items-center gap-1 pr-2 border-r border-gray-200 dark:border-gray-700">
        <Button
          size="sm"
          variant="ghost"
          onClick={onUndo}
          disabled={!canUndo}
          title="撤销 (Ctrl+Z)"
        >
          <Undo2 className="w-4 h-4" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={onRedo}
          disabled={!canRedo}
          title="重做 (Ctrl+Shift+Z)"
        >
          <Redo2 className="w-4 h-4" />
        </Button>
      </div>
      
      {/* 编辑控制组 */}
      <div className="flex items-center gap-1 pr-2 border-r border-gray-200 dark:border-gray-700">
        <Button
          size="sm"
          variant="ghost"
          onClick={onCopy}
          title="复制 (Ctrl+C)"
        >
          <Copy className="w-4 h-4" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={onCut}
          title="剪切 (Ctrl+X)"
        >
          <Scissors className="w-4 h-4" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={onPaste}
          disabled={!canPaste}
          title="粘贴 (Ctrl+V)"
        >
          <Clipboard className="w-4 h-4" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={onDelete}
          title="删除 (Delete)"
        >
          <Trash2 className="w-4 h-4" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={onSelectAll}
          title="全选 (Ctrl+A)"
        >
          <Square className="w-4 h-4" />
        </Button>
      </div>
      
      {/* 视图控制组 */}
      <div className="flex items-center gap-1 pr-2 border-r border-gray-200 dark:border-gray-700">
        <Button
          size="sm"
          variant="ghost"
          onClick={onZoomOut}
          title="缩小"
        >
          <ZoomOut className="w-4 h-4" />
        </Button>
        <span className="text-xs text-gray-500 dark:text-gray-400 min-w-[50px] text-center">
          {Math.round(zoom * 100)}%
        </span>
        <Button
          size="sm"
          variant="ghost"
          onClick={onZoomIn}
          title="放大"
        >
          <ZoomIn className="w-4 h-4" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={onFitView}
          title="适应视图"
        >
          <Maximize2 className="w-4 h-4" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={onToggleMinimap}
          title="切换小地图"
        >
          <Layers className="w-4 h-4" />
        </Button>
      </div>
      
      {/* 文件操作组 */}
      <div className="flex items-center gap-1">
        <Button
          size="sm"
          variant="ghost"
          onClick={onSave}
          title="保存"
        >
          <Save className="w-4 h-4" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={onLoad}
          title="加载"
        >
          <FolderOpen className="w-4 h-4" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={onExport}
          title="导出"
        >
          <Download className="w-4 h-4" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={onImport}
          title="导入"
        >
          <Upload className="w-4 h-4" />
        </Button>
      </div>
    </div>
  );
};

export default CanvasToolbar;