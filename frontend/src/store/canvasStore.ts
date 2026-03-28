import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { 
  type Edge, 
  type Node, 
  type OnNodesChange, 
  type OnEdgesChange, 
  applyNodeChanges, 
  applyEdgeChanges,
  addEdge as addEdgeToGraph,
  type Connection,
  Position
} from '@xyflow/react';
import dagre from 'dagre';

export type NodeData = {
  label: string;
  type?: string;
  status?: 'pending' | 'running' | 'completed' | 'failed';
  details?: string;
  [key: string]: unknown; // Allow index signature for compatibility
};

export type CanvasState = {
  nodes: Node<NodeData>[];
  edges: Edge[];
  onNodesChange: OnNodesChange<Node<NodeData>>;
  onEdgesChange: OnEdgesChange;
  onConnect: (connection: Connection) => void;
  setNodes: (nodes: Node<NodeData>[]) => void;
  setEdges: (edges: Edge[]) => void;
  layout: () => void;
  addAgentNode: (id: string, label: string) => void;
  addTaskNode: (id: string, label: string, parentId?: string) => void;
  addEdge: (source: string, target: string, label?: string) => void;
  reset: () => void;
};

const getLayoutedElements = (nodes: Node<NodeData>[], edges: Edge[]) => {
  const dagreGraph = new dagre.graphlib.Graph();
  dagreGraph.setDefaultEdgeLabel(() => ({}));

  dagreGraph.setGraph({ rankdir: 'TB', ranksep: 80, nodesep: 50 });

  nodes.forEach((node) => {
    dagreGraph.setNode(node.id, { width: 180, height: 80 });
  });

  edges.forEach((edge) => {
    dagreGraph.setEdge(edge.source, edge.target);
  });

  dagre.layout(dagreGraph);

  const layoutedNodes = nodes.map((node) => {
    const nodeWithPosition = dagreGraph.node(node.id);
    return {
      ...node,
      targetPosition: Position.Top,
      sourcePosition: Position.Bottom,
      position: {
        x: nodeWithPosition.x - 90,
        y: nodeWithPosition.y - 40,
      },
    };
  });

  return { nodes: layoutedNodes, edges };
};

export const useCanvasStore = create<CanvasState>()(
  persist(
    (set, get) => ({
      nodes: [],
      edges: [],
      onNodesChange: (changes) => {
        set({
          nodes: applyNodeChanges(changes, get().nodes),
        });
      },
      onEdgesChange: (changes) => {
        set({
          edges: applyEdgeChanges(changes, get().edges),
        });
      },
      onConnect: (connection) => {
        set({
            edges: addEdgeToGraph(connection, get().edges)
        });
      },
      setNodes: (nodes) => set({ nodes }),
      setEdges: (edges) => set({ edges }),
      layout: () => {
        const { nodes, edges } = getLayoutedElements(get().nodes, get().edges);
        set({ nodes, edges });
      },
      addAgentNode: (id, label) => {
        // Check if exists
        if (get().nodes.find((n) => n.id === id)) return;

        const newNode: Node<NodeData> = {
          id,
          type: 'agent',
          position: { x: 0, y: 0 },
          data: { label, type: 'agent', status: 'running' },
        };
        
        set((state) => {
          const newNodes = [...state.nodes, newNode];
          const { nodes: layoutedNodes, edges } = getLayoutedElements(newNodes, state.edges);
          return { nodes: layoutedNodes, edges };
        });
      },
      addTaskNode: (id, label, parentId) => {
        set((state) => {
            if (state.nodes.find(n => n.id === id)) return state;
            
            const newNode: Node<NodeData> = {
              id,
              type: 'task',
              position: { x: 0, y: 0 },
              data: { label, type: 'task', status: 'pending' },
            };

            const newEdges = [...state.edges];
            if (parentId) {
                const edgeId = `e-${parentId}-${id}`;
                if (!newEdges.find(e => e.id === edgeId)) {
                    newEdges.push({
                        id: edgeId,
                        source: parentId,
                        target: id,
                        animated: true,
                        type: 'smoothstep'
                    });
                }
            }
            
            const newNodes = [...state.nodes, newNode];
            const { nodes: layoutedNodes, edges } = getLayoutedElements(newNodes, newEdges);
            return { nodes: layoutedNodes, edges };
        });
      },
      addEdge: (source, target, label) => {
          const id = `e-${source}-${target}`;
          if (get().edges.find(e => e.id === id)) return;
          
          set((state) => {
              const newEdges = [...state.edges, {
                  id,
                  source,
                  target,
                  label,
                  animated: true,
                  type: 'smoothstep',
                  ...(label && { label })
              }];
              // Optional: re-layout or just add edge
              // const { nodes, edges } = getLayoutedElements(state.nodes, newEdges);
              return { edges: newEdges }; 
          });
      },
      reset: () => set({ nodes: [], edges: [] }),
    }),
    {
      name: 'canvas-storage',
      partialize: (state) => ({ nodes: state.nodes, edges: state.edges }),
    }
  )
);
