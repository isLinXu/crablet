/**
 * Canvas Clipboard Hook - 复制/粘贴/剪切功能
 * 简化版：直接从外部传入节点和边
 */

import { useCallback, useState, useEffect } from 'react';
import type { Node, Edge } from '@xyflow/react';

export interface ClipboardData {
  nodes: Node[];
  edges: Edge[];
  timestamp: number;
}

export interface UseClipboardReturn {
  clipboard: ClipboardData | null;
  copy: (nodes: Node[], edges: Edge[]) => void;
  cut: (nodes: Node[], edges: Edge[], onDelete: (ids: string[]) => void) => void;
  paste: (offset?: { x: number; y: number }) => ClipboardData | null;
  canPaste: boolean;
  clear: () => void;
  // 兼容属性
  hasClipboard: boolean;
}

/**
 * 复制节点和边到剪贴板
 */
export function useClipboard(): UseClipboardReturn {
  const [clipboard, setClipboard] = useState<ClipboardData | null>(null);
  
  /**
   * 复制选中的节点和相关的边
   */
  const copy = useCallback((nodes: Node[], edges: Edge[]) => {
    if (nodes.length === 0) return;
    
    const nodeIds = new Set(nodes.map(n => n.id));
    const relatedEdges = edges.filter(
      e => nodeIds.has(e.source) && nodeIds.has(e.target)
    );
    
    const data: ClipboardData = {
      nodes: JSON.parse(JSON.stringify(nodes)),
      edges: JSON.parse(JSON.stringify(relatedEdges)),
      timestamp: Date.now(),
    };
    
    setClipboard(data);
  }, []);
  
  /**
   * 剪切选中的节点和相关的边
   */
  const cut = useCallback((nodes: Node[], edges: Edge[], onDelete: (ids: string[]) => void) => {
    if (nodes.length === 0) return;
    
    // 先复制
    copy(nodes, edges);
    // 然后删除
    onDelete(nodes.map(n => n.id));
  }, [copy]);
  
  /**
   * 粘贴剪贴板内容
   */
  const paste = useCallback((offset?: { x: number; y: number }): ClipboardData | null => {
    if (!clipboard) return null;
    
    const offsetX = offset?.x ?? 50;
    const offsetY = offset?.y ?? 50;
    
    // 为节点生成新 ID
    const idMap = new Map<string, string>();
    const newNodes = clipboard.nodes.map(node => {
      const newId = `${node.type}-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
      idMap.set(node.id, newId);
      return {
        ...node,
        id: newId,
        position: {
          x: node.position.x + offsetX,
          y: node.position.y + offsetY,
        },
      };
    });
    
    // 更新边的引用
    const newEdges = clipboard.edges.map(edge => ({
      ...edge,
      id: `${edge.id}-paste-${Date.now()}`,
      source: idMap.get(edge.source) || edge.source,
      target: idMap.get(edge.target) || edge.target,
    }));
    
    return {
      nodes: newNodes,
      edges: newEdges,
      timestamp: Date.now(),
    };
  }, [clipboard]);
  
  const clear = useCallback(() => {
    setClipboard(null);
  }, []);
  
  // 同步剪贴板到 localStorage
  useEffect(() => {
    if (clipboard) {
      try {
        localStorage.setItem('canvas_clipboard', JSON.stringify(clipboard));
      } catch (e) {
        console.error('Failed to save clipboard:', e);
      }
    }
  }, [clipboard]);
  
  // 从 localStorage 恢复剪贴板
  useEffect(() => {
    try {
      const stored = localStorage.getItem('canvas_clipboard');
      if (stored) {
        setClipboard(JSON.parse(stored));
      }
    } catch (e) {
      console.error('Failed to load clipboard:', e);
    }
  }, []);
  
  return {
    clipboard,
    copy,
    cut,
    paste,
    canPaste: !!clipboard,
    clear,
    hasClipboard: !!clipboard,
  };
}

export default useClipboard;
