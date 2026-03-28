/**
 * Canvas State Hook - 统一的画布状态管理
 * 整合所有增强功能的状态管理
 */

import { useCallback, useState, useEffect, useRef } from 'react';
import type { Node, Edge, OnNodesChange, OnEdgesChange, Connection } from '@xyflow/react';
import { applyNodeChanges, applyEdgeChanges, addEdge as addEdgeToGraph } from '@xyflow/react';
import { createSnapshot, type CanvasSnapshot } from './useHistory';
import { useClipboard, type ClipboardData } from './useClipboard';

export interface CanvasStateOptions {
  maxHistory?: number;
  onNodesChange?: OnNodesChange;
  onEdgesChange?: OnEdgesChange;
}

export interface UseCanvasStateReturn {
  // 状态
  nodes: Node[];
  edges: Edge[];
  selectedNodes: Node[];
  selectedEdges: Edge[];
  canUndo: boolean;
  canRedo: boolean;
  canPaste: boolean;
  
  // 状态变更
  setNodes: (nodes: Node[]) => void;
  setEdges: (edges: Edge[]) => void;
  onNodesChange: OnNodesChange;
  onEdgesChange: OnEdgesChange;
  onConnect: (connection: Connection) => void;
  
  // 编辑操作
  addNode: (node: Node) => void;
  removeNodes: (ids: string[]) => void;
  removeEdges: (ids: string[]) => void;
  updateNodeData: (id: string, data: Partial<Node['data']>) => void;
  
  // 高级操作
  copySelected: () => void;
  paste: () => ClipboardData | null;
  cutSelected: () => void;
  deleteSelected: () => void;
  selectAll: () => void;
  deselectAll: () => void;
  
  // 历史操作
  undo: () => void;
  redo: () => void;
  pushHistory: (description?: string) => void;
  
  // 工具函数
  getNode: (id: string) => Node | undefined;
  getEdge: (id: string) => Edge | undefined;
  exportWorkflow: () => string;
  importWorkflow: (json: string) => void;
}

/**
 * 统一的 Canvas 状态管理 Hook
 */
export function useCanvasState(options: CanvasStateOptions = {}): UseCanvasStateReturn {
  const { maxHistory = 50 } = options;
  
  // 内部状态
  const [nodes, setNodesInternal] = useState<Node[]>([]);
  const [edges, setEdgesInternal] = useState<Edge[]>([]);
  
  // 历史记录
  const historyRef = useRef<{
    past: CanvasSnapshot[];
    future: CanvasSnapshot[];
  }>({ past: [], future: [] });
  
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);
  
  // 剪贴板
  const clipboard = useClipboard();
  
  // 选中状态
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  
  // 历史操作
  const pushHistory = useCallback((description?: string) => {
    const snapshot = createSnapshot(nodes, edges, description);
    historyRef.current.past = [...historyRef.current.past, snapshot].slice(-maxHistory);
    historyRef.current.future = [];
    setCanUndo(historyRef.current.past.length > 0);
    setCanRedo(false);
  }, [nodes, edges, maxHistory]);
  
  const undo = useCallback(() => {
    if (historyRef.current.past.length === 0) return;
    
    const current = createSnapshot(nodes, edges, 'current');
    historyRef.current.future = [current, ...historyRef.current.future];
    
    const previous = historyRef.current.past[historyRef.current.past.length - 1];
    historyRef.current.past = historyRef.current.past.slice(0, -1);
    
    setNodesInternal(previous.nodes);
    setEdgesInternal(previous.edges);
    setCanUndo(historyRef.current.past.length > 0);
    setCanRedo(true);
  }, [nodes, edges]);
  
  const redo = useCallback(() => {
    if (historyRef.current.future.length === 0) return;
    
    const current = createSnapshot(nodes, edges, 'current');
    historyRef.current.past = [...historyRef.current.past, current];
    
    const next = historyRef.current.future[0];
    historyRef.current.future = historyRef.current.future.slice(1);
    
    setNodesInternal(next.nodes);
    setEdgesInternal(next.edges);
    setCanUndo(true);
    setCanRedo(historyRef.current.future.length > 0);
  }, [nodes, edges]);
  
  // 节点变更处理
  const handleNodesChange: OnNodesChange = useCallback((changes) => {
    // 记录历史 (仅在移动和删除时)
    const hasMoveOrRemove = changes.some(c => 
      c.type === 'position' || c.type === 'remove'
    );
    
    if (hasMoveOrRemove) {
      // 防抖处理在外部
    }
    
    setNodesInternal(prev => applyNodeChanges(changes, prev));
  }, []);
  
  // 边变更处理
  const handleEdgesChange: OnEdgesChange = useCallback((changes) => {
    setEdgesInternal(prev => applyEdgeChanges(changes, prev));
  }, []);
  
  // 连接处理
  const onConnect = useCallback((connection: Connection) => {
    setEdgesInternal(prev => addEdgeToGraph(connection, prev));
    pushHistory('Add connection');
  }, [pushHistory]);
  
  // 设置节点
  const setNodes = useCallback((newNodes: Node[]) => {
    setNodesInternal(newNodes);
  }, []);
  
  // 设置边
  const setEdges = useCallback((newEdges: Edge[]) => {
    setEdgesInternal(newEdges);
  }, []);
  
  // 添加节点
  const addNode = useCallback((node: Node) => {
    setNodesInternal(prev => [...prev, node]);
    pushHistory('Add node');
  }, [pushHistory]);
  
  // 移除节点
  const removeNodes = useCallback((ids: string[]) => {
    setNodesInternal(prev => prev.filter(n => !ids.includes(n.id)));
    setEdgesInternal(prev => prev.filter(e => !ids.includes(e.source) && !ids.includes(e.target)));
    pushHistory('Remove nodes');
  }, [pushHistory]);
  
  // 移除边
  const removeEdges = useCallback((ids: string[]) => {
    setEdgesInternal(prev => prev.filter(e => !ids.includes(e.id)));
    pushHistory('Remove edges');
  }, [pushHistory]);
  
  // 更新节点数据
  const updateNodeData = useCallback((id: string, data: Partial<Node['data']>) => {
    setNodesInternal(prev => prev.map(n => 
      n.id === id ? { ...n, data: { ...n.data, ...data } } : n
    ));
  }, []);
  
  // 复制选中
  const copySelected = useCallback(() => {
    const selected = nodes.filter(n => selectedIds.has(n.id));
    const selectedEdges = edges.filter(e => 
      selectedIds.has(e.source) && selectedIds.has(e.target)
    );
    clipboard.copy(selected, selectedEdges);
  }, [nodes, edges, selectedIds, clipboard]);
  
  // 粘贴
  const paste = useCallback(() => {
    const data = clipboard.paste();
    if (data) {
      setNodesInternal(prev => [...prev, ...data.nodes]);
      setEdgesInternal(prev => [...prev, ...data.edges]);
      setSelectedIds(new Set(data.nodes.map(n => n.id)));
      pushHistory('Paste');
    }
    return data;
  }, [clipboard, pushHistory]);
  
  // 剪切选中
  const cutSelected = useCallback(() => {
    const selected = nodes.filter(n => selectedIds.has(n.id));
    const selectedEdges = edges.filter(e => 
      selectedIds.has(e.source) && selectedIds.has(e.target)
    );
    clipboard.cut(selected, selectedEdges, removeNodes);
    setSelectedIds(new Set());
    pushHistory('Cut');
  }, [nodes, edges, selectedIds, clipboard, removeNodes, pushHistory]);
  
  // 删除选中
  const deleteSelected = useCallback(() => {
    const nodeIds = Array.from(selectedIds);
    const edgeIds = edges
      .filter(e => selectedIds.has(e.source) && selectedIds.has(e.target))
      .map(e => e.id);
    
    removeNodes(nodeIds);
    removeEdges(edgeIds);
    setSelectedIds(new Set());
  }, [selectedIds, edges, removeNodes, removeEdges]);
  
  // 全选
  const selectAll = useCallback(() => {
    setSelectedIds(new Set(nodes.map(n => n.id)));
  }, [nodes]);
  
  // 取消选择
  const deselectAll = useCallback(() => {
    setSelectedIds(new Set());
  }, []);
  
  // 获取节点/边
  const getNode = useCallback((id: string) => nodes.find(n => n.id === id), [nodes]);
  const getEdge = useCallback((id: string) => edges.find(e => e.id === id), [edges]);
  
  // 导出工作流
  const exportWorkflow = useCallback(() => {
    return JSON.stringify({ nodes, edges }, null, 2);
  }, [nodes, edges]);
  
  // 导入工作流
  const importWorkflow = useCallback((json: string) => {
    try {
      const data = JSON.parse(json);
      setNodesInternal(data.nodes || []);
      setEdgesInternal(data.edges || []);
      setSelectedIds(new Set());
      historyRef.current = { past: [], future: [] };
      setCanUndo(false);
      setCanRedo(false);
    } catch (e) {
      console.error('Failed to import workflow:', e);
    }
  }, []);
  
  // 计算选中节点和边
  const selectedNodes = nodes.filter(n => selectedIds.has(n.id));
  const selectedEdges = edges.filter(e => selectedIds.has(e.source) && selectedIds.has(e.target));
  
  // 键盘快捷键
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return;
      }
      
      const isCtrl = e.ctrlKey || e.metaKey;
      
      if (isCtrl && e.key === 'z' && !e.shiftKey) {
        e.preventDefault();
        undo();
      } else if (isCtrl && (e.key === 'Z' || e.key === 'y')) {
        e.preventDefault();
        redo();
      } else if (isCtrl && e.key === 'c') {
        copySelected();
      } else if (isCtrl && e.key === 'x') {
        cutSelected();
      } else if (isCtrl && e.key === 'v') {
        paste();
      } else if (isCtrl && e.key === 'a') {
        e.preventDefault();
        selectAll();
      } else if (e.key === 'Delete' || e.key === 'Backspace') {
        if (selectedIds.size > 0) {
          e.preventDefault();
          deleteSelected();
        }
      } else if (e.key === 'Escape') {
        deselectAll();
      }
    };
    
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [undo, redo, copySelected, cutSelected, paste, selectAll, deleteSelected, deselectAll, selectedIds]);
  
  return {
    nodes,
    edges,
    selectedNodes,
    selectedEdges,
    canUndo,
    canRedo,
    canPaste: clipboard.canPaste,
    setNodes,
    setEdges,
    onNodesChange: handleNodesChange,
    onEdgesChange: handleEdgesChange,
    onConnect,
    addNode,
    removeNodes,
    removeEdges,
    updateNodeData,
    copySelected,
    paste,
    cutSelected,
    deleteSelected,
    selectAll,
    deselectAll,
    undo,
    redo,
    pushHistory,
    getNode,
    getEdge,
    exportWorkflow,
    importWorkflow,
  };
}

export default useCanvasState;