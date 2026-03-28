/**
 * Canvas Selection Hook - 多选和框选功能
 * 简化版：直接操作外部的 nodes/edges
 */

import { useCallback, useState, useMemo } from 'react';
import type { Node, Edge } from '@xyflow/react';

export interface UseSelectionOptions {
  nodes: Node[];
  edges: Edge[];
  onChange?: (newNodes: Node[], newEdges: Edge[]) => void;
}

export interface UseSelectionReturn {
  selectedIds: Set<string>;
  isMultiSelect: boolean;
  // 操作方法
  selectAll: () => void;
  clearSelection: () => void;
  deleteSelected: () => void;
}

export function useSelection(options: UseSelectionOptions): UseSelectionReturn {
  const { nodes, edges, onChange } = options;
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  // 全选
  const selectAll = useCallback(() => {
    setSelectedIds(new Set(nodes.map(n => n.id)));
  }, [nodes]);

  // 清除选择
  const clearSelection = useCallback(() => {
    setSelectedIds(new Set());
  }, []);

  // 删除选中
  const deleteSelected = useCallback(() => {
    if (selectedIds.size === 0) return;

    // 删除选中的节点
    const newNodes = nodes.filter(n => !selectedIds.has(n.id));
    // 删除与选中节点相关的边
    const newEdges = edges.filter(e => 
      !selectedIds.has(e.source) && !selectedIds.has(e.target)
    );

    // 清除选择
    setSelectedIds(new Set());

    // 通知变化
    onChange?.(newNodes, newEdges);
  }, [nodes, edges, selectedIds, onChange]);

  const isMultiSelect = selectedIds.size > 1;

  return {
    selectedIds,
    isMultiSelect,
    selectAll,
    clearSelection,
    deleteSelected,
  };
}

export default useSelection;
