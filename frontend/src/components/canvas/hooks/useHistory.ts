/**
 * Canvas History Hook - 撤销/重做系统
 * 使用命令模式和快照机制
 */

import { useCallback, useRef, useEffect, useState } from 'react';
import type { Node, Edge } from '@xyflow/react';

export interface CanvasSnapshot {
  nodes: Node[];
  edges: Edge[];
  timestamp: number;
  description?: string;
}

export interface UseHistoryOptions {
  maxHistory?: number;
  debounceMs?: number;
}

export interface UseHistoryReturn {
  past: CanvasSnapshot[];
  future: CanvasSnapshot[];
  canUndo: boolean;
  canRedo: boolean;
  undo: () => CanvasSnapshot | undefined;
  redo: () => CanvasSnapshot | undefined;
  pushSnapshot: (snapshot: CanvasSnapshot) => void;
  clearHistory: () => void;
}

/**
 * 创建快照
 */
export function createSnapshot(nodes: Node[], edges: Edge[], description?: string): CanvasSnapshot {
  return {
    nodes: JSON.parse(JSON.stringify(nodes)),
    edges: JSON.parse(JSON.stringify(edges)),
    timestamp: Date.now(),
    description,
  };
}

/**
 * 深度比较两个状态是否相同
 */
function isEqual(nodesA: Node[], edgesA: Edge[], nodesB: Node[], edgesB: Edge[]): boolean {
  if (nodesA.length !== nodesB.length || edgesA.length !== edgesB.length) {
    return false;
  }
  
  const nodeMapA = new Map(nodesA.map(n => [n.id, n]));
  const nodeMapB = new Map(nodesB.map(n => [n.id, n]));
  
  for (const [id, nodeA] of nodeMapA) {
    const nodeB = nodeMapB.get(id);
    if (!nodeB) return false;
    if (JSON.stringify(nodeA.position) !== JSON.stringify(nodeB.position)) return false;
    if (JSON.stringify(nodeA.data) !== JSON.stringify(nodeB.data)) return false;
  }
  
  return true;
}

/**
 * Canvas History Hook
 */
export function useHistory(options: UseHistoryOptions = {}): UseHistoryReturn {
  const { maxHistory = 50, debounceMs = 500 } = options;
  
  // 使用 ref 存储状态以避免不必要的重渲染
  const pastRef = useRef<CanvasSnapshot[]>([]);
  const futureRef = useRef<CanvasSnapshot[]>([]);
  const lastSnapshotRef = useRef<string>('');
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isUndoRedoRef = useRef(false);
  
  // 强制更新的 trigger
  const [, setTick] = useState(0);
  
  const forceUpdate = useCallback(() => {
    setTick(t => t + 1);
  }, []);

  /**
   * 推入新快照 (带防抖)
   */
  const pushSnapshot = useCallback((snapshot: CanvasSnapshot) => {
    // 如果是撤销/重做操作，不记录历史
    if (isUndoRedoRef.current) {
      isUndoRedoRef.current = false;
      return;
    }
    
    // 检查是否与上一个快照相同
    const currentKey = JSON.stringify({
      nodes: snapshot.nodes,
      edges: snapshot.edges
    });
    
    if (currentKey === lastSnapshotRef.current) {
      return;
    }
    
    lastSnapshotRef.current = currentKey;
    
    // 添加到历史
    pastRef.current = [...pastRef.current, snapshot].slice(-maxHistory);
    futureRef.current = []; // 新的操作清除未来历史
    
    forceUpdate();
  }, [maxHistory, forceUpdate]);

  /**
   * 撤销
   */
  const undo = useCallback((): CanvasSnapshot | undefined => {
    if (pastRef.current.length === 0) return undefined;
    
    isUndoRedoRef.current = true;
    
    // 保存当前状态到未来
    const current = createSnapshot(
      pastRef.current.length > 0 ? pastRef.current[pastRef.current.length - 1].nodes : [],
      pastRef.current.length > 0 ? pastRef.current[pastRef.current.length - 1].edges : [],
      'Current state'
    );
    futureRef.current = [current, ...futureRef.current].slice(0, maxHistory);
    
    // 恢复上一个状态
    const previous = pastRef.current[pastRef.current.length - 1];
    pastRef.current = pastRef.current.slice(0, -1);
    
    lastSnapshotRef.current = JSON.stringify({
      nodes: previous.nodes,
      edges: previous.edges
    });
    
    forceUpdate();
    return previous;
  }, [maxHistory, forceUpdate]);

  /**
   * 重做
   */
  const redo = useCallback((): CanvasSnapshot | undefined => {
    if (futureRef.current.length === 0) return undefined;
    
    isUndoRedoRef.current = true;
    
    // 保存当前状态到历史
    const current = createSnapshot(
      pastRef.current.length > 0 ? pastRef.current[pastRef.current.length - 1].nodes : [],
      pastRef.current.length > 0 ? pastRef.current[pastRef.current.length - 1].edges : [],
      'Current state'
    );
    pastRef.current = [...pastRef.current, current].slice(-maxHistory);
    
    // 恢复未来状态
    const next = futureRef.current[0];
    futureRef.current = futureRef.current.slice(1);
    
    lastSnapshotRef.current = JSON.stringify({
      nodes: next.nodes,
      edges: next.edges
    });
    
    forceUpdate();
    return next;
  }, [maxHistory, forceUpdate]);

  /**
   * 清除历史
   */
  const clearHistory = useCallback(() => {
    pastRef.current = [];
    futureRef.current = [];
    lastSnapshotRef.current = '';
    forceUpdate();
  }, [forceUpdate]);

  return {
    past: pastRef.current,
    future: futureRef.current,
    canUndo: pastRef.current.length > 0,
    canRedo: futureRef.current.length > 0,
    undo,
    redo,
    pushSnapshot,
    clearHistory,
  };
}

export default useHistory;
