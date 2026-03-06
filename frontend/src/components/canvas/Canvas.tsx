import React, { useEffect } from 'react';
import { 
  ReactFlow, 
  Controls, 
  Background, 
  MiniMap,
  Panel,
  Handle,
  Position,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useCanvasStore, type NodeData } from '../../store/canvasStore';
import { RefreshCw, Plus } from 'lucide-react';
import { Button } from '../ui/Button';

// Custom Node Types (Simple for now)
const AgentNode = ({ data }: { data: NodeData }) => (
  <div className="px-4 py-2 shadow-md rounded-md bg-white border-2 border-blue-500 min-w-[150px]">
    <Handle type="target" position={Position.Top} className="w-3 h-3 bg-blue-500" />
    <div className="flex items-center">
      <div className="ml-2">
        <div className="text-lg font-bold text-gray-900">{data.label}</div>
        <div className="text-gray-500 text-xs">{data.type}</div>
      </div>
    </div>
    <Handle type="source" position={Position.Bottom} className="w-3 h-3 bg-blue-500" />
  </div>
);

const TaskNode = ({ data }: { data: NodeData }) => {
    let statusColor = "border-gray-300";
    if (data.status === 'running') statusColor = "border-yellow-500 animate-pulse";
    else if (data.status === 'completed') statusColor = "border-green-500";
    else if (data.status === 'failed') statusColor = "border-red-500";

    return (
      <div className={`px-4 py-2 shadow-md rounded-md bg-white border-2 ${statusColor} min-w-[150px]`}>
        <Handle type="target" position={Position.Top} className="w-3 h-3 bg-gray-500" />
        <div className="flex items-center">
          <div className="ml-2">
            <div className="text-sm font-bold text-gray-900">{data.label}</div>
            <div className="text-gray-500 text-xs uppercase">{data.status}</div>
          </div>
        </div>
        <Handle type="source" position={Position.Bottom} className="w-3 h-3 bg-gray-500" />
      </div>
    );
};

const nodeTypes = {
  agent: AgentNode,
  task: TaskNode,
};

export const Canvas: React.FC = () => {
  const { 
    nodes, 
    edges, 
    onNodesChange, 
    onEdgesChange, 
    onConnect,
    layout, 
    reset,
    addAgentNode,
    addTaskNode
  } = useCanvasStore();

  const handleAddAgent = () => {
    const id = `agent-${Date.now()}`;
    addAgentNode(id, `Agent ${nodes.length + 1}`);
  };

  const handleAddTask = () => {
    const id = `task-${Date.now()}`;
    // Simple logic: attach to last agent if exists
    const lastAgent = [...nodes].reverse().find(n => n.type === 'agent');
    addTaskNode(id, `Task ${nodes.filter(n => n.type === 'task').length + 1}`, lastAgent?.id);
  };

  // Layout on mount or change
  useEffect(() => {
    if (nodes.length > 0) {
        // Only layout if nodes were added programmatically without position?
        // Or always layout? The store layout function resets positions.
        // Maybe we shouldn't layout automatically on every change if user drags?
        // But the original code did this:
        // layout();
    }
  }, [nodes.length, edges.length, layout]);

  return (
    <div className="h-full w-full bg-gray-50 dark:bg-gray-900">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        nodeTypes={nodeTypes}
        fitView
        attributionPosition="bottom-right"
        className="bg-gray-50 dark:bg-gray-900"
      >
        <Background />
        <Controls />
        <MiniMap zoomable pannable />
        <Panel position="top-right" className="flex gap-2">
            <Button size="sm" onClick={handleAddAgent}>
                <Plus className="w-4 h-4 mr-1" />
                Add Agent
            </Button>
            <Button size="sm" variant="secondary" onClick={handleAddTask}>
                <Plus className="w-4 h-4 mr-1" />
                Add Task
            </Button>
            <Button size="sm" variant="secondary" onClick={() => layout()}>
                <RefreshCw className="w-4 h-4 mr-1" />
                Relayout
            </Button>
            <Button size="sm" variant="ghost" onClick={reset}>
                Clear
            </Button>
        </Panel>
      </ReactFlow>
      
      {nodes.length === 0 && (
          <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
              <div className="text-center text-gray-400">
                  <p className="text-lg font-semibold">Canvas Empty</p>
                  <p className="text-sm">Agent workflow visualization will appear here.</p>
              </div>
          </div>
      )}
    </div>
  );
};
