/**
 * Canvas Version History Hook - 版本历史管理
 * 支持保存、恢复、对比工作流版本
 */

import { useCallback, useRef, useState, useEffect } from 'react';
import type { Node, Edge } from '@xyflow/react';

export interface WorkflowVersion {
  id: string;
  name: string;
  nodes: Node[];
  edges: Edge[];
  timestamp: number;
  description?: string;
}

export interface VersionHistoryOptions {
  maxVersions?: number;
  storageKey?: string;
  autoSave?: boolean;
  autoSaveInterval?: number;
}

export interface UseVersionHistoryReturn {
  versions: WorkflowVersion[];
  currentVersionId: string | null;
  isAutoSaving: boolean;
  saveVersion: (name: string, description?: string) => void;
  loadVersion: (id: string) => { nodes: Node[]; edges: Edge[] } | null;
  deleteVersion: (id: string) => void;
  exportVersion: (id: string) => string;
  importVersion: (json: string) => boolean;
  getVersionDiff: (id1: string, id2: string) => VersionDiff;
}

export interface VersionDiff {
  addedNodes: string[];
  removedNodes: string[];
  modifiedNodes: string[];
  addedEdges: string[];
  removedEdges: string[];
  modifiedEdges: string[];
}

/**
 * 创建版本快照
 */
export function createVersion(
  nodes: Node[],
  edges: Edge[],
  name: string,
  description?: string
): WorkflowVersion {
  return {
    id: `v_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
    name,
    nodes: JSON.parse(JSON.stringify(nodes)),
    edges: JSON.parse(JSON.stringify(edges)),
    timestamp: Date.now(),
    description,
  };
}

/**
 * 计算版本差异
 */
export function computeDiff(
  v1: WorkflowVersion,
  v2: WorkflowVersion
): VersionDiff {
  const v1Nodes = new Set(v1.nodes.map((n) => n.id));
  const v2Nodes = new Set(v2.nodes.map((n) => n.id));
  const v1Edges = new Set(v1.edges.map((e) => e.id));
  const v2Edges = new Set(v2.edges.map((e) => e.id));

  return {
    addedNodes: v2.nodes.filter((n) => !v1Nodes.has(n.id)).map((n) => n.id),
    removedNodes: v1.nodes.filter((n) => !v2Nodes.has(n.id)).map((n) => n.id),
    modifiedNodes: v2.nodes
      .filter((n) => {
        const old = v1.nodes.find((o) => o.id === n.id);
        if (!old) return false;
        return JSON.stringify(old.data) !== JSON.stringify(n.data);
      })
      .map((n) => n.id),
    addedEdges: v2.edges.filter((e) => !v1Edges.has(e.id)).map((e) => e.id),
    removedEdges: v1.edges.filter((e) => !v2Edges.has(e.id)).map((e) => e.id),
    modifiedEdges: v2.edges
      .filter((e) => {
        const old = v1.edges.find((o) => o.id === e.id);
        if (!old) return false;
        return old.source !== e.source || old.target !== e.target;
      })
      .map((e) => e.id),
  };
}

export function useVersionHistory(
  nodes: Node[],
  edges: Edge[],
  options: VersionHistoryOptions = {}
): UseVersionHistoryReturn {
  const {
    maxVersions = 50,
    storageKey = 'canvas_version_history',
  } = options;

  const [versions, setVersions] = useState<WorkflowVersion[]>([]);
  const [currentVersionId, setCurrentVersionId] = useState<string | null>(null);
  const [isAutoSaving, setIsAutoSaving] = useState(false);
  
  const nodesRef = useRef<Node[]>(nodes);
  const edgesRef = useRef<Edge[]>(edges);
  const lastSavedRef = useRef<number>(Date.now());

  // Update refs when values change
  useEffect(() => {
    nodesRef.current = nodes;
    edgesRef.current = edges;
  }, [nodes, edges]);

  // Load versions from localStorage
  useEffect(() => {
    try {
      const stored = localStorage.getItem(storageKey);
      if (stored) {
        const parsed = JSON.parse(stored);
        setVersions(parsed.versions || []);
        setCurrentVersionId(parsed.currentId || null);
      }
    } catch (e) {
      console.error('Failed to load version history:', e);
    }
  }, [storageKey]);

  // Save versions to localStorage
  const saveToStorage = useCallback(
    (newVersions: WorkflowVersion[], currentId: string | null) => {
      try {
        localStorage.setItem(
          storageKey,
          JSON.stringify({ versions: newVersions, currentId })
        );
      } catch (e) {
        console.error('Failed to save version history:', e);
      }
    },
    [storageKey]
  );

  // Save a new version
  const saveVersion = useCallback(
    (name: string, description?: string) => {
      const version = createVersion(
        nodesRef.current,
        edgesRef.current,
        name,
        description
      );

      setVersions((prev) => {
        const newVersions = [version, ...prev].slice(0, maxVersions);
        saveToStorage(newVersions, version.id);
        setCurrentVersionId(version.id);
        lastSavedRef.current = Date.now();
        return newVersions;
      });
    },
    [maxVersions, saveToStorage]
  );

  // Load a specific version
  const loadVersion = useCallback(
    (id: string): { nodes: Node[]; edges: Edge[] } | null => {
      const version = versions.find((v) => v.id === id);
      if (!version) return null;

      setCurrentVersionId(id);
      lastSavedRef.current = Date.now();
      return {
        nodes: JSON.parse(JSON.stringify(version.nodes)),
        edges: JSON.parse(JSON.stringify(version.edges)),
      };
    },
    [versions]
  );

  // Delete a version
  const deleteVersion = useCallback(
    (id: string) => {
      setVersions((prev) => {
        const newVersions = prev.filter((v) => v.id !== id);
        saveToStorage(newVersions, currentVersionId);
        if (currentVersionId === id) {
          setCurrentVersionId(newVersions[0]?.id || null);
        }
        return newVersions;
      });
    },
    [currentVersionId, saveToStorage]
  );

  // Export version as JSON
  const exportVersion = useCallback(
    (id: string): string => {
      const version = versions.find((v) => v.id === id);
      if (!version) return '';
      return JSON.stringify(
        { nodes: version.nodes, edges: version.edges },
        null,
        2
      );
    },
    [versions]
  );

  // Import version from JSON
  const importVersion = useCallback(
    (json: string): boolean => {
      try {
        const data = JSON.parse(json);
        const version = createVersion(
          data.nodes || [],
          data.edges || [],
          `Imported ${new Date().toLocaleString()}`,
          'Imported from external file'
        );
        setVersions((prev) => {
          const newVersions = [version, ...prev].slice(0, maxVersions);
          saveToStorage(newVersions, version.id);
          return newVersions;
        });
        return true;
      } catch (e) {
        console.error('Failed to import version:', e);
        return false;
      }
    },
    [maxVersions, saveToStorage]
  );

  // Get diff between two versions
  const getVersionDiff = useCallback(
    (id1: string, id2: string): VersionDiff => {
      const v1 = versions.find((v) => v.id === id1);
      const v2 = versions.find((v) => v.id === id2);
      if (!v1 || !v2) {
        return {
          addedNodes: [],
          removedNodes: [],
          modifiedNodes: [],
          addedEdges: [],
          removedEdges: [],
          modifiedEdges: [],
        };
      }
      return computeDiff(v1, v2);
    },
    [versions]
  );

  return {
    versions,
    currentVersionId,
    isAutoSaving,
    saveVersion,
    loadVersion,
    deleteVersion,
    exportVersion,
    importVersion,
    getVersionDiff,
  };
}

export default useVersionHistory;
