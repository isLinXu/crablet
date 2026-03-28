import { create } from 'zustand';
import { persist } from 'zustand/middleware';

/**
 * Canvas Version Control Store
 * 
 * Manages canvas snapshots, versions, and rollback functionality.
 * Data flows:
 * - Hot data (current state): Stored in Redis for fast access
 * - Cold data (versions): Stored in SQLite for persistence
 */

export interface CanvasNode {
  id: string;
  type: string;
  label: string;
  position: { x: number; y: number };
  data?: Record<string, unknown>;
}

export interface CanvasEdge {
  id: string;
  source: string;
  target: string;
  label?: string;
}

export interface CanvasSnapshot {
  id: string;
  canvasId: string;
  nodes: CanvasNode[];
  edges: CanvasEdge[];
  metadata?: Record<string, unknown>;
  createdAt: number;
  createdBy?: string;
}

export interface CanvasVersion {
  id: string;
  canvasId: string;
  name?: string;
  description?: string;
  snapshot: CanvasSnapshot;
  createdAt: number;
  createdBy?: string;
}

export interface CanvasVersionState {
  // Current canvas state
  currentCanvasId: string | null;
  
  // Version history for current canvas
  versions: CanvasVersion[];
  
  // Draft/preview state
  draftSnapshot: CanvasSnapshot | null;
  isDraftModified: boolean;
  
  // Undo/Redo stacks
  undoStack: CanvasSnapshot[];
  redoStack: CanvasSnapshot[];
  
  // Lock state (for collaboration)
  lockedNodes: Record<string, { userId: string; userName: string; lockedAt: number }>;
  
  // Actions
  setCurrentCanvas: (canvasId: string | null) => void;
  addVersion: (version: Omit<CanvasVersion, 'id' | 'createdAt'>) => void;
  loadVersion: (versionId: string) => CanvasSnapshot | null;
  rollbackToVersion: (versionId: string) => void;
  deleteVersion: (versionId: string) => void;
  listVersions: () => CanvasVersion[];
  
  // Snapshot management
  createSnapshot: (canvasId: string, nodes: CanvasNode[], edges: CanvasEdge[], metadata?: Record<string, unknown>) => CanvasSnapshot;
  setDraftSnapshot: (snapshot: CanvasSnapshot | null) => void;
  markDraftModified: (modified: boolean) => void;
  
  // Undo/Redo
  pushUndo: (snapshot: CanvasSnapshot) => void;
  undo: () => CanvasSnapshot | null;
  redo: () => CanvasSnapshot | null;
  clearUndoRedo: () => void;
  
  // Lock management
  acquireLock: (nodeId: string, userId: string, userName: string) => boolean;
  releaseLock: (nodeId: string) => void;
  getLockInfo: (nodeId: string) => { userId: string; userName: string; lockedAt: number } | null;
  clearAllLocks: () => void;
}

const generateVersionId = () => `ver-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
const generateSnapshotId = () => `snap-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

export const useCanvasVersionStore = create<CanvasVersionState>()(
  persist(
    (set, get) => ({
      currentCanvasId: null,
      versions: [],
      draftSnapshot: null,
      isDraftModified: false,
      undoStack: [],
      redoStack: [],
      lockedNodes: {},
      
      setCurrentCanvas: (canvasId) => {
        set({ 
          currentCanvasId: canvasId,
          draftSnapshot: null,
          isDraftModified: false,
          undoStack: [],
          redoStack: [],
        });
      },
      
      addVersion: (versionData) => {
        const id = generateVersionId();
        const version: CanvasVersion = {
          ...versionData,
          id,
          createdAt: Date.now(),
        };
        
        set((state) => ({
          versions: [version, ...state.versions].slice(0, 100), // Keep last 100 versions
        }));
        
        // TODO: Persist to backend (Redis + SQLite)
        return id;
      },
      
      loadVersion: (versionId) => {
        const version = get().versions.find(v => v.id === versionId);
        return version?.snapshot ?? null;
      },
      
      rollbackToVersion: (versionId) => {
        const version = get().versions.find(v => v.id === versionId);
        if (!version) return;
        
        const { snapshot } = version;
        
        // Push current state to undo stack before rollback
        const currentSnapshot = get().draftSnapshot;
        if (currentSnapshot) {
          get().pushUndo(currentSnapshot);
        }
        
        set({ draftSnapshot: snapshot, isDraftModified: true });
        
        // TODO: Emit CanvasRollback event via WebSocket
      },
      
      deleteVersion: (versionId) => {
        set((state) => ({
          versions: state.versions.filter(v => v.id !== versionId),
        }));
      },
      
      listVersions: () => {
        return get().versions.filter(v => v.canvasId === get().currentCanvasId);
      },
      
      createSnapshot: (canvasId, nodes, edges, metadata) => {
        const snapshot: CanvasSnapshot = {
          id: generateSnapshotId(),
          canvasId,
          nodes: [...nodes],
          edges: [...edges],
          metadata,
          createdAt: Date.now(),
        };
        return snapshot;
      },
      
      setDraftSnapshot: (snapshot) => {
        set({ draftSnapshot: snapshot, isDraftModified: false });
      },
      
      markDraftModified: (modified) => {
        set({ isDraftModified: modified });
      },
      
      pushUndo: (snapshot) => {
        set((state) => ({
          undoStack: [...state.undoStack, snapshot].slice(-50), // Keep last 50
          redoStack: [], // Clear redo on new action
        }));
      },
      
      undo: () => {
        const { undoStack, draftSnapshot } = get();
        if (undoStack.length === 0) return null;
        
        const previous = undoStack[undoStack.length - 1];
        const newUndoStack = undoStack.slice(0, -1);
        
        // Push current to redo
        if (draftSnapshot) {
          set((state) => ({
            undoStack: newUndoStack,
            redoStack: [...state.redoStack, draftSnapshot].slice(-50),
            draftSnapshot: previous,
            isDraftModified: true,
          }));
        } else {
          set({
            undoStack: newUndoStack,
            draftSnapshot: previous,
            isDraftModified: true,
          });
        }
        
        return previous;
      },
      
      redo: () => {
        const { redoStack } = get();
        if (redoStack.length === 0) return null;
        
        const next = redoStack[redoStack.length - 1];
        const newRedoStack = redoStack.slice(0, -1);
        
        set((state) => ({
          redoStack: newRedoStack,
          undoStack: [...state.undoStack, next].slice(-50),
          draftSnapshot: next,
          isDraftModified: true,
        }));
        
        return next;
      },
      
      clearUndoRedo: () => {
        set({ undoStack: [], redoStack: [] });
      },
      
      acquireLock: (nodeId, userId, userName) => {
        const existing = get().lockedNodes[nodeId];
        if (existing && existing.userId !== userId) {
          return false; // Already locked by someone else
        }
        
        set((state) => ({
          lockedNodes: {
            ...state.lockedNodes,
            [nodeId]: { userId, userName, lockedAt: Date.now() },
          },
        }));
        return true;
      },
      
      releaseLock: (nodeId) => {
        set((state) => {
          const rest = { ...state.lockedNodes };
          delete rest[nodeId];
          return { lockedNodes: rest };
        });
      },
      
      getLockInfo: (nodeId) => {
        return get().lockedNodes[nodeId] ?? null;
      },
      
      clearAllLocks: () => {
        set({ lockedNodes: {} });
      },
    }),
    {
      name: 'crablet-canvas-versions',
      partialize: (state) => ({
        versions: state.versions,
      }),
    }
  )
);

// Helper to create auto-save version
export const autoSaveVersion = async (
  canvasId: string,
  nodes: CanvasNode[],
  edges: CanvasEdge[],
  userId?: string
) => {
  const store = useCanvasVersionStore.getState();
  const snapshot = store.createSnapshot(canvasId, nodes, edges, { autoSave: true });
  
  store.addVersion({
    canvasId,
    snapshot,
    createdBy: userId,
    name: `Auto-save ${new Date().toLocaleString()}`,
  });
};

// Helper to create named version
export const createNamedVersion = (
  canvasId: string,
  nodes: CanvasNode[],
  edges: CanvasEdge[],
  name: string,
  description?: string,
  userId?: string
) => {
  const store = useCanvasVersionStore.getState();
  const snapshot = store.createSnapshot(canvasId, nodes, edges);
  
  return store.addVersion({
    canvasId,
    snapshot,
    name,
    description,
    createdBy: userId,
  });
};
